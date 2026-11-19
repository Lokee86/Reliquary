use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, process::Command};

#[test]
fn authentic_writer_refuses_unrelocated_generation_and_destination_collision() {
    let scratch = tempfile::tempdir().unwrap();
    let transfer = scratch.path().join("transfer.jsonl");
    let destination = scratch.path().join("candidate.rel");
    let rows = [
        json!({
            "schema_id": "reliquary-d01-authentic-native-frames-v1",
            "source_sha256": "11".repeat(32), "source_frames": "2",
            "file_kind": "rel", "owner_uuid": "22".repeat(16),
            "scope": "project", "global_head": "1",
        }),
        json!({
            "kind": "reservation", "source_ordinal": "0", "global_version": "1",
            "transaction_time_ns": null,
        }),
        json!({
            "kind": "vector_receipt", "source_ordinal": "1", "global_version": "1",
            "vector_version": "1", "generation_source_ordinal": "0",
        }),
    ];
    let body = rows
        .iter()
        .map(|row| format!("{row}\n"))
        .collect::<String>();
    fs::write(&transfer, &body).unwrap();
    let digest = hex::encode(Sha256::digest(body.as_bytes()));
    let invoke = || {
        Command::new(env!("CARGO_BIN_EXE_d01-synthetic-target-writer"))
            .arg("--authentic-frames")
            .arg(&transfer)
            .arg(&digest)
            .arg(&destination)
            .output()
            .unwrap()
    };

    let missing = invoke();
    assert_eq!(missing.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("unrelocated"));
    assert!(!destination.exists());

    fs::write(&destination, "existing owner").unwrap();
    let collision = invoke();
    assert_eq!(collision.status.code(), Some(5));
    assert_eq!(fs::read(&destination).unwrap(), b"existing owner");
}
