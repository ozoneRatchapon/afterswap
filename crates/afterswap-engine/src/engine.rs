//! `ExitEngine` — the tick-driven orchestrator.
//!
//! Wires ruliology machinery into the post-swap loop:
//! `FsmEnumerator::enumerate` → window tournament (`sim::evaluate_matrix`)
//! → `RuliologyPruner` Pareto filter → `ExitBandit` (UCB1 arms) → the
//! selected FSM drives live tranche exits; realized edge feeds back as the
//! arm's reward. A `SimulationGate` on the latest `WinMatrix` decides when
//! a re-tournament is worth running.

use std::collections::HashMap;

use katgpt_ruliology::{
    FsmEnumerator, FsmStrategy, RuliologyArm, RuliologyPruner, SimpleProgram, SimulationGate,
    SimulationStrategy, WinMatrix,
};
use log::info;
use serde::Serialize;

use crate::bandit::{ArmSnapshot, ExitBandit};
use crate::sim::evaluate_matrix;
use crate::types::{EngineConfig, Position};
use crate::windows::WindowStore;

/// Minimum full windows buffered before the first tournament runs.
const MIN_TOURNAMENT_WINDOWS: usize = 2;

/// Events emitted by `on_tick` for the execution/UI layer.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEvent {
    /// A tranche sell was applied to the paper position at `price`.
    /// The execution layer mirrors it (live mode: real DFlow order).
    TrancheFilled {
        tick: u64,
        arm: usize,
        price: f64,
        frac: f64,
        remaining: f64,
    },
    /// The live evaluation window closed; the driving arm was rewarded.
    WindowClosed {
        arm: usize,
        reward_bps: f64,
        pulls: u32,
    },
    /// A new arm took over the live position.
    ArmSelected { arm: usize, fsm_id: u64 },
    /// A (re-)tournament ran and the arm set was rebuilt.
    Tournament {
        route: String,
        windows_used: usize,
        strategies: usize,
        arms: usize,
        compression_ratio: f32,
    },
    /// Position fully exited.
    PositionClosed { tick: u64, final_value_norm: f64 },
}

/// Serializable gate/routing summary for the UI.
#[derive(Debug, Clone, Serialize)]
pub struct GateSummary {
    pub route: String,
    pub compression_ratio: f32,
    pub is_irreducible: bool,
}

/// Full engine state snapshot for the dashboard.
#[derive(Serialize)]
pub struct EngineSnapshot {
    pub tick: Option<u64>,
    pub last_price: Option<f64>,
    pub position: Option<Position>,
    pub position_value_norm: Option<f64>,
    pub hold_value_norm: Option<f64>,
    pub arms: Vec<ArmSnapshot>,
    pub live_arm: Option<usize>,
    pub live_fsm_state: Option<u8>,
    pub gate: Option<GateSummary>,
    pub completed_windows: usize,
    pub strategies_enumerated: usize,
    pub recent_prices: Vec<f64>,
}

/// The FSM currently driving the live position.
struct LiveArm {
    arm: usize,
    fsm: FsmStrategy,
    ticks: usize,
    start_cash: f64,
    start_remaining: f64,
}

/// Tick-driven post-swap exit engine. Pure — no IO.
pub struct ExitEngine {
    config: EngineConfig,
    store: WindowStore,
    strategies: Vec<FsmStrategy>,
    bandit: Option<ExitBandit>,
    sim_edges: Vec<f64>,
    last_matrix: Option<WinMatrix>,
    gate: SimulationGate,
    last_gate: Option<GateSummary>,
    position: Option<Position>,
    live: Option<LiveArm>,
    completed_windows: usize,
    windows_since_refresh: usize,
    /// Realized reward stats per FSM id, carried across arm rebuilds.
    realized: HashMap<u64, (f64, u32)>,
    prev_price: Option<f64>,
}

impl ExitEngine {
    /// Enumerate the strategy space and prepare an empty engine.
    pub fn new(config: EngineConfig) -> Self {
        let strategies = FsmEnumerator::enumerate(config.n_fsm_states);
        info!(
            "enumerated {} distinct {}-state FSM exit strategies",
            strategies.len(),
            config.n_fsm_states
        );
        let store = WindowStore::new(config.window_len, config.window_stride, config.max_windows);
        Self {
            config,
            store,
            strategies,
            bandit: None,
            sim_edges: Vec::new(),
            last_matrix: None,
            gate: SimulationGate::default(),
            last_gate: None,
            position: None,
            live: None,
            completed_windows: 0,
            windows_since_refresh: 0,
            realized: HashMap::new(),
            prev_price: None,
        }
    }

