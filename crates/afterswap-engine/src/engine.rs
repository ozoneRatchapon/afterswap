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
    FsmEnumerator, FsmStrategy, FsmTemplateProposer, MAX_STATES, RuliologyArm, RuliologyPruner,
    SimpleProgram, SimulationGate, SimulationStrategy, WinMatrix,
};
use log::info;
use serde::Serialize;

use crate::bandit::{ArmSnapshot, ExitBandit};
use crate::sim::{evaluate_matrix, replay_exit};
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
        /// FSM state after the transition that emitted this sell.
        state: u8,
        /// Tick direction the machine saw: 1 = up, 0 = down/flat.
        input: u8,
        /// Whether the off-peak bit was set (price ≥ peak_drop_bps below
        /// its running peak) when this sell fired.
        off_peak: bool,
    },
    /// The live evaluation window closed; the driving arm was rewarded.
    WindowClosed {
        arm: usize,
        reward_bps: f64,
        pulls: u32,
    },
    /// A new arm took over the live position.
    ArmSelected { arm: usize, fsm_id: u64, name: String },
    Evolved {
        parent_name: String,
        child_name: String,
        generation: u32,
        sim_edge_bps: f64,
    },
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

/// Portable learning state: what's worth keeping across sessions.
/// Enumeration is free to redo; realized rewards and evolved genomes are not.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct LearningState {
    /// (fsm_id, reward_sum_bps, pulls)
    /// (fsm_id, regime, reward_sum_bps, pulls)
    pub realized: Vec<(u64, u8, f64, u32)>,
    /// Evolved genomes: (transitions, outputs, n_states)
    pub evolved: Vec<([[u8; 2]; MAX_STATES], [u8; MAX_STATES], u8)>,
    /// (fsm_id, generation)
    pub generations: Vec<(u64, u32)>,
}

/// Activity-feed ring size.
const RECENT_EVENTS: usize = 24;

/// One engine event with the tick it happened on (activity feed).
#[derive(Debug, Clone, Serialize)]
pub struct TickEvent {
    pub tick: u64,
    pub event: EngineEvent,
}

/// Locked result of the last fully-exited position (keeps the story on
/// screen after close).
#[derive(Debug, Clone, Serialize)]
pub struct ClosedSummary {
    pub closed_at_tick: u64,
    pub final_value_norm: f64,
    pub hold_value_norm: f64,
    /// Final edge vs never-selling, in bps.
    pub edge_bps: f64,
    /// The closed position (entry, fills) for chart/tape rendering.
    pub position: Position,
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
    /// Renoise confidence: fraction of perturbed replays where the live
    /// arm stays top-3. None when no live arm.
    pub live_confidence: Option<f64>,
    pub gate: Option<GateSummary>,
    pub last_closed: Option<ClosedSummary>,
    /// Newest-first recent events for the activity feed.
    pub recent_events: Vec<TickEvent>,
    /// Arm that most recently drove (kept after close for the FSM panel).
    pub last_arm: Option<usize>,
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
    last_closed: Option<ClosedSummary>,
    recent_events: std::collections::VecDeque<TickEvent>,
    last_arm: Option<usize>,
    /// Seeded RNG for the random-arm floor (config.random_arm_seed).
    floor_rng: Option<fastrand::Rng>,
    /// Mutants admitted by evolution (persist across re-tournaments).
    evolved: Vec<FsmStrategy>,
    /// FSM id → generation (0 = enumerated, absent = 0).
    generations: std::collections::HashMap<u64, u32>,
    live: Option<LiveArm>,
    /// Temporal-derivative surprise state: fast/slow EMAs of signed
    /// per-tick returns (bps) and slow EMA of |returns| (vol proxy).
    surprise_fast: f64,
    surprise_slow: f64,
    surprise_vol: f64,
    surprise_cooldown: u32,
    completed_windows: usize,
    windows_since_refresh: usize,
    /// Realized reward stats per FSM id, carried across arm rebuilds.
    /// (fsm_id, regime) -> (reward_sum_bps, pulls). Regime is 0 when
    /// per-regime statistics are disabled, so the map shape never changes.
    realized: HashMap<(u64, u8), (f64, u32)>,
    prev_price: Option<f64>,
}

