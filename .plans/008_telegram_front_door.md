# Plan 008 — Telegram front door (accessibility)

## Why

Deadline verified on the rules page (2026-08-31): **23:59 ICT on Sep 2**, not
Aug 31. Judging is on *usefulness, clarity, execution, originality* — explicitly
"over technical perfection".

Against those four, the entry's weak axis is **reach**, not engineering. Today a
user must open a dashboard and read an FSM state diagram, a bandit leaderboard
and a gate meter to get value. The machinery is done; the front door is not.

Telegram is the cheapest surface that removes every install step for the actual
audience (Superteam TH lives there). Watch-only mode needs no wallet, no key,
no signature — so the demo is "send two words, get plain-language exit calls".

## Scope

New crate `afterswap-bot`. Nothing in the engine, dflow, rail, policy or
worker crates changes — the bot is another consumer of the same
`EngineEvent` stream `paper.rs` already drives.

- `phrase.rs` — `EngineEvent` → one plain sentence. Pure, no IO. **This is the
  product change**; everything else is plumbing.
- `session.rs` — command parsing (`/watch`, `/status`, `/stop`, `/proof`,
  `/help`). Pure.
- `telegram.rs` — minimal Bot API client (long-poll `getUpdates`, `sendMessage`)
  behind a `Sink` trait so the whole loop runs and is demoable with **no bot
  token** (`--dry-run` prints to stdout).
- `watcher.rs` — per-chat task: `PricePoller` → `ExitEngine::on_tick` →
  phrases → sink.

## Rules inherited

1. No claim without a floor and a standard error — the bot reports the same
   numbers the dashboard does, including when holding wins. It must not
   acquire a rosier voice than the evidence.
2. Never grow the model to chase an edge. The bot adds **zero** engine
   behaviour; identical config, identical events.
3. Plain language is not spin: "sold 10%" is allowed, "profit" is not.

## Out of scope

Registering the bot with BotFather and running it against a live token is a
user action (requires their Telegram account). Everything up to that is built
and tested here.

## Tasks

- [x] `afterswap-bot` crate skeleton + workspace member
- [x] `phrase.rs` + unit tests (every `EngineEvent` variant renders)
- [x] `session.rs` + unit tests (command grammar, bad input)
- [x] `telegram.rs` (`Sink` trait, stdout sink, HTTP sink)
- [x] `watcher.rs` live loop, `main.rs` bin with `--dry-run`
- [x] README hero + `docs/SUBMISSION.md` reframe (lead with what the product
      *does*; keep the retraction, move it to the proof section)
- [x] Date correction: build window is Aug 21 – Sep 2

## Result (2026-08-31)

Shipped and verified end to end against the live DFlow dev API. Real transcript
from `--dry-run --interval-ms 1000`:

    Watching 1 SOL from 102.552950.
    saw a dip -> sell-state S1 -> sold 10% of your SOL at 102.573590. 90% left.
    ...
    Position fully exited. The plan finished at -0.1 bps versus your entry price.

25 tests in the new crate, workspace suite green, clippy clean on the touched
crate.

Two defects the live run caught that the unit tests had not:

1. `/status` immediately after an exit replied "Not watching anything" —
   the engine clears the position on close, so the view had nothing to read.
   A finished watch is not an absent watch; `StatusView.finished` now carries
   the final value and `/status` reports the completed run. Regression test
   added.
2. A wrapped format string had lost its `\` continuation, so source
   indentation appeared as a gap in the chat. Invisible in review, obvious to
   a user. Guard test added over the whole message corpus.

Both are the same lesson the project already has in a different domain: the
instrument has to be exercised on the real path before it is trusted. Unit
tests over `phrase()` could not have found either, because neither is a
property of a single event.

## Not done, and why

Registering the bot with @BotFather and running it against a live token needs
the user's Telegram account. The `--dry-run` path exists precisely so this is
the only remaining step, and so a judge who has no token can still reproduce
the transcript.