    /// Feed one price tick; returns events for the execution/UI layer.
    pub fn on_tick(&mut self, price: f64) -> Vec<EngineEvent> {
        let prev = self.prev_price;
        self.prev_price = Some(price);
        self.store.push(price);
        let tick = self.store.last_tick().unwrap_or(0);
        let mut events = Vec::new();

        // Bootstrap or refresh the arm set.
        let window_count = self.store.windows().len();
        let need_bootstrap = self.bandit.is_none() && window_count >= MIN_TOURNAMENT_WINDOWS;
        let need_refresh = self.bandit.is_some()
            && self.windows_since_refresh >= self.config.refresh_every_windows;
        if need_bootstrap || need_refresh {
            if let Some(ev) = self.run_tournament() {
                events.push(ev);
            }
            self.windows_since_refresh = 0;
        }

        // Drive the live position.
        if self.position.is_some() && self.bandit.is_some() {
            let (prev_p, cur_p) = match prev {
                Some(p) => (p, price),
                None => (price, price),
            };
            self.drive_live(tick, prev_p, cur_p, &mut events);
        }

        events
    }

    /// Open a position at the latest price (the swap fill).
    pub fn open_position(&mut self, size: f64) -> Option<&Position> {
        let price = self.store.last_price()?;
        let tick = self.store.last_tick()?;
        self.position = Some(Position::open(price, size, tick));
        self.live = None;
        info!("position opened: {size} @ {price}");
        self.position.as_ref()
    }

    /// Force-close (UI escape hatch). Returns the final normalized value.
    pub fn close_position(&mut self) -> Option<f64> {
        let price = self.store.last_price()?;
        let value = self.position.as_ref().map(|p| p.value_norm(price));
        self.position = None;
        self.live = None;
        value
    }

    /// Latest full snapshot for the dashboard.
    pub fn snapshot(&self, n_prices: usize) -> EngineSnapshot {
        let last_price = self.store.last_price();
        let position_value_norm = match (&self.position, last_price) {
            (Some(pos), Some(p)) => Some(pos.value_norm(p)),
            _ => None,
        };
        let hold_value_norm = match (&self.position, last_price) {
            (Some(pos), Some(p)) => Some(p / pos.entry_price),
            _ => None,
        };
        EngineSnapshot {
            tick: self.store.last_tick(),
            last_price,
            position: self.position.clone(),
            position_value_norm,
            hold_value_norm,
            arms: match &self.bandit {
                Some(b) => b.snapshots(&self.sim_edges),
                None => Vec::new(),
            },
            live_arm: self.live.as_ref().map(|l| l.arm),
            live_fsm_state: self.live.as_ref().map(|l| l.fsm.state()),
            gate: self.last_gate.clone(),
            completed_windows: self.completed_windows,
            strategies_enumerated: self.strategies.len(),
            recent_prices: self.store.recent(n_prices),
        }
    }