impl ExitEngine {
    /// Enumerate the strategy space and prepare an empty engine.
    pub fn new(config: EngineConfig) -> Self {
        let seed = config.random_arm_seed;
        let strategies = enumerate_cached(config.n_fsm_states);
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
            last_closed: None,
            recent_events: std::collections::VecDeque::new(),
            last_arm: None,
            floor_rng: seed.map(fastrand::Rng::with_seed),
            evolved: Vec::new(),
            generations: std::collections::HashMap::new(),
            live: None,
            surprise_fast: 0.0,
            surprise_slow: 0.0,
            surprise_vol: 0.0,
            surprise_cooldown: 0,
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

        // Temporal-derivative surprise: fast vs slow EMA of signed returns,
        // normalized by volatility. A spike means the market's drift flipped
        // faster than the refresh cadence notices.
        const FAST_ALPHA: f64 = 0.3;
        const SLOW_ALPHA: f64 = 0.05;
        let mut surprised = false;
        if let Some(p) = prev {
            let ret_bps = (price - p) / p * 10_000.0;
            self.surprise_fast += FAST_ALPHA * (ret_bps - self.surprise_fast);
            self.surprise_slow += SLOW_ALPHA * (ret_bps - self.surprise_slow);
            self.surprise_vol += SLOW_ALPHA * (ret_bps.abs() - self.surprise_vol);
            self.surprise_cooldown = self.surprise_cooldown.saturating_sub(1);
            if self.config.surprise_ratio > 0.0 && self.surprise_cooldown == 0 {
                let deviation = (self.surprise_fast - self.surprise_slow).abs();
                surprised = deviation / self.surprise_vol.max(0.1) >= self.config.surprise_ratio;
            }
        }

        // Bootstrap or refresh the arm set.
        let window_count = self.store.windows().len();
        let need_bootstrap = self.bandit.is_none() && window_count >= MIN_TOURNAMENT_WINDOWS;
        let need_refresh = self.bandit.is_some()
            && self.windows_since_refresh >= self.config.refresh_every_windows;
        let need_surprise =
            surprised && self.bandit.is_some() && window_count >= MIN_TOURNAMENT_WINDOWS;
        if need_bootstrap || need_refresh || need_surprise {
            // Surprise overrides the gate's skip: fresh evidence of regime
            // change is exactly when "dynamics unchanged" stops being true.
            if let Some(ev) = self.run_tournament(need_surprise && !need_refresh) {
                events.push(ev);
            }
            self.windows_since_refresh = 0;
            if need_surprise {
                self.surprise_cooldown = self.config.window_len as u32;
                self.surprise_fast = self.surprise_slow;
            }
        }

        // Drive the live position.
        if self.position.is_some() && self.bandit.is_some() {
            let (prev_p, cur_p) = match prev {
                Some(p) => (p, price),
                None => (price, price),
            };
            self.drive_live(tick, prev_p, cur_p, &mut events);
        }

        let tick = self.store.last_tick().unwrap_or(0);
        let window_closed = events
            .iter()
            .any(|e| matches!(e, EngineEvent::WindowClosed { .. }));
        if window_closed
            && self.config.evolve_every_windows > 0
            && self
                .completed_windows
                .is_multiple_of(self.config.evolve_every_windows)
            && let Some(ev) = self.evolve_step(tick)
        {
            events.push(ev);
        }
        for ev in &events {
            self.recent_events.push_front(TickEvent {
                tick,
                event: ev.clone(),
            });
        }
        self.recent_events.truncate(RECENT_EVENTS);
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
        let pos = self.position.take()?;
        let tick = self.store.last_tick().unwrap_or(0);
        let value = pos.value_norm(price);
        self.record_close(pos, tick, price);
        self.live = None;
        Some(value)
    }

