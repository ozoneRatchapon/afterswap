//! Quote snapshots — a pre-trade depth reading captured beside the price it
//! belongs to.
//!
//! Bench 038 measured what a depth reading is worth as a CUPED control variate
//! and found the binding constraint is **freshness, not volume**:
//!
//! | lag | rho(depth_t, depth_t+k) | variance reduction |
//! |-----|-------------------------|--------------------|
//! | 1   | +0.588                  | 34.6%              |
//! | 5   | +0.439                  | 19.3%              |
//! | 30  | +0.114                  |  1.3%              |
//!
//! A depth history sampled a minute apart is worth nothing. The same reading
//! captured beside each quote is worth a third of the variance. So the design
//! goal is not "record depth" — it is "record depth that provably shares a slot
//! with the price the decision used".
//!
//! Two things in the old pipeline made that impossible:
//!
//! 1. **`PricePoller::poll` returned `f64`.** Every quote arrived carrying
//!    `priceImpactPct`, `contextSlot` and the route plan, and all of it was
//!    dropped at the source. Depth could not be attached downstream because it
//!    no longer existed.
//! 2. **The two-quote depth probe could straddle slots.** The recording in
//!    `data/incoming/bonk_depth.jsonl` derives `depth_bps` from a small-clip and
//!    a large-clip quote. Those are two HTTP requests; nothing guaranteed they
//!    were computed against the same chain state, and the row did not say.
//!
//! The fix for (1) is this module. The fix for (2) is to prefer the impact
//! figure the *same* response already carries — `impact_bps` below shares its
//! `context_slot` with `price` by construction, so its lag is structurally zero
//! rather than merely small. The two-quote probe is kept as an optional extra,
//! and now records both slots so staleness is measurable instead of assumed.

use serde::{Deserialize, Serialize};

use crate::types::QuoteResponse;

/// How well-paired a snapshot's control variate is with its price.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// Control variate and price came from one response — same slot by
    /// construction. The only class that supports a lag-0 CUPED adjustment.
    SameQuote,
    /// Two responses, same `context_slot`.
    SameSlot,
    /// Two responses, `gap` slots apart.
    Stale { gap: u64 },
    /// At least one slot is missing; staleness cannot be established.
    Unknown,
}

impl Freshness {
    /// Usable for the CUPED adjustment bench 038 measured. Anything beyond one
    /// slot has already lost a third of its value, and by 30 it is noise.
    pub fn is_usable(self) -> bool {
        matches!(
            self,
            Freshness::SameQuote | Freshness::SameSlot | Freshness::Stale { gap: 1 }
        )
    }
}

/// The larger-clip probe used to derive an executable-depth spread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthProbe {
    /// Spread in bps between the probe clip's price and the primary price.
    pub depth_bps: f64,
    /// Probe notional in the input mint's smallest units.
    pub probe_amount: u64,
    /// Slot the probe was computed against, when the API reported one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_slot: Option<u64>,
}