    /// Run (or gate-skip) a tournament and rebuild the bandit arms.
    fn run_tournament(&mut self) -> Option<EngineEvent> {
        let all_windows = self.store.windows();
        if all_windows.len() < MIN_TOURNAMENT_WINDOWS {
            return None;
        }

        // Route on the PREVIOUS matrix: reducible dynamics → keep arms.
        let route = match &self.last_matrix {
            Some(m) => self.gate.route(m),
            None => self.gate.route(&WinMatrix::new(
                vec![vec![0.0; 1]; 1],
                vec![0],
            )),
        };
        let route_name = match route.strategy {
            SimulationStrategy::AnalyticalShortcut => "skip",
            SimulationStrategy::LightweightSimulation => "light",
            SimulationStrategy::FullSimulation => "full",
        };
        self.last_gate = Some(GateSummary {
            route: route_name.to_string(),
            compression_ratio: route.compression_ratio,
            is_irreducible: route.is_irreducible,
        });

        let skip = matches!(route.strategy, SimulationStrategy::AnalyticalShortcut)
            && self.bandit.is_some();
        if skip {
            return Some(EngineEvent::Tournament {
                route: route_name.to_string(),
                windows_used: 0,
                strategies: self.strategies.len(),
                arms: self.bandit.as_ref().map(ExitBandit::num_arms).unwrap_or(0),
                compression_ratio: route.compression_ratio,
            });
        }

        // Lightweight → newest half of the windows; full → all.
        let windows: Vec<Vec<f64>> = match route.strategy {
            SimulationStrategy::LightweightSimulation if all_windows.len() > 2 => {
                all_windows[all_windows.len() / 2..].to_vec()
            }
            _ => all_windows,
        };

        let (matrix, complexities) =
            evaluate_matrix(&self.strategies, &windows, self.config.tranche_frac);
        let pruner = RuliologyPruner::new(
            self.config.payoff_threshold_bps,
            self.config.complexity_threshold,
        );
        let survivors = pruner.filter(&matrix, &complexities);
        self.sim_edges = survivors.iter().map(|&i| matrix.avg_payoff(i)).collect();

        let arms: Vec<RuliologyArm> = survivors
            .iter()
            .map(|&i| RuliologyArm::new(self.strategies[i].clone()))
            .collect();
        let mut bandit = ExitBandit::from_arms(arms);

        // Carry realized reward stats across the rebuild (mean-seeded).
        for i in 0..bandit.num_arms() {
            let id = bandit.strategy(i).id();
            if let Some(&(sum, pulls)) = self.realized.get(&id) {
                let mean = match pulls {
                    0 => 0.0,
                    p => sum / f64::from(p),
                };
                for _ in 0..pulls {
                    bandit.update(i, mean);
                }
            }
        }

        let n_arms = bandit.num_arms();
        let windows_used = matrix.payoffs.first().map(Vec::len).unwrap_or(0);
        self.last_matrix = Some(matrix);
        self.bandit = Some(bandit);
        // The live arm index may no longer exist — force reselection.
        self.live = None;

        info!("tournament: route={route_name} windows={windows_used} arms={n_arms}");
        Some(EngineEvent::Tournament {
            route: route_name.to_string(),
            windows_used,
            strategies: self.strategies.len(),
            arms: n_arms,
            compression_ratio: route.compression_ratio,
        })
    }

    /// Step the live FSM one tick and manage window/reward accounting.
    fn drive_live(&mut self, tick: u64, prev_p: f64, cur_p: f64, events: &mut Vec<EngineEvent>) {
        let Some(bandit) = self.bandit.as_mut() else {
            return;
        };
        if bandit.num_arms() == 0 {
            return;
        }

        // (Re)select the driving arm.
        if self.live.is_none() {
            let arm = bandit.select_arm();
            let mut fsm = bandit.strategy(arm).clone();
            fsm.reset();
            let pos = self.position.as_ref().expect("checked by caller");
            self.live = Some(LiveArm {
                arm,
                fsm,
                ticks: 0,
                start_cash: pos.cash_norm,
                start_remaining: pos.remaining_frac,
            });
            events.push(EngineEvent::ArmSelected {
                arm,
                fsm_id: bandit.strategy(arm).id(),
            });
        }

        let live = self.live.as_mut().expect("set above");
        let pos = self.position.as_mut().expect("checked by caller");

        // Step the machine on the realized tick direction.
        let input: u8 = match cur_p > prev_p {
            true => 1,
            false => 0,
        };
        let action = live.fsm.next_action(&[input]);
        live.ticks += 1;

        if action == 1 && !pos.is_closed() {
            let frac = self.config.tranche_frac.min(pos.remaining_frac);
            pos.apply_fill(tick, cur_p, frac);
            events.push(EngineEvent::TrancheFilled {
                tick,
                arm: live.arm,
                price: cur_p,
                frac,
                remaining: pos.remaining_frac,
            });
        }

        // Window boundary → realized reward vs hold counterfactual.
        if live.ticks >= self.config.window_len {
            let rel = cur_p / pos.entry_price;
            let actual = pos.cash_norm + pos.remaining_frac * rel;
            let counterfactual = live.start_cash + live.start_remaining * rel;
            let reward_bps = match counterfactual.abs() > f64::EPSILON {
                true => (actual - counterfactual) / counterfactual * 10_000.0,
                false => 0.0,
            };
            let arm = live.arm;
            bandit.update(arm, reward_bps);
            let entry = self.realized.entry(bandit.strategy(arm).id()).or_insert((0.0, 0));
            entry.0 += reward_bps;
            entry.1 += 1;
            self.completed_windows += 1;
            self.windows_since_refresh += 1;
            events.push(EngineEvent::WindowClosed {
                arm,
                reward_bps,
                pulls: bandit.arms()[arm].pulls(),
            });
            self.live = None;
        }

        if pos.is_closed() {
            let final_value = pos.value_norm(cur_p);
            events.push(EngineEvent::PositionClosed {
                tick,
                final_value_norm: final_value,
            });
            self.position = None;
            self.live = None;
        }
    }
}
