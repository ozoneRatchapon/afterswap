# 007 — What TradingView Ultimate actually sells, and which parts matter here

Source: https://www.tradingview.com/pricing/ (Ultimate, $199.95/mo annual;
$0 Basic → $12.95 Essential → $29.95 Plus → $59.95 Premium → $199.95 Ultimate),
read 2026-08-30.

**Scrape caveat:** the markdown extraction preserved numeric rows
("Indicators per chart 2 / 5 / 10 / 25 / 50") but flattened the ✓ marks on
boolean rows, so a boolean feature's exact tier is not directly readable.
Where a row shows N values against 5 columns, the feature exists only in the
top N tiers — that inference is used below and is marked as such. The
per-plan bullet lists on the page are identical across all four paid tiers
(they render "show all key features"), so they carry no tier signal at all.

## The structural finding

**Roughly four-fifths of the ladder is quota, not capability.** Charts per
tab (1→16), indicators per chart (2→50), price alerts (3→1,000), watchlists,
portfolios, saved layouts, parallel connections (2→200) are the same feature
metered harder. None of it is a product idea; it is packaging.

The genuine capability gates cluster in **three** places. Two of them are
where afterswap is already pointed, which is the useful part of this exercise.

### Gate 1 — time resolution (the actual $200/mo moat)

- `Historical data by the tick` — **one** value present (7 days) ⇒ Ultimate only
- `Historical data by the second` — **two** values (All, All) ⇒ Premium + Ultimate
- `Historical data by the minute` — 180d / 365d / All / All ⇒ Essential and up
- Plus: second-based intervals, tick-based intervals, second-based alerts,
  "fastest data flow", dedicated backup data feed
- `Time limit for making calculations`: 20s / 40s / 40s / 40s / **100s**

The single most expensive thing on the page is **sub-minute resolution and the
compute budget to use it**. Tick history is the only data row exclusive to the
top tier.

This is external price validation for `.plans/006`. The ms-cadence work is not
a micro-optimisation; it is the axis the incumbent charges the most money for.

### Gate 2 — simulation fidelity

The backtesting rows form a fidelity ladder, not a feature list:
strategy backtesting → basic report metrics → advanced report metrics →
**deep backtesting** → **high detalization of historical bars** →
**each history tick execution**.

The premium product is not "can you backtest" but "does the backtest execute
against every tick rather than a bar approximation."

Bearing on us: bench 018 is on **1-minute bars** (already recorded in 006 as
the evidence correction). The industry's top tier is sold on precisely the gap
between that and per-tick execution. `benches/041_ms_feed` is step one of
closing it.

### Gate 3 — replay / interrogation of history

`Bar Replay` (all paid tiers), then `Indicators Replay` and
`Trading in Bar Replay` gated higher. Scrub back through history and watch the
decision unfold at each step.

**This is the one genuine feature gap here.** `replay` in the dashboard today
is a *data fallback* — bundled prices replayed when DFlow is unreachable, plus
a `?replay` query param (`index.html` ~1249, ~1552, ~1564). There is no
scrubber, no seek, no play/pause over recorded history.

### The two cheap pro affordances

- **Export everywhere** — chart data export, export trades in CSV, export
  report in XLSX, screener data export. Present at nearly every paid tier;
  pros will not accept data they cannot get out. We have `export_learning()`
  internally but nothing user-facing.
- **Webhook notifications** — the escape hatch that makes alerts programmable.
  Maps directly onto the rail.

## Ranked for afterswap

1. **Rail replay scrubber** — highest value per unit of work. The receipts
   already exist (rail seq 120–127 sealed, proofs verified), and the engine is
   pure-tick and deterministic at ~1.2 µs/tick, so the whole history re-runs
   instantly and **exactly**. TradingView's Bar Replay is an approximation of
   what happened; ours would be a bit-exact re-execution against sealed,
   signed receipts — a strictly stronger claim than the incumbent's paid
   feature, and it turns the demo from "watch it run" into "interrogate what
   it did." Post-Sep-3.
2. **Per-tick execution fidelity** — continue 006/041. This is Gate 1 + Gate 2
   together and it is the expensive axis. ≥24 h recording → re-bench per the
   018 protocol → native two-plane prototype.
3. **Export the fills tape + race series as CSV/JSON** — hours of work, and it
   fits the project's "checkable, not asserted" ethos better than it fits
   TradingView's. A judge downloading the fills is the same move as a judge
   fetching a rail proof.
4. **Webhook on state transition / fill** — natural rail extension, but only
   once there is a consumer for it.

## Explicitly not worth copying

Quota inflation (multi-chart tabs, watchlists, portfolios, 1,000 alerts),
110+ drawing tools, screeners over 150+ exchanges, the social/community
surface (badges, public ideas, invite-only scripts). That is TradingView's
breadth business. afterswap is one pair, one question, and depth is the whole
proposition.

## Status

Analysis only — no code touched. Item 1 and 3 are candidates for the Demo Day
build order in `.plans/005_demo_day.md` §5; nothing here goes near the
deployed demo before the submission deadline.
