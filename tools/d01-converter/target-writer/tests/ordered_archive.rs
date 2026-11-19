use reliquary_owner_session::{OwnerScope, RelSession};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};

const WRITER: &str = env!("CARGO_BIN_EXE_d01-synthetic-target-writer");
const TRANSFER: &[u8] = include_bytes!("ordered-archive-transfer.json");

fn workspace() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}

fn write(source: &Path, candidate: &Path, digest: &str) -> std::process::Output {
    Command::new(WRITER)
        .arg("--synthetic-fixture")
        .arg(source)
        .arg(digest)
        .arg(candidate)
        .output()
        .unwrap()
}

#[test]
fn writes_native_rel_and_retains_archive_cuts_instead_of_just_the_head() {
    let workspace = workspace();
    let source = workspace.path().join("source.json");
    let candidate = workspace.path().join("candidate.rel");
    fs::write(&source, TRANSFER).unwrap();
    let digest = hex::encode(Sha256::digest(TRANSFER));

    let written = write(&source, &candidate, &digest);
    assert!(written.status.success(), "{:?}", written);
    assert!(written.stderr.is_empty());
    let receipt: Value = serde_json::from_slice(&written.stdout).unwrap();
    assert_eq!(receipt["status"], "INCOMPLETE");
    assert_eq!(receipt["authentic_source_admission"], false);
    assert_eq!(receipt["current_only_owner_reopen"], true);
    assert_eq!(receipt["source_sha256_before"], digest);
    assert_eq!(receipt["source_sha256_after"], digest);
    let history = receipt["archive_history"].as_array().unwrap();
    assert_eq!(history.len(), 5);
    for (version, cut) in history.iter().enumerate() {
        assert_eq!(cut["archive_version"], version.to_string());
        assert_eq!(cut["retained_records"], version.to_string());
        assert_eq!(cut["proof_sha256"].as_str().unwrap().len(), 64);
    }

    let reader = RelSession::open_path_current(&candidate).unwrap();
    assert_eq!(reader.owner_id(), [0x32; 16]);
    assert_eq!(reader.scope(), OwnerScope::Project);
    let node = reader
        .archive_node("synthetic-conversation", "root")
        .unwrap();
    assert_eq!(node.timestamp_ns, 9_007_199_254_740_993);
    assert_eq!(node.principal_id.as_deref(), Some("fixture-principal"));
    assert_eq!(node.content_id.0, <[u8; 32]>::from(Sha256::digest("café")));
    let reply = reader
        .archive_node("synthetic-conversation", "reply")
        .unwrap()
        .clone();
    assert_eq!(reply.parent_id.as_deref(), Some("root"));
    let mut at_two = reader.current_historical_cut();
    at_two.archive_version = 2;
    let proof = reader.historical_proof(reader.owner_id(), &at_two).unwrap();
    assert_eq!(proof.retained_records, 2);
    assert_eq!(history[2]["proof_sha256"], hex::encode(proof.digest));
    drop(reader);
    let first_candidate = fs::read(&candidate).unwrap();
    assert_eq!(
        receipt["candidate_sha256"],
        hex::encode(Sha256::digest(&first_candidate))
    );

    let mut alternate: Value = serde_json::from_slice(TRANSFER).unwrap();
    alternate["revisions"].as_array_mut().unwrap().swap(1, 2);
    alternate["revisions"][1]["version"] = json!("2");
    alternate["revisions"][2]["version"] = json!("3");
    let alternate_source = workspace.path().join("alternate.json");
    let alternate_candidate = workspace.path().join("alternate.rel");
    let alternate_bytes = serde_json::to_vec(&alternate).unwrap();
    fs::write(&alternate_source, &alternate_bytes).unwrap();
    let alternate_digest = hex::encode(Sha256::digest(alternate_bytes));
    let written = write(&alternate_source, &alternate_candidate, &alternate_digest);
    assert!(written.status.success(), "{:?}", written);
    let alternate_receipt: Value = serde_json::from_slice(&written.stdout).unwrap();
    assert_ne!(
        history[2]["proof_sha256"],
        alternate_receipt["archive_history"][2]["proof_sha256"]
    );
    let alternate_reader = RelSession::open_path_current(&alternate_candidate).unwrap();
    assert_eq!(
        alternate_reader.archive_node("synthetic-conversation", "reply"),
        Some(&reply)
    );

    let collision = write(&source, &candidate, &digest);
    assert_eq!(collision.status.code(), Some(5));
    assert_eq!(fs::read(&candidate).unwrap(), first_candidate);
    assert_eq!(fs::read(&source).unwrap(), TRANSFER);
}

#[test]
fn rejects_wrong_hash_and_conflicting_history_without_publishing_candidates() {
    let workspace = workspace();
    let source = workspace.path().join("source.json");
    let candidate = workspace.path().join("candidate.rel");
    fs::write(&source, TRANSFER).unwrap();
    assert_eq!(
        write(&source, &candidate, &"0".repeat(64)).status.code(),
        Some(3)
    );
    assert!(!candidate.exists());

    let original: Value = serde_json::from_slice(TRANSFER).unwrap();
    let invalid = [
        ("history_generation", json!("0".repeat(64))),
        ("revisions/1/version", json!("3")),
        ("revisions/1/parent", json!("another-owner")),
        ("revisions/0/content_id", json!("0".repeat(64))),
        ("revisions/0/timestamp_ns", json!("09007199254740993")),
        ("revisions/2/leaf", json!("future-or-missing-node")),
        ("revisions/3/leaf", json!("root")),
        ("revisions/0/family", json!("real_frs07")),
    ];
    for (pointer, replacement) in invalid {
        let mut value = original.clone();
        *value.pointer_mut(&format!("/{pointer}")).unwrap() = replacement;
        let bytes = serde_json::to_vec(&value).unwrap();
        fs::write(&source, &bytes).unwrap();
        let result = write(&source, &candidate, &hex::encode(Sha256::digest(&bytes)));
        assert_eq!(result.status.code(), Some(4), "{pointer}: {result:?}");
        assert!(result.stdout.is_empty());
        assert!(!candidate.exists());
        assert_eq!(fs::read(&source).unwrap(), bytes);
        assert_eq!(workspace.path().read_dir().unwrap().count(), 1);
    }

    let mut missing = original.clone();
    missing["revisions"][0]
        .as_object_mut()
        .unwrap()
        .remove("parent");
    let bytes = serde_json::to_vec(&missing).unwrap();
    fs::write(&source, &bytes).unwrap();
    assert_eq!(
        write(&source, &candidate, &hex::encode(Sha256::digest(&bytes)))
            .status
            .code(),
        Some(4)
    );
    assert!(!candidate.exists());
}

#[test]
fn refuses_different_global_bindings_with_identical_local_archive_history() {
    let workspace = workspace();
    let source = workspace.path().join("source.json");
    let candidate = workspace.path().join("candidate.rel");
    let local_history: Value = serde_json::from_slice(TRANSFER).unwrap();
    for global_version in ["3", "7"] {
        let mut transfer = local_history.clone();
        transfer["revisions"][0]["global_version"] = json!(global_version);
        let bytes = serde_json::to_vec(&transfer).unwrap();
        fs::write(&source, &bytes).unwrap();
        let written = write(&source, &candidate, &hex::encode(Sha256::digest(&bytes)));
        assert_eq!(written.status.code(), Some(4), "{written:?}");
        assert!(written.stdout.is_empty());
        assert!(!candidate.exists());
        assert_eq!(fs::read(&source).unwrap(), bytes);
    }
}
