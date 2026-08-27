# Sensitivity of the constants we chose by intuition

Objective: mean(edge vs TWAP, edge vs trailing) over 6 corpora. One constant varied at a time from the shipped default (**bold**). A flat sweep means the number was never a knob; a steep one means it needs its own evidence.

- **peak_drop_bps (off-peak input bit)** — spread **1 bps**
  - 10: +43 · 20: +44 · 30: +43 · 50: +43 · 100: +43
- **surprise_ratio (forced re-tournament)** — spread **0 bps**
  - 0.0: +43 · 0.8: +43 · 1.2: +43 · 2.0: +43 · 4.0: +43
- **tranche_frac (clip size)** — spread **3 bps**
  - 5%: +41 · 10%: +43 · 20%: +43 · 34%: +44 · 50%: +45
- **window_len (evaluation window)** — spread **34 bps**
  - 6: +42 · 12: +43 · 24: +41 · 48: +10
- **refresh_every_windows (re-tournament cadence)** — spread **2 bps**
  - 1: +43 · 2: +43 · 4: +45 · 8: +45
- **max_arms (population cap)** — spread **2 bps**
  - 8: +42 · 16: +43 · 24: +43 · 48: +42

Read the spreads, not the individual numbers: a constant with a large spread is a
result that depends on a choice nobody validated, and every such choice in this table
was made by intuition before this bench existed.

