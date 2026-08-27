//! Cross-implementation check: a proof served by the Worker's wasm must
//! verify under the native build of the same crate. If these two ever
//! disagree, the shared-bytes discipline has failed somewhere visible.

use afterswap_rail::{AuditRecord, merkle_verify, record_hash};

fn hex32(s: &str) -> Option<[u8; 32]> {
    (s.len() == 64).then(|| {
        let mut out = [0u8; 32];
        for (i, c) in s.as_bytes().chunks(2).enumerate() {
            out[i] = u8::from_str_radix(std::str::from_utf8(c).unwrap_or("0"), 16).unwrap_or(0);
        }
        out
    })
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: check_proof <FILE>");
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
    let record: AuditRecord = serde_json::from_value(v["record"].clone()).expect("record");
    let root = hex32(v["proof"]["segment_root"].as_str().expect("root")).expect("root hex");
    let served: [u8; 32] = hex32(v["proof"]["record_hash"].as_str().expect("hash")).expect("hex");

    let native = record_hash(&record);
    assert_eq!(native, served, "wasm and native record_hash disagree");

    let steps: Vec<([u8; 32], bool)> = v["proof"]["proof"]
        .as_array()
        .expect("steps")
        .iter()
        .map(|s| (hex32(s[0].as_str().expect("sib")).expect("hex"), s[1].as_bool().expect("side")))
        .collect();
    match merkle_verify(&native, &steps, &root) {
        true => println!("proof(native) : VERIFIED — wasm-served proof checks under the native crate"),
        false => {
            eprintln!("proof(native) : FAILED");
            std::process::exit(1);
        }
    }
}