    /// Store the locked result of a fully- or force-closed position.
    fn record_close(&mut self, position: Position, tick: u64, price: f64) {
        let final_value_norm = position.value_norm(price);
        let hold_value_norm = price / position.entry_price;
        let edge_bps = match hold_value_norm.abs() > f64::EPSILON {
            true => (final_value_norm - hold_value_norm) / hold_value_norm * 10_000.0,
            false => 0.0,
        };
        self.last_closed = Some(ClosedSummary {
            closed_at_tick: tick,
            final_value_norm,
            hold_value_norm,
            edge_bps,
            position,
        });
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
                Some(b) => b.snapshots(&self.sim_edges, &self.generations),
                None => Vec::new(),
            },
            live_arm: self.live.as_ref().map(|l| l.arm),
            live_fsm_state: self.live.as_ref().map(|l| l.fsm.state()),
            live_confidence: self.live_confidence(),
            gate: self.last_gate.clone(),
            last_closed: self.last_closed.clone(),
            recent_events: self.recent_events.iter().cloned().collect(),
            last_arm: self.last_arm,
            completed_windows: self.completed_windows,
            strategies_enumerated: self.strategies.len(),
            recent_prices: self.store.recent(n_prices),
        }
    }

    /// Run (or gate-skip) a tournament and rebuild the bandit arms.
    /// Closed-form regime label from the surprise EMAs: 1 = trend-up,
    /// 2 = trend-down, 0 = chop. No training, no thresholds beyond a
    /// drift-to-volatility ratio, so it stays deterministic (G1/G6).
    fn current_regime(&self) -> u8 {
        if !self.config.per_regime_stats {
            return 0;
        }
        const DRIFT_RATIO: f64 = 0.25;
        let vol = self.surprise_vol.max(0.1);
        let drift = self.surprise_slow / vol;
        match drift {
            d if d >= DRIFT_RATIO => 1,
            d if d <= -DRIFT_RATIO => 2,
            _ => 0,
        }
    }

    /// One evolution step: mutate current arms (incl. 4-state growth past
    /// the enumerable frontier), replay candidates on stored windows, and
    /// replace the worst arm when the best mutant beats it. Deterministic
    /// for a given tick (seeded RNG) — GOAT G1 holds.
    fn evolve_step(&mut self, tick: u64) -> Option<EngineEvent> {
        let windows = self.store.windows();
        if windows.len() < MIN_TOURNAMENT_WINDOWS || self.sim_edges.is_empty() {
            return None;
        }
        let bandit = self.bandit.as_mut()?;
        let mut rng = fastrand::Rng::with_seed(0xAF7E_25EE ^ tick);

        let mut candidates: Vec<(u64, FsmStrategy)> = Vec::new();
        for _ in 0..self.config.evolve_candidates {
            let parent = bandit
                .strategy(rng.u32(..bandit.num_arms() as u32) as usize)
                .clone();
            let grow = (parent.n_states() as usize) < MAX_STATES && rng.f32() < 0.35;
            let child = match grow {
                true => grow_state(&parent, &mut rng),
                false => FsmTemplateProposer::default_for(parent.n_states())
                    .propose(&parent, &mut rng),
            };
            let known = (0..bandit.num_arms()).any(|i| bandit.strategy(i).id() == child.id())
                || candidates.iter().any(|(_, c)| c.id() == child.id());
            if !known {
                candidates.push((parent.id(), child));
            }
        }
        if candidates.is_empty() {
            return None;
        }

        let pool: Vec<FsmStrategy> = candidates.iter().map(|(_, c)| c.clone()).collect();
        let (matrix, _) = evaluate_matrix(
            &pool,
            &windows,
            self.config.tranche_frac,
            self.config.peak_drop_bps,
        );
        let best = (0..pool.len()).max_by(|&a, &b| {
            matrix.avg_payoff(a).total_cmp(&matrix.avg_payoff(b))
        })?;
        let best_edge = matrix.avg_payoff(best);

        let worst = (0..self.sim_edges.len())
            .min_by(|&a, &b| self.sim_edges[a].total_cmp(&self.sim_edges[b]))?;
        if best_edge <= self.sim_edges[worst] {
            return None;
        }

        let (parent_id, child) = candidates.swap_remove(best);
        let generation = self.generations.get(&parent_id).copied().unwrap_or(0) + 1;
        self.generations.insert(child.id(), generation);
        bandit.replace_arm(worst, child.clone());
        self.sim_edges[worst] = best_edge;
        self.evolved.push(child.clone());
        if self.evolved.len() > MAX_EVOLVED {
            self.evolved.remove(0);
        }
        // The replaced arm may be driving — force reselection next tick.
        if self.live.as_ref().is_some_and(|l| l.arm == worst) {
            self.live = None;
        }
        info!(
            "evolved gen{generation}: {parent_id:x} → {:x} ({best_edge:+.1} bps sim)",
            child.id()
        );
        Some(EngineEvent::Evolved {
            parent_name: crate::bandit::machine_name(parent_id),
            child_name: crate::bandit::machine_name(child.id()),
            generation,
            sim_edge_bps: best_edge,
        })
    }

    /// Renoise confidence (perturb → re-resolve → measure drift): the live
    /// arm's replay rank is computed on the clean trailing window, then on
    /// P noise-perturbed copies (noise scaled to observed tick volatility).
    /// Confidence = fraction of perturbations where the rank drifts ≤ 3
    /// places — "would this decision survive a slightly different market?"
    fn live_confidence(&self) -> Option<f64> {
        const PERTURBATIONS: usize = 8;
        const MAX_DRIFT: usize = 3;
        let live = self.live.as_ref()?;
        let bandit = self.bandit.as_ref()?;
        let window = self.store.recent(self.config.window_len);
        if window.len() < self.config.window_len.min(8) {
            return None;
        }
        let rank_of = |prices: &[f64]| -> usize {
            let edges: Vec<f64> = (0..bandit.num_arms())
                .map(|i| {
                    replay_exit(
                        bandit.strategy(i),
                        prices,
                        self.config.tranche_frac,
                        self.config.peak_drop_bps,
                    )
                })
                .collect();
            let live_edge = edges[live.arm];
            edges.iter().filter(|&&e| e > live_edge).count()
        };
        let clean_rank = rank_of(&window);
        let mean_abs_delta = window
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .sum::<f64>()
            / (window.len() - 1) as f64;
        let tick = self.store.last_tick().unwrap_or(0);
        let mut rng = fastrand::Rng::with_seed(0x00C0_F1DE ^ tick);
        let hits = (0..PERTURBATIONS)
            .filter(|_| {
                let perturbed: Vec<f64> = window
                    .iter()
                    .map(|&p| p + (rng.f64() - 0.5) * 2.0 * mean_abs_delta)
                    .collect();
                rank_of(&perturbed).abs_diff(clean_rank) <= MAX_DRIFT
            })
            .count();
        Some(hits as f64 / PERTURBATIONS as f64)
    }

    /// The open position, if any (cheap accessor — the shadow evaluator
    /// needs the entry price without building a full snapshot).
    pub fn position(&self) -> Option<&Position> {
        self.position.as_ref()
    }

    /// Export the learning artifacts (for localStorage / cross-session).
    pub fn export_learning(&self) -> LearningState {
        LearningState {
            realized: self
                .realized
                .iter()
                .map(|(&(id, regime), &(sum, pulls))| (id, regime, sum, pulls))
                .collect(),
            evolved: self
                .evolved
                .iter()
                .map(|f| (*f.transitions(), *f.outputs(), f.n_states()))
                .collect(),
            generations: self.generations.iter().map(|(&k, &v)| (k, v)).collect(),
        }
    }

    /// Import learning artifacts. Call BEFORE feeding prices so the first
    /// tournament already sees the evolved pool and realized seeding.
    pub fn import_learning(&mut self, state: &LearningState) {
        for &(id, regime, sum, pulls) in &state.realized {
            self.realized.insert((id, regime), (sum, pulls));
        }
        for &(transitions, outputs, n_states) in &state.evolved {
            let fsm = FsmStrategy::new(transitions, outputs, n_states, 0);
            if !self.evolved.iter().any(|f| f.id() == fsm.id()) {
                self.evolved.push(fsm);
            }
        }
        self.evolved.truncate(MAX_EVOLVED);
        for &(id, generation) in &state.generations {
            self.generations.insert(id, generation);
        }
    }

    fn run_tournament(&mut self, forced_by_surprise: bool) -> Option<EngineEvent> {
        let all_windows = self.store.windows();
        if all_windows.len() < MIN_TOURNAMENT_WINDOWS {
            return None;
        }

        // Route on the PREVIOUS matrix: reducible dynamics → keep arms.
        // First run has no matrix — that's a bootstrap, always full.
        let (route, route_name) = match (&self.last_matrix, forced_by_surprise) {
            (_, true) => (None, "surprise"),
            (m, false) => match m {
                Some(m) => {
                let r = self.gate.route(m);
                let name = match r.strategy {
                    SimulationStrategy::AnalyticalShortcut => "skip",
                    SimulationStrategy::LightweightSimulation => "light",
                    SimulationStrategy::FullSimulation => "full",
                };
                (Some(r), name)
            }
                None => (None, "bootstrap"),
            },
        };
        let (compression_ratio, is_irreducible) = match &route {
            Some(r) => (r.compression_ratio, r.is_irreducible),
            None => (1.0, true),
        };
        self.last_gate = Some(GateSummary {
            route: route_name.to_string(),
            compression_ratio,
            is_irreducible,
        });

        let skip = matches!(
            route.as_ref().map(|r| &r.strategy),
            Some(SimulationStrategy::AnalyticalShortcut)
        ) && self.bandit.is_some();
        if skip {
            return Some(EngineEvent::Tournament {
                route: route_name.to_string(),
                windows_used: 0,
                strategies: self.strategies.len(),
                arms: self.bandit.as_ref().map(ExitBandit::num_arms).unwrap_or(0),
                compression_ratio,
            });
        }

        // Lightweight → newest half of the windows; full/bootstrap → all.
        let light = matches!(
            route.as_ref().map(|r| &r.strategy),
            Some(SimulationStrategy::LightweightSimulation)
        );
        let windows: Vec<Vec<f64>> = match light && all_windows.len() > 2 {
            true => all_windows[all_windows.len() / 2..].to_vec(),
            false => all_windows,
        };

        // Pool = the exhaustive enumeration plus every admitted mutant.
        let pool: Vec<FsmStrategy> = self
            .strategies
            .iter()
            .chain(self.evolved.iter())
            .cloned()
            .collect();
        let (matrix, complexities) =
            evaluate_matrix(
                &pool,
                &windows,
                self.config.tranche_frac,
                self.config.peak_drop_bps,
            );
        let pruner = RuliologyPruner::new(
            self.config.payoff_threshold_bps,
            self.config.complexity_threshold,
        );
        let mut survivors = pruner.filter(&matrix, &complexities);
        // Cap arms: top by simulated edge (bps), simplest first on ties.
        // Plackett–Luce rank-consistency ordering was tried here (roadmap
        // #3, bench 006) and MEASURED WORSE on every floor — the objective
        // is bps magnitude, not rank consistency; a machine that wins big
        // in trend windows beats one that consistently edges flat ones.
        // PL stays available in `rating.rs`; mean payoff stays the ranker.
        // On flat windows nothing dominates → the cap does the bounding.
        survivors.sort_by(|&a, &b| {
            matrix
                .avg_payoff(b)
                .total_cmp(&matrix.avg_payoff(a))
                .then(complexities[a].total_cmp(&complexities[b]))
        });
        survivors.truncate(self.config.max_arms);
        self.sim_edges = survivors.iter().map(|&i| matrix.avg_payoff(i)).collect();

        let arms: Vec<RuliologyArm> = survivors
            .iter()
            .map(|&i| RuliologyArm::new(pool[i].clone()))
            .collect();
        let mut bandit = ExitBandit::from_arms(arms);

        // Carry realized reward stats across the rebuild (mean-seeded),
        // reading the record for the regime we are in now: a machine's
        // downtrend track record should not be diluted by rally windows.
        let regime = self.current_regime();
        for i in 0..bandit.num_arms() {
            let id = bandit.strategy(i).id();
            // Shrinkage: trust the regime-specific record only once it has
            // enough pulls to be worth more than the pooled one; otherwise
            // fall back to all regimes. Splitting statistics without this
            // measured worse (bench 016) — thin buckets are noisy buckets.
            const MIN_REGIME_PULLS: u32 = 4;
            let regime_record = self
                .realized
                .get(&(id, regime))
                .copied()
                .filter(|&(_, pulls)| pulls >= MIN_REGIME_PULLS);
            let pooled = || {
                let (mut s, mut p) = (0.0f64, 0u32);
                for r in 0..3u8 {
                    if let Some(&(ss, pp)) = self.realized.get(&(id, r)) {
                        s += ss;
                        p += pp;
                    }
                }
                match p {
                    0 => None,
                    _ => Some((s, p)),
                }
            };
            if let Some((sum, pulls)) = regime_record.or_else(pooled) {
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
            compression_ratio,
        })
    }

    /// Step the live FSM one tick and manage window/reward accounting.
    fn drive_live(&mut self, tick: u64, prev_p: f64, cur_p: f64, events: &mut Vec<EngineEvent>) {
        // Computed before the bandit borrow: `self.realized` and
        // `self.bandit` are disjoint fields, but a `&self` method call
        // would borrow the whole struct.
        let regime = self.current_regime();
        let Some(bandit) = self.bandit.as_mut() else {
            return;
        };
        if bandit.num_arms() == 0 {
            return;
        }

        // (Re)select the driving arm.
        if self.live.is_none() {
            let arm = match self.floor_rng.as_mut() {
                Some(rng) => rng.u32(..bandit.num_arms() as u32) as usize,
                None => bandit.select_arm(),
            };
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
            self.last_arm = Some(arm);
            events.push(EngineEvent::ArmSelected {
                arm,
                fsm_id: bandit.strategy(arm).id(),
                name: crate::bandit::machine_name(bandit.strategy(arm).id()),
            });
        }

        let live = self.live.as_mut().expect("set above");
        let pos = self.position.as_mut().expect("checked by caller");

        // Two machine steps per tick: direction bit, then off-peak bit —
        // the same protocol the tournament replays use (sim::replay_exit).
        let input: u8 = match cur_p > prev_p {
            true => 1,
            false => 0,
        };
        pos.peak_price = pos.peak_price.max(cur_p);
        let off_peak = (pos.peak_price - cur_p) / pos.peak_price * 10_000.0
            >= self.config.peak_drop_bps;
        live.fsm.next_action(&[input]);
        let action = live.fsm.next_action(&[u8::from(off_peak)]);
        live.ticks += 1;

        if action == 1 && !pos.is_closed() {
            let frac = self.config.tranche_frac.min(pos.remaining_frac);
            pos.apply_fill_with_cost(tick, cur_p, frac, self.config.fill_cost_bps);
            events.push(EngineEvent::TrancheFilled {
                tick,
                arm: live.arm,
                price: cur_p,
                frac,
                remaining: pos.remaining_frac,
                state: live.fsm.state(),
                input,
                off_peak,
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
            let seated_id = bandit.strategy(arm).id();

            // Off-policy credit: the realized window is equally informative
            // about every arm that did NOT drive — replay each one on it and
            // credit its counterfactual edge. Note the measures differ
            // slightly (seated arm scores actual-vs-hold on the live
            // position; others score a fresh replay), but both are
            // "bps vs holding over this window", which is what UCB1 ranks.
            let mut off_policy: Vec<(u64, f64)> = Vec::new();
            if self.config.off_policy_credit {
                let window = self.store.recent(self.config.window_len);
                if window.len() >= 2 {
                    for i in 0..bandit.num_arms() {
                        if i == arm {
                            continue;
                        }
                        let edge = crate::sim::replay_exit_cost(
                            bandit.strategy(i),
                            &window,
                            self.config.tranche_frac,
                            self.config.peak_drop_bps,
                            self.config.fill_cost_bps,
                        );
                        let id = bandit.strategy(i).id();
                        bandit.update(i, edge);
                        off_policy.push((id, edge));
                    }
                }
            }
            let e = self.realized.entry((seated_id, regime)).or_insert((0.0, 0));
            e.0 += reward_bps;
            e.1 += 1;
            for (id, edge) in off_policy {
                let e = self.realized.entry((id, regime)).or_insert((0.0, 0));
                e.0 += edge;
                e.1 += 1;
            }
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
            // Reward the partial window before the position vanishes — a
            // machine that dumps fast must still teach the bandit. Guarded
            // on `self.live`: if the window boundary above already rewarded
            // and cleared it, don't double-count.
            if let Some(l) = self.live.as_ref() {
                let rel = cur_p / pos.entry_price;
                let actual = pos.cash_norm + pos.remaining_frac * rel;
                let counterfactual = l.start_cash + l.start_remaining * rel;
                let reward_bps = match counterfactual.abs() > f64::EPSILON {
                    true => (actual - counterfactual) / counterfactual * 10_000.0,
                    false => 0.0,
                };
                let arm = l.arm;
                bandit.update(arm, reward_bps);
                let entry = self
                    .realized
                    .entry((bandit.strategy(arm).id(), regime))
                    .or_insert((0.0, 0));
                entry.0 += reward_bps;
                entry.1 += 1;
                events.push(EngineEvent::WindowClosed {
                    arm,
                    reward_bps,
                    pulls: bandit.arms()[arm].pulls(),
                });
            }
            events.push(EngineEvent::PositionClosed {
                tick,
                final_value_norm: final_value,
            });
            let closed = self.position.take().expect("checked above");
            self.record_close(closed, tick, cur_p);
            self.live = None;
        }
    }
}

/// Enumeration is pure and deterministic per state count — cache it per
/// process so repeated engine construction (Workers requests, browser
/// re-boots, GOAT sims) pays the blake3 dedup exactly once.
fn enumerate_cached(n_states: u8) -> Vec<FsmStrategy> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<u8, Vec<FsmStrategy>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cache.lock().expect("enumeration cache poisoned");
    guard
        .entry(n_states)
        .or_insert_with(|| FsmEnumerator::enumerate(n_states))
        .clone()
}

/// Pool cap for admitted mutants (bounds tournament cost).
const MAX_EVOLVED: usize = 64;

/// Grow a parent FSM by one state: copy tables, give the new state random
/// transitions/output, and reroute one existing edge into it.
fn grow_state(parent: &FsmStrategy, rng: &mut fastrand::Rng) -> FsmStrategy {
    let n = parent.n_states();
    debug_assert!((n as usize) < MAX_STATES);
    let mut transitions = *parent.transitions();
    let mut outputs = *parent.outputs();
    let new = n as usize;
    transitions[new] = [rng.u8(..n + 1), rng.u8(..n + 1)];
    outputs[new] = rng.u8(..2);
    let (st, input) = (rng.u32(..new as u32) as usize, rng.u32(..2) as usize);
    transitions[st][input] = new as u8;
    FsmStrategy::new(transitions, outputs, n + 1, 0)
}
