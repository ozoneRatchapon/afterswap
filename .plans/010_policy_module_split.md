# 010 — Split the policy program into per-instruction modules

Status: **done, committed** in `692b4fd` (not pushed). 60 policy tests green,
workspace green, `cargo clippy --workspace --all-targets` clean.

## Why

`crates/afterswap-policy/src/lib.rs` had reached 1,314 lines — past the
1,024-line ceiling in the global agent rules, and past the point where the
dispatch, the byte layouts and seven handlers could be read independently.
Every session that hardened a gate grew it further, so this was going to keep
getting worse.

## What moved

Pure relocation. No handler logic was edited: each function moved with its
doc comment intact, and the only edits were visibility (`fn` →
`pub(crate) fn` for the seven handlers and the two token helpers; `const` →
`pub(crate) const` for the private layout offsets) and per-module `use`
lists.

| file | lines | contents |
| --- | --- | --- |
| `lib.rs` | 105 | crate docs, `mod` declarations, `pub use types::*`, entrypoint, tag dispatch |
| `types.rs` | 150 | PDA seeds, account layouts + offsets, instruction tags + lengths, pinned program ids |
| `token.rs` | 89 | `require_vault_ata`, `invoke_transfer_checked` — shared by deposit, sell and close |
| `policy.rs` | 72 | tag 0 `CommitPolicy` |
| `execution.rs` | 211 | tags 1/3 `AuthorizeExecution`, `RevokeAuthorization` |
| `vault.rs` | 267 | tags 4/5 `DepositToVault`, `CloseVault` |
| `sell.rs` | 318 | tag 2 `ValidateAndSell` — the critical path, alone in its own file |
| `memo.rs` | 187 | tag 6 `AnchorFill` + the no-alloc `push_hex` / `push_dec` / `render_fill_memo` renderer |

The public API is unchanged: `pub use types::*` re-exports every `pub const`
the integration tests import (`POLICY_SEED`, `IX_VALIDATE_SELL`,
`VALIDATE_SELL_IX_LEN`, …), and `process_instruction` stays at the crate
root. Not one line of `tests/` needed changing — which is the evidence that
the split was behaviour-preserving.

## Cost

The SBF binary went 46,936 → **47,088 bytes** (+152). The likely cause is
codegen-unit partitioning: SBF release builds do not use `codegen-units = 1`,
and CGUs follow module boundaries, so cross-module inlining decisions shifted
slightly. Still far inside the 60 KB budget. If it ever matters, setting
`codegen-units = 1` for the release profile should erase it and then some.

## Verification

- `cargo-build-sbf --manifest-path crates/afterswap-policy/Cargo.toml` before
  every test run (a `cargo test` alone does **not** rebuild the `.so` — see
  the memory note; a green suite can be testing stale program code)
- `cargo test -p afterswap-policy` → 60 passing (0+8+9+2+18+23), unchanged
- `cargo test --workspace` → 45 result lines, no failures
- `cargo clippy --workspace --all-targets` → zero warnings

No `cargo fmt` was run: HEAD is deliberately not rustfmt-clean (the hand
wrapping in the doc comments is load-bearing and a previous fmt pass was
reverted on instruction). The new files preserve the original wrapping
verbatim.

## Not done

`.plans/009_gate_hardening.md` still describes the program as a single
`lib.rs` in a couple of places. Left alone rather than rewritten, because 009
is a record of what was found and fixed, not a live map of the file tree.
