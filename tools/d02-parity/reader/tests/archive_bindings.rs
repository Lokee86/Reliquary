use d02_current_reader::{CandidateKind, observe_archive_binding_cut};
use reliquary_archive::{Branch, TurnInput};
use reliquary_container::Container;
use reliquary_owner_session::{OwnerScope, RelWriter};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
use tempfile::tempdir;

const OWNER: [u8; 16] = [0x36; 16];
const RESERVED_TIME: i64 = -9_007_199_254_740_995;

fn raw_archive(path: &Path) {
    let mut writer = RelWriter::create_current_path(path, OwnerScope::Project, OWNER).unwrap();
    for (id, parent, role, time) in [
        ("root", None, "user", 9_007_199_254_740_993),
        ("reply", Some("root"), "assistant", 9_007_199_254_741_009),
    ] {
        writer
            .accept_turn_and_advance_branch(
                TurnInput {
                    id: id.into(),
                    conversation_id: "archive-binding-fixture".into(),
                    parent_id: parent.map(str::to_owned),
                    role: role.into(),
                    principal_id: None,
                    timestamp_ns: time,
                    content: format!("body {id}"),
                    attachments: vec![],
                    project_attachments: vec![],
                },
                Branch {
                    id: "main".into(),
                    conversation_id: "archive-binding-fixture".into(),
                    leaf_node_id: id.into(),
                    canonical: true,
                },
            )
            .unwrap();
    }
}

fn applying(local: u64, record: &[u8]) -> Vec<u8> {
    let mut body = b"RELAPB01".to_vec();
    body.extend(local.to_le_bytes());
    body.push(0);
    body.extend(record);
    body
}

fn repeat(local: u64, first: u64) -> Vec<u8> {
    let mut body = b"RELAPB01".to_vec();
    body.extend(local.to_le_bytes());
    body.push(1);
    body.extend(first.to_le_bytes());
    body
}

#[derive(Clone, Copy)]
enum Change {
    None,
    ReservedTime,
    GlobalBinding,
    ReservationMode,
    RepeatOrigin,
    RepeatOfRepeat,
}

fn bound_archive(raw: &Path, target: &Path, change: Change) {
    let original = Container::open_read_only(raw).unwrap();
    let mut copy = Container::create(target, original.identity(), original.owner_id()).unwrap();
    let mut records = Vec::new();
    for frame in original.read_frames().unwrap() {
        if matches!(frame.payload.get(..8), Some(b"RELTURN2" | b"RELBRCH1")) {
            records.push(frame.payload);
        } else {
            copy.append(&frame.payload).unwrap();
        }
    }
    assert_eq!(records.len(), 4);

    let reserved_time = if matches!(change, Change::ReservedTime) {
        RESERVED_TIME - 1
    } else {
        RESERVED_TIME
    };
    let swap = matches!(change, Change::GlobalBinding);
    let (first_time, third_time, branch_global, repeat_global) = if swap {
        (None, Some(reserved_time), 3, 1)
    } else {
        (Some(reserved_time), None, 1, 3)
    };
    assert_eq!(copy.reserve_version(first_time).unwrap(), 1);
    copy.append_publication_envelope(&applying(1, &records[0]), Some(9_007_199_254_741_010))
        .unwrap();
    if swap {
        assert_eq!(copy.reserve_version(third_time).unwrap(), 3);
    }
    copy.append_bound_publication_envelope(branch_global, &applying(2, &records[1]))
        .unwrap();
    if !swap {
        assert_eq!(copy.reserve_version(third_time).unwrap(), 3);
    }
    copy.append_publication_envelope(&applying(3, &records[2]), None)
        .unwrap();
    copy.append_bound_publication_envelope(repeat_global, &repeat(4, 1))
        .unwrap();

    let branch = applying(5, &records[3]);
    if matches!(change, Change::ReservationMode) {
        assert_eq!(copy.reserve_version(Some(15)).unwrap(), 5);
        copy.append_bound_publication_envelope(5, &branch).unwrap();
    } else {
        copy.append_publication_envelope(&branch, Some(15)).unwrap();
    }
    let first = match change {
        Change::RepeatOrigin => 5,
        Change::RepeatOfRepeat => 4,
        _ => 2,
    };
    copy.append_publication_envelope(&repeat(6, first), Some(-19))
        .unwrap();
    copy.sync().unwrap();
}

fn fresh_process(path: &Path, cut: u64) -> Value {
    let result = Command::new(env!("CARGO_BIN_EXE_d02-current-reader"))
        .args(["--kind", "rel", "--candidate"])
        .arg(path)
        .args(["--archive-binding-cut", &cut.to_string()])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stderr.is_empty(), "{:?}", result.stderr);
    serde_json::from_slice(&result.stdout).unwrap()
}

