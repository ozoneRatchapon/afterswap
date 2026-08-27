//! Execution-cycle records and the net-margin decomposition.
//!
//! The quantity under test is the net realisable margin identity from the cost
//! research, measured rather than modelled:
//!
//! ```text
//! Margin_net = Spread_gross - (Fee_pool + Tip_priority + Drift_latency + Fee_L1)
//! ```
//!
//! Each component is recorded separately, for a reason that decides whether the
//! experiment works. A control variate taken from the arrival quote predicts
//! **impact**, not drift: `impact_bps` describes how much the pool moves against
//! a clip of this size, and knows nothing about where the price wanders between
//! quote and block. Applying CUPED to the aggregate would dilute a strong
//! covariate across a term it cannot explain. Keeping the components apart lets
//! the analysis adjust the impact component — where bench 038's rho lives — and
//! report the aggregate honestly beside it.
//!
//! Sign convention throughout: **positive is in our favour.** `realised_bps`
//! above zero means the fill beat arrival; cost components are subtracted, so
//! `net_margin_bps` above zero means the cycle made money after everything.

use serde::{Deserialize, Serialize};

/// Lamports per SOL.
const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;

/// One executed cycle: arrival state, fill outcome, and explicit costs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionCycle {
    pub cycle: u64,
    pub t_ms: u64,

    /// Slot of the arrival quote — the reference point the shortfall is against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrival_slot: Option<u64>,
    /// Arrival price: the quote in hand when the decision was made.
    pub arrival_price: f64,
    /// Price impact from the arrival quote, in bps. **The lag-0 control
    /// variate** — same response as `arrival_price`, so same slot by
    /// construction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrival_impact_bps: Option<f64>,

    /// Slot the fill landed in, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_slot: Option<u64>,
    /// Price quoted immediately before submission — isolates delay from impact.
    pub submit_price: f64,
    /// Effective realised price of the fill.
    pub fill_price: f64,

    /// Notional executed, in quote-currency units.
    pub notional: f64,
    /// Validator priority tip actually paid.
    ///
    /// Converted to bps against `notional`, so the same tip is a large cost on
    /// a small clip and a rounding error on a large one. The research cost
    /// table's 10-15 bps tip figure is therefore a claim about retail clip
    /// sizes, not a property of the tip auction — holding notional fixed across
    /// cycles is what keeps it from becoming a confound.
    pub tip_lamports: u64,
    /// L1 base fee actually paid.
    pub l1_fee_lamports: u64,
    /// SOL price in quote currency, for converting lamports to bps.
    pub sol_price: f64,

    /// Route fingerprint at arrival — `venue|hops`.
    pub arrival_route: String,
    /// Route fingerprint at submission. A change between the two makes the
    /// cycle a different experiment, not a noisy one.
    pub submit_route: String,

    /// Whether the order filled at all. Unfilled cycles are not zero-outcome
    /// cycles; dropping them silently would condition the sample on success.
    pub filled: bool,

    /// True when the fill was modelled rather than executed.
    ///
    /// Paper runs exercise the whole harness — schema, admissibility, CUPED
    /// path — without capital, which is how the plumbing gets verified before
    /// any is spent. But a modelled fill price is a restatement of the quote,
    /// so a paper run can only ever confirm the machinery works, never what the
    /// margin is. The flag exists so analysis can refuse to conflate the two;
    /// defaults false so a hand-written record is treated as live until it says
    /// otherwise.
    #[serde(default)]
    pub simulated: bool,

    /// Transaction signature, for live cycles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Revert reason when the transaction landed but failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revert: Option<String>,
}

/// The net-margin identity, decomposed.
#[derive(Debug, Clone, Copy)]
pub struct MarginBreakdown {
    /// Fill against arrival — total realised movement, drift included.
    pub realised_bps: f64,
    /// Arrival to submission: delay cost, nothing to do with our clip size.
    pub drift_bps: f64,
    /// Submission to fill: execution slip, what the control variate predicts.
    pub impact_bps: f64,
    /// Priority tip as bps of notional.
    pub tip_bps: f64,
    /// L1 base fee as bps of notional.
    pub l1_bps: f64,
    /// Realised movement net of explicit costs.
    pub net_margin_bps: f64,
}

fn lamports_to_bps(lamports: u64, sol_price: f64, notional: f64) -> f64 {
    match notional > 0.0 {
        true => (lamports as f64 / LAMPORTS_PER_SOL) * sol_price / notional * 10_000.0,
        false => 0.0,
    }
}

impl ExecutionCycle {
    /// Decompose this cycle. Returns `None` for an unfilled cycle or a
    /// non-positive arrival price — both are recorded, neither is a margin.
    pub fn breakdown(&self) -> Option<MarginBreakdown> {
        if !self.filled || self.arrival_price <= 0.0 || self.submit_price <= 0.0 {
            return None;
        }
        let realised_bps = (self.fill_price - self.arrival_price) / self.arrival_price * 10_000.0;
        let drift_bps = (self.submit_price - self.arrival_price) / self.arrival_price * 10_000.0;
        let impact_bps = (self.fill_price - self.submit_price) / self.submit_price * 10_000.0;
        let tip_bps = lamports_to_bps(self.tip_lamports, self.sol_price, self.notional);
        let l1_bps = lamports_to_bps(self.l1_fee_lamports, self.sol_price, self.notional);
        Some(MarginBreakdown {
            realised_bps,
            drift_bps,
            impact_bps,
            tip_bps,
            l1_bps,
            net_margin_bps: realised_bps - tip_bps - l1_bps,
        })
    }

    /// Whether the route was stable across the cycle. An unstable route changes
    /// the venue, the fee tier and the depth profile at once; such cycles
    /// belong in a separate stratum, not in the pooled estimate.
    pub fn route_stable(&self) -> bool {
        self.arrival_route == self.submit_route
    }

    /// Slots between arrival quote and fill, when both are known. This is the
    /// staleness of the control variate for this cycle — bench 038's decay is
    /// measured in exactly these units.
    pub fn control_lag_slots(&self) -> Option<u64> {
        Some(self.fill_slot?.abs_diff(self.arrival_slot?))
    }

    /// Admissible for the pooled CUPED estimate: filled, route-stable, and
    /// carrying a control variate whose lag is inside the usable band.
    pub fn admissible(&self, max_lag_slots: u64) -> bool {
        self.filled
            && self.route_stable()
            && self.arrival_impact_bps.is_some()
            && self.control_lag_slots().is_none_or(|g| g <= max_lag_slots)
    }
}

/// Outcome/covariate pairs for CUPED, drawn from admissible cycles only.
///
/// Returns `(net_margin, impact_component, control_variate)`. The impact
/// component is offered separately because that is the term the arrival quote's
/// impact figure actually predicts.
pub fn cuped_inputs(cycles: &[ExecutionCycle], max_lag_slots: u64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut net = Vec::new();
    let mut impact = Vec::new();
    let mut control = Vec::new();
    for c in cycles.iter().filter(|c| c.admissible(max_lag_slots)) {
        let (Some(b), Some(x)) = (c.breakdown(), c.arrival_impact_bps) else {
            continue;
        };
        net.push(b.net_margin_bps);
        impact.push(b.impact_bps);
        control.push(x);
    }
    (net, impact, control)
}
