use reliquary_container::{Container, decode_publication};
use reliquary_owner_session::{
    GraphAdmission, RelHistoryRelation, RelSession, RelWriter, VerifiedOwnerCopyPlan,
    compare_rel_histories, prune_terminal_archive_copy,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};

const WRITER: &str = env!("CARGO_BIN_EXE_d01-synthetic-target-writer");
const TRANSFER: &[u8] = include_bytes!("archive-bindings-transfer.json");

fn workspace() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}

fn write(source: &Path, candidate: &Path, bytes: &[u8]) -> std::process::Output {
    fs::write(source, bytes).unwrap();
    Command::new(WRITER)
        .arg("--synthetic-archive-bindings")
        .arg(source)
        .arg(hex::encode(Sha256::digest(bytes)))
        .arg(candidate)
        .output()
        .unwrap()
}

#[test]
fn normal_reopen_preserves_order_clocks_and_nonapplying_repeats() {
    let workspace = workspace();
    let source = workspace.path().join("transfer.json");
    let candidate = workspace.path().join("candidate.rel");
    let result = write(&source, &candidate, TRANSFER);
    assert!(result.status.success(), "{result:?}");
    let receipt: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(receipt["current_only_owner_reopen"], true);
    assert_eq!(receipt["authentic_source_admission"], false);
    assert_eq!(receipt["archive_history"].as_array().unwrap().len(), 7);
    assert_eq!(fs::read(&source).unwrap(), TRANSFER);

    let reader = RelSession::open_path_current(&candidate).unwrap();
    let coordinates: Vec<_> = reader
        .archive_publication_bindings()
        .iter()
        .map(|binding| {
            (
                binding.archive_version,
                binding.global_version,
                binding.transaction_time_ns,
                binding.bound_reservation,
                binding.first_applying_archive_version,
                binding.first_applying_global_version,
            )
        })
        .collect();
    assert_eq!(
        coordinates,
        [
            (1, 2, Some(9_007_199_254_741_010), false, 1, 2),
            (2, 1, Some(-9_007_199_254_740_995), true, 2, 1),
            (3, 4, None, false, 3, 4),
            (4, 3, None, true, 1, 2),
            (5, 5, Some(9_007_199_254_741_030), false, 5, 5),
            (6, 6, Some(-9_007_199_254_741_050), false, 2, 1),
        ]
    );
    assert_eq!(reader.archive_version(), 6);
    assert_eq!(reader.rel_turn_count(), 2);
    assert_eq!(
        reader.conversation_summaries().unwrap()[0].leaf_node_id,
        "reply"
    );
    let container = Container::open_read_only(&candidate).unwrap();
    let frames = container.read_frames().unwrap();
    for binding in reader.archive_publication_bindings() {
        let frame = frames
            .iter()
            .find(|frame| frame.object == binding.object)
            .unwrap();
        let envelope = decode_publication(&frame.payload).unwrap().unwrap();
        assert_eq!(envelope.version, binding.global_version);
        assert_eq!(envelope.bound_reservation, binding.bound_reservation);
        assert_eq!(
            container.transaction_time_ns(binding.global_version),
            binding.transaction_time_ns
        );
    }
}

#[test]
fn equal_archive_heads_with_different_origins_and_times_diverge() {
    let workspace = workspace();
    let source = workspace.path().join("transfer.json");
    let left = workspace.path().join("left.rel");
    let right = workspace.path().join("right.rel");
    assert!(write(&source, &left, TRANSFER).status.success());
    let mut alternate: Value = serde_json::from_slice(TRANSFER).unwrap();
    alternate["events"][7]["clock"]["transaction_time_ns"] = json!("-9007199254741051");
    alternate["events"][7]["first_applying_archive_version"] = json!("1");
    let bytes = serde_json::to_vec(&alternate).unwrap();
    assert!(write(&source, &right, &bytes).status.success());

    let lhs = RelSession::open_path_current(&left).unwrap();
    let rhs = RelSession::open_path_current(&right).unwrap();
    let cut = lhs.current_historical_cut();
    assert_eq!(cut, rhs.current_historical_cut());
    assert_eq!(
        lhs.conversation_summaries().unwrap(),
        rhs.conversation_summaries().unwrap()
    );
    assert_ne!(
        lhs.historical_proof(lhs.owner_id(), &cut).unwrap().digest,
        rhs.historical_proof(rhs.owner_id(), &cut).unwrap().digest
    );
    assert_eq!(
        compare_rel_histories(&left, &right).unwrap().relation,
        RelHistoryRelation::Diverged
    );
}

#[test]
fn unsupported_clock_and_repeat_coordinates_leave_no_candidate() {
    let workspace = workspace();
    let source = workspace.path().join("transfer.json");
    let candidate = workspace.path().join("candidate.rel");
    let original: Value = serde_json::from_slice(TRANSFER).unwrap();
    let failures = [
        ("/events/1/clock/global_version", json!("1")),
        (
            "/events/2/clock/transaction_time_ns",
            json!("-9007199254740996"),
        ),
        ("/events/2/clock/bound_reservation", json!(false)),
        ("/events/4/revision/version", json!("4")),
        ("/events/5/first_applying_archive_version", json!("0")),
        ("/events/7/first_applying_archive_version", json!("4")),
        ("/events/7/clock/bound_reservation", json!(true)),
    ];
    for (field, value) in failures {
        let mut forged = original.clone();
        *forged.pointer_mut(field).unwrap() = value;
        let bytes = serde_json::to_vec(&forged).unwrap();
        let result = write(&source, &candidate, &bytes);
        assert_eq!(result.status.code(), Some(4), "{field}: {result:?}");
        assert!(result.stdout.is_empty());
        assert!(!candidate.exists());
        assert_eq!(fs::read(&source).unwrap(), bytes);
    }
}

#[test]
fn archive_prune_keeps_bindings_for_unavailable_turns_and_their_repeats() {
    let workspace = workspace();
    let source = workspace.path().join("transfer.json");
    let candidate = workspace.path().join("candidate.rel");
    let pruned = workspace.path().join("pruned.rel");
    let result = write(&source, &candidate, TRANSFER);
    assert!(result.status.success(), "{result:?}");
    let mut writer = RelWriter::open_path_current(&candidate).unwrap();
    assert!(
        writer
            .tombstone_conversation("synthetic-conversation")
            .unwrap()
    );
    drop(writer);

    let plan = VerifiedOwnerCopyPlan::prepare(&candidate).unwrap();
    let admission = GraphAdmission::for_replay([0x32; 16], [], [], [], [], []);
    let report = prune_terminal_archive_copy(
        &plan,
        &candidate,
        &pruned,
        &["synthetic-conversation".into()],
        &admission,
    )
    .unwrap();
    assert_eq!(report.pruned_turns, 2);
    let reopened = RelSession::open_path_current(&pruned).unwrap();
    assert_eq!(reopened.archive_version(), 7);
    assert_eq!(reopened.rel_turn_count(), 2);
    assert!(
        reopened
            .resolved_archive_turn("synthetic-conversation", "root")
            .is_err()
    );
    assert_eq!(
        reopened.archive_publication_bindings()[3].first_applying_global_version,
        2
    );
    assert_eq!(
        reopened.archive_publication_bindings()[1].transaction_time_ns,
        Some(-9_007_199_254_740_995)
    );
}
