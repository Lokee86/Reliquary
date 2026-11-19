use reliquary_memory_core::{MemoryId, PayloadState};
use reliquary_owner_session::PhySession;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};

const WRITER: &str = env!("CARGO_BIN_EXE_d01-synthetic-target-writer");
const TRANSFER: &[u8] = include_bytes!("phy-memory-transfer.json");
const MEMORY_ID: &str = "c30df370d053280ec83a3af8bbac9dbe638551448e14f9663510b1661ed84f5f";

fn workspace() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}

fn write(source: &Path, candidate: &Path, bytes: &[u8]) -> std::process::Output {
    fs::write(source, bytes).unwrap();
    Command::new(WRITER)
        .arg("--synthetic-memory-fixture")
        .arg(source)
        .arg(hex::encode(Sha256::digest(bytes)))
        .arg(candidate)
        .output()
        .unwrap()
}

#[test]
fn native_phy_reopens_both_memory_revisions_and_exact_clocks() {
    let workspace = workspace();
    let source = workspace.path().join("transfer.json");
    let candidate = workspace.path().join("candidate.phy");
    let result = write(&source, &candidate, TRANSFER);
    assert!(result.status.success(), "{result:?}");
    assert!(result.stderr.is_empty());
    let receipt: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(receipt["status"], "INCOMPLETE");
    assert_eq!(receipt["authentic_source_admission"], false);
    assert_eq!(receipt["current_only_owner_reopen"], true);
    let cuts = receipt["memory_history"].as_array().unwrap();
    assert_eq!(cuts.len(), 2);
    assert_eq!(cuts[0]["global_version"], "1");
    assert_eq!(cuts[0]["transaction_time_ns"], "9007199254741010");
    assert_eq!(cuts[1]["global_version"], "2");
    assert_eq!(cuts[1]["transaction_time_ns"], "9007199254741030");
    assert_eq!(fs::read(&source).unwrap(), TRANSFER);

    let reader = PhySession::open_path_current(&candidate).unwrap();
    assert_eq!(reader.owner_id(), [0x45; 16]);
    assert_eq!(reader.memory_version(), 2);
    let id = MemoryId(hex::decode(MEMORY_ID).unwrap().try_into().unwrap());
    assert!(reader.memory_history_at_cut(id, 0).unwrap().is_none());
    let first = reader.memory_history_at_cut(id, 1).unwrap().unwrap();
    let second = reader.memory_history_at_cut(id, 2).unwrap().unwrap();
    assert_eq!(first.revisions.len(), 1);
    assert_eq!(second.revisions.len(), 2);
    assert_eq!(first.revisions[0], second.revisions[0]);
    assert_eq!(first.revisions[0].record.category, "fact");
    assert!(!first.revisions[0].record.archived);
    let second_revision = &second.revisions[1];
    assert_eq!(second_revision.record.category, "decision");
    assert_eq!(second_revision.record.temporal_status, "historical");
    assert!(second_revision.record.archived);
    assert_eq!(
        second_revision.record.source_time_ns,
        Some(-9_007_199_254_740_995)
    );
    assert_eq!(second_revision.record.updated_at_ns, 9_007_199_254_741_021);
    assert!(
        matches!(&second_revision.payload, PayloadState::Available(body)
        if body.title == "Private fixture" && body.content == "Mémoire synthetic")
    );
    assert_eq!(first.revisions[0].payload, second_revision.payload);
    assert!(second.tombstone.is_none());
    drop(reader);

    let physical = reliquary_container::Container::open_read_only(&candidate).unwrap();
    assert_eq!(physical.latest_version(), 2);
    assert_eq!(physical.transaction_time_ns(1), Some(9_007_199_254_741_010));
    assert_eq!(physical.transaction_time_ns(2), Some(9_007_199_254_741_030));
    let before = fs::read(&candidate).unwrap();
    assert_eq!(write(&source, &candidate, TRANSFER).status.code(), Some(5));
    assert_eq!(fs::read(&candidate).unwrap(), before);
}

#[test]
fn native_phy_rejects_forged_revision_identity_and_clock_coordinates() {
    let workspace = workspace();
    let source = workspace.path().join("transfer.json");
    let candidate = workspace.path().join("candidate.phy");
    let original: Value = serde_json::from_slice(TRANSFER).unwrap();
    let failures = [
        ("/revisions/0/memory_id", json!("0".repeat(64))),
        ("/revisions/1/content", json!("different body")),
        ("/revisions/1/revision", json!("3")),
        ("/revisions/1/memory_version", json!("1")),
        ("/revisions/1/global_version", json!("1")),
        ("/revisions/1/global_version", json!("3")),
        (
            "/revisions/1/transaction_time_ns",
            json!(9007199254741030_i64),
        ),
        ("/revisions/1/mutation_id", json!("phy-create-v1")),
    ];
    for (field, replacement) in failures {
        let mut changed = original.clone();
        *changed.pointer_mut(field).unwrap() = replacement;
        let bytes = serde_json::to_vec(&changed).unwrap();
        let result = write(&source, &candidate, &bytes);
        assert_eq!(result.status.code(), Some(4), "{field}: {result:?}");
        assert!(result.stdout.is_empty());
        assert!(!candidate.exists());
        assert_eq!(fs::read(&source).unwrap(), bytes);
        assert_eq!(workspace.path().read_dir().unwrap().count(), 1);
    }
}
