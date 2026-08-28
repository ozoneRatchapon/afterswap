//! Guards on the production-ingest shipper that must not regress silently.

use afterswap_server::rail_ship::check_scheme;

/// Plaintext to a remote host lets a network position drop records
/// indistinguishably from an outage, so only loopback is exempt.
#[test]
fn https_required_for_remote_hosts() {
    assert!(check_scheme("https://rail.example.workers.dev").is_ok());
    assert!(check_scheme("http://localhost:8787").is_ok());
    assert!(check_scheme("http://127.0.0.1:8787").is_ok());
    assert!(check_scheme("http://rail.example.workers.dev").is_err());
    assert!(check_scheme("rail.example.workers.dev").is_err());
}