/// One pre-trade observation: the price a decision may use, and the depth
/// reading that explains its execution cost, captured together.
///
/// Field order is the reading order: identity, then price, then control
/// variates, then provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteSnapshot {
    /// Monotonic counter within one recording session. Gaps mean dropped polls
    /// and are load-bearing — a CUPED lag is counted in sequence steps, so a
    /// silent gap would understate staleness.
    pub seq: u64,
    /// Wall clock, milliseconds since epoch. For human alignment only; never
    /// use it to compute lag, because poll interval and slot time drift apart.
    pub t_ms: u64,
    /// Slot the primary quote was computed against. **This is the freshness
    /// key.** Wall clock is not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_slot: Option<u64>,

    /// Implied price from this quote, unfiltered.
    pub price: f64,
    /// The price the engine actually consumed after the median-of-3 spike
    /// filter, when a filter is in play.
    ///
    /// These differ, and the difference is a lag. The filter emits the median
    /// of ticks `t-2..=t`, so on a rising or falling run the consumed price is
    /// one tick old while `context_slot` describes tick `t`. Recording both
    /// lets the analysis pair the control variate against the price the
    /// decision saw, rather than against the one that happened to arrive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_used: Option<f64>,

    /// Price impact reported by this same quote, in bps. Lag-0 by
    /// construction — same response, same slot as `price`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact_bps: Option<f64>,
    /// The raw `priceImpactPct` string exactly as the API sent it.
    ///
    /// Kept because `impact_bps` assumes the field is a fraction (0.0012 =
    /// 12 bps). If that convention is ever wrong, the raw string lets every
    /// recorded row be reinterpreted without re-recording — the alternative is
    /// discovering a unit error after a month of capture and having nothing to
    /// recompute from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact_raw: Option<String>,

    /// Optional larger-clip probe, when depth-spread capture is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe: Option<DepthProbe>,

    /// First venue in the route plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venue: Option<String>,
    /// Number of hops in the route plan. A route change between quotes moves
    /// depth for reasons unrelated to liquidity, so this is a covariate, not
    /// decoration.
    pub hops: u8,
    /// Round-trip latency of the primary quote request, microseconds. Bounds
    /// how stale the reading already was when it arrived.
    pub latency_us: u64,
}

/// `priceImpactPct` as bps, on the documented convention that the field is a
/// fraction. Pinned by `tests/snapshot.rs` so a convention change fails loudly.
pub fn impact_pct_to_bps(raw: &str) -> Option<f64> {
    raw.trim().parse::<f64>().ok().map(|p| p * 10_000.0)
}

impl QuoteSnapshot {
    /// Build from a primary quote response. `price` is taken from the same
    /// response, so `impact_bps` is same-slot by construction.
    pub fn from_quote(seq: u64, t_ms: u64, latency_us: u64, q: &QuoteResponse) -> Option<Self> {
        let price = q.price()?;
        let impact_raw = q.price_impact_pct.clone();
        Some(Self {
            seq,
            t_ms,
            context_slot: q.context_slot,
            price,
            price_used: None,
            impact_bps: impact_raw.as_deref().and_then(impact_pct_to_bps),
            impact_raw,
            probe: None,
            venue: q.route_plan.first().map(|s| s.venue.clone()),
            hops: q.route_plan.len() as u8,
            latency_us,
        })
    }

    /// Record the post-filter price the engine consumed on this tick.
    pub fn with_price_used(mut self, used: f64) -> Self {
        self.price_used = Some(used);
        self
    }

    /// Attach a larger-clip probe, deriving the depth spread against `price`.
    pub fn with_probe(mut self, probe_amount: u64, probe: &QuoteResponse) -> Self {
        if let Some(pp) = probe.price() {
            let depth_bps = match self.price > 0.0 {
                true => (self.price - pp) / self.price * 10_000.0,
                false => 0.0,
            };
            self.probe = Some(DepthProbe {
                depth_bps,
                probe_amount,
                context_slot: probe.context_slot,
            });
        }
        self
    }

    /// Freshness of this row's best available control variate.
    pub fn freshness(&self) -> Freshness {
        match (&self.probe, self.impact_bps) {
            // No probe: the control variate is the same response's impact.
            (None, Some(_)) => Freshness::SameQuote,
            (None, None) => Freshness::Unknown,
            (Some(p), _) => match (self.context_slot, p.context_slot) {
                (Some(a), Some(b)) if a == b => Freshness::SameSlot,
                (Some(a), Some(b)) => Freshness::Stale {
                    gap: a.abs_diff(b),
                },
                _ => Freshness::Unknown,
            },
        }
    }

    /// The control variate to feed CUPED, preferring the lag-0 one.
    pub fn control_variate(&self) -> Option<f64> {
        self.impact_bps.or(self.probe.as_ref().map(|p| p.depth_bps))
    }
}