#[test]
fn observes_exact_archive_and_global_clocks_without_reapplying_repeats() {
    let directory = tempdir().unwrap();
    let raw = directory.path().join("raw.rel");
    let bound = directory.path().join("bound.rel");
    raw_archive(&raw);
    bound_archive(&raw, &bound, Change::None);
    let before = fs::read(&bound).unwrap();

    let genesis = fresh_process(&bound, 0);
    assert_eq!(genesis["binding_projection"]["bindings"], json!([]));
    let cut = fresh_process(&bound, 6);
    assert_eq!(cut, fresh_process(&bound, 6));
    assert_eq!(cut["status"], "INCOMPLETE");
    assert_eq!(cut["archive_binding_observation_status"], "OBSERVED");
    assert_eq!(cut["archive_head"], "6");
    assert_eq!(cut["candidate_unchanged"], true);
    assert_eq!(
        cut["candidate_sha256_before"],
        cut["candidate_sha256_after"]
    );
    let bindings = cut["binding_projection"]["bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 6);
    assert_eq!(bindings[0]["global_version"], "2");
    assert_eq!(bindings[0]["transaction_time_ns"], "9007199254741010");
    assert_eq!(bindings[1]["global_version"], "1");
    assert_eq!(bindings[1]["transaction_time_ns"], "-9007199254740995");
    assert_eq!(bindings[1]["clock_mode"], "bind_reserved");
    assert_eq!(bindings[2]["global_version"], "4");
    assert_eq!(bindings[2]["transaction_time_ns"], Value::Null);
    assert_eq!(bindings[3]["global_version"], "3");
    assert_eq!(bindings[3]["transaction_time_ns"], Value::Null);
    assert_eq!(bindings[3]["first_applying_archive_version"], "1");
    assert_eq!(bindings[3]["first_applying_global_version"], "2");
    assert_eq!(bindings[5]["first_applying_archive_version"], "2");
    assert_eq!(bindings[5]["first_applying_global_version"], "1");
    assert_eq!(cut["projection"]["activity_cut"], "2");
    assert_eq!(cut["projection"]["branches"][0]["leaf_node_id"], "reply");
    assert_eq!(
        cut["projection"]["branch_revisions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        fresh_process(&bound, 2)["binding_projection"]["bindings"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(fs::read(&bound).unwrap(), before);
}

#[test]
fn detects_same_head_divergent_intermediate_bindings() {
    let directory = tempdir().unwrap();
    let raw = directory.path().join("raw.rel");
    let baseline = directory.path().join("expected.rel");
    raw_archive(&raw);
    bound_archive(&raw, &baseline, Change::None);
    let expected = fresh_process(&baseline, 6);

    for (name, change, first_cut) in [
        ("time", Change::ReservedTime, 2),
        ("global", Change::GlobalBinding, 2),
        ("mode", Change::ReservationMode, 5),
        ("origin", Change::RepeatOrigin, 6),
    ] {
        let changed = directory.path().join(format!("{name}.rel"));
        bound_archive(&raw, &changed, change);
        let head = fresh_process(&changed, 6);
        assert_eq!(head["archive_binding_observation_status"], "OBSERVED");
        assert_eq!(head["projection"], expected["projection"]);
        assert_eq!(head["projection_sha256"], expected["projection_sha256"]);
        assert_ne!(
            head["binding_projection_sha256"],
            expected["binding_projection_sha256"]
        );
        assert_eq!(
            fresh_process(&changed, first_cut - 1)["binding_projection_sha256"],
            fresh_process(&baseline, first_cut - 1)["binding_projection_sha256"],
        );
        assert_ne!(
            fresh_process(&changed, first_cut)["binding_projection_sha256"],
            fresh_process(&baseline, first_cut)["binding_projection_sha256"],
        );
    }
}

#[test]
fn unclocked_and_repeat_of_repeat_candidates_fail_closed() {
    let directory = tempdir().unwrap();
    let raw = directory.path().join("raw.rel");
    let corrupt = directory.path().join("corrupt.rel");
    raw_archive(&raw);
    let unclocked = observe_archive_binding_cut(CandidateKind::Rel, &raw, 1);
    assert_eq!(
        unclocked["archive_binding_observation_status"],
        "INCOMPLETE"
    );
    assert_eq!(unclocked["binding_projection"], Value::Null);
    assert_eq!(unclocked["reason"], "archive_binding_cut_incomplete");

    bound_archive(&raw, &corrupt, Change::RepeatOfRepeat);
    let rejected = observe_archive_binding_cut(CandidateKind::Rel, &corrupt, 6);
    assert_eq!(rejected["archive_binding_observation_status"], "INCOMPLETE");
    assert_eq!(rejected["binding_projection"], Value::Null);
}
