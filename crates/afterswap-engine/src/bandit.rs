//! UCB1 bandit over Pareto-surviving exit strategies.
//!
//! Reuses `katgpt_ruliology::RuliologyArm` (UCB1 math) directly; the crate's
//! `RuliologyBandit::from_strategies` is hardwired to two-player game
//! payoffs, so arm construction from a strategy×window `WinMatrix` lives
//! in `ExitEngine` instead (pruner → survivor indices → `from_arms`).

use katgpt_ruliology::{FsmStrategy, RuliologyArm, SimpleProgram};
use serde::Serialize;

/// Bandit whose arms are Pareto-optimal FSM exit strategies.
pub struct ExitBandit {
    arms: Vec<RuliologyArm>,
    total_pulls: u32,
}

/// UI-facing snapshot of one arm.
#[derive(Debug, Clone, Serialize)]
pub struct ArmSnapshot {
    pub index: usize,
    pub id: u64,
    pub n_states: u8,
    pub transitions: Vec<[u8; 2]>,
    pub outputs: Vec<u8>,
    pub complexity: f32,
    pub mean_reward_bps: f64,
    pub pulls: u32,
    pub ucb1: f64,
    /// Mean simulated edge (bps) from the latest tournament.
    pub sim_edge_bps: f64,
    /// 0 = exhaustively enumerated; n>0 = nth-generation mutant.
    pub generation: u32,
}

impl ExitBandit {
    /// Build from pre-constructed arms (Pareto survivors).
    pub fn from_arms(arms: Vec<RuliologyArm>) -> Self {
        Self {
            arms,
            total_pulls: 0,
        }
    }

    /// UCB1 arm selection.
    pub fn select_arm(&self) -> usize {
        let mut best = 0usize;
        let mut best_score = f64::NEG_INFINITY;
        for (i, arm) in self.arms.iter().enumerate() {
            let score = arm.ucb1_score(self.total_pulls);
            if score > best_score {
                best_score = score;
                best = i;
            }
        }
        best
    }

    /// Record a realized reward (bps edge vs hold) for `arm`.
    pub fn update(&mut self, arm: usize, reward_bps: f64) {
        self.arms[arm].update(reward_bps);
        self.total_pulls += 1;
    }

    /// The FSM behind an arm.
    pub fn strategy(&self, arm: usize) -> &FsmStrategy {
        &self.arms[arm].strategy
    }

    /// Replace `arm`'s strategy (evolution): resets its pulls/payoff.
    pub fn replace_arm(&mut self, arm: usize, strategy: FsmStrategy) {
        self.arms[arm] = RuliologyArm::new(strategy);
    }

    /// Number of arms.
    pub fn num_arms(&self) -> usize {
        self.arms.len()
    }

    /// Immutable arm access (for stat carry-over across refreshes).
    pub fn arms(&self) -> &[RuliologyArm] {
        &self.arms
    }

    /// UI snapshots; `sim_edges[i]` is aligned with arm order (`&[]` to skip).
    pub fn snapshots(
        &self,
        sim_edges: &[f64],
        generations: &std::collections::HashMap<u64, u32>,
    ) -> Vec<ArmSnapshot> {
        self.arms
            .iter()
            .enumerate()
            .map(|(i, arm)| {
                let s = &arm.strategy;
                let n = s.n_states() as usize;
                ArmSnapshot {
                    index: i,
                    id: s.id(),
                    n_states: s.n_states(),
                    transitions: s.transitions()[..n].to_vec(),
                    outputs: s.outputs()[..n].to_vec(),
                    complexity: s.complexity(),
                    mean_reward_bps: arm.payoff(),
                    pulls: arm.pulls(),
                    ucb1: arm.ucb1_score(self.total_pulls),
                    sim_edge_bps: sim_edges.get(i).copied().unwrap_or(0.0),
                    generation: generations.get(&s.id()).copied().unwrap_or(0),
                }
            })
            .collect()
    }
}
