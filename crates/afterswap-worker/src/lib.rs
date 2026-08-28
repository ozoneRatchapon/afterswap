//! Pure-Rust rail Worker — the R2/R3 endpoints with the TypeScript layer
//! removed.
//!
//! The whole point of this crate over `worker/rail.ts`: the Sequencer DO
//! links `afterswap-rail` as a plain dependency and calls `verify_record`,
//! `record_hash`, `merkle_root` as typed functions. The JSON-string interface
//! into a side-loaded wasm module — the boundary where a full-range u64 got
//! mangled by JavaScript's number type — does not exist here to get wrong.
//!
//! Free-tier invariant (the deployment constraint this crate is shaped by):
//! Workers Free + SQLite Durable Objects (this repo's Scoreboard DO already
//! runs on that plan), R2 within its own free tier (10 GB storage, 1M
//! class-A / 10M class-B per month) — and the R2 binding is *optional*: with
//! no bucket configured, closed segments are retained in the DO's SQLite
//! instead of trimmed, which at ~4 KB/record holds >1M records inside the
//! free 5 GB. Anchoring targets devnet (airdropped fees). Nothing here
//! assumes a paid plan.

mod sequencer;

pub use sequencer::RailSequencer;

use worker::*;

#[event(start)]
fn start() {
    // Without this, a wasm panic surfaces as an opaque "Critical error";
    // with it, the panic message and location reach the console.
    console_error_panic_hook::set_once();
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let path = req.path();
    if req.method() == Method::Options {
        let headers = Headers::new();
        headers.set("access-control-allow-origin", "*")?;
        headers.set("access-control-allow-methods", "GET, POST, OPTIONS")?;
        headers.set("access-control-allow-headers", "content-type")?;
        return Ok(Response::empty()?.with_headers(headers));
    }
    match path.starts_with("/rail/") {
        true => {
            // One object per deployment: a single chain, a single writer.
            // `rail-prod-v1` supersedes `rail-v1`: the attestation key was
            // rotated off the public dev seed, and the verifier checks every
            // record against the single current pubkey, so a dev-attested
            // prefix would read as verification failures. A fresh instance
            // starts the production chain at seq 0 under the production key.
            let ns = env.durable_object("RAIL")?;
            let stub = ns.id_from_name("rail-prod-v1")?.get_stub()?;
            stub.fetch_with_request(req).await
        }
        false => Response::error("rail worker: unknown route", 404),
    }
}
