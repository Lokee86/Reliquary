use d02_current_reader::{CandidateKind, observe_archive_cut};
use reliquary_archive::{AttachmentInput, Branch, EpisodeBoundary, EpisodeOrigin, TurnInput};
use reliquary_owner_session::{
    GraphAdmission, OwnerScope, RelWriter, VerifiedOwnerCopyPlan, prune_terminal_archive_copy,
};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
use tempfile::tempdir;

const OWNER: [u8; 16] = [0x32; 16];
const BODY: &str = "archival caf\u{e9}\r\n\u{1f9ea}";
const BODY_HASH: &str = "bb2be724c0afc604b2d117f8167dbf78cf35bad9a39adda11fe923e8641fe7f4";
const ATTACHMENT_HASH: &str = "712450d3c4a79eea9509e75dc1dacdeff58034df538536cfae2da882bd8a0c50";
const FIRST_NS: i64 = 9_007_199_254_740_993;

fn accept(writer: &mut RelWriter, id: &str, parent: Option<&str>, body: &str, ns: i64) {
    writer
        .accept_turn_and_advance_branch(
            TurnInput {
                id: id.into(),
                conversation_id: "synthetic-conversation".into(),
                parent_id: parent.map(str::to_owned),
                role: if parent.is_none() {
                    "user"
                } else {
                    "assistant"
                }
                .into(),
                principal_id: parent.is_none().then(|| "fixture-principal".into()),
                timestamp_ns: ns,
                content: body.into(),
                attachments: if parent.is_none() {
                    vec![AttachmentInput {
                        filename: "fixture.bin".into(),
                        mime_type: Some("application/octet-stream".into()),
                        bytes: vec![0, 255, 10],
                    }]
                } else {
                    vec![]
                },
                project_attachments: vec![],
            },
            Branch {
                id: "main".into(),
                conversation_id: "synthetic-conversation".into(),
                leaf_node_id: id.into(),
                canonical: true,
            },
        )
        .unwrap();
}

fn build(path: &Path, middle: &str, terminal: bool) {
    let mut writer = RelWriter::create_current_path(path, OwnerScope::Project, OWNER).unwrap();
    accept(&mut writer, "first", None, BODY, FIRST_NS);
    accept(&mut writer, "middle", Some("first"), middle, FIRST_NS + 1);
    accept(
        &mut writer,
        "head",
        Some("middle"),
        "same final body",
        FIRST_NS + 2,
    );
    if terminal {
        writer
            .tombstone_conversation("synthetic-conversation")
            .unwrap();
    } else {
        writer
            .finalize_episode(
                "synthetic-conversation",
                "first",
                "head",
                EpisodeOrigin::Live,
                EpisodeBoundary::Explicit,
                FIRST_NS + 3,
            )
            .unwrap();
    }
}

fn fresh_process(path: &Path, version: u64) -> Value {
    let result = Command::new(env!("CARGO_BIN_EXE_d02-current-reader"))
        .args(["--kind", "rel", "--candidate"])
        .arg(path)
        .args(["--archive-cut", &version.to_string()])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stderr.is_empty(), "{:?}", result.stderr);
    serde_json::from_slice(&result.stdout).unwrap()
}

#[test]
fn independent_archive_cut_preserves_exact_payload_evidence_and_order() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("archive.rel");
    build(&path, "original intermediate body", false);
    let before = fs::read(&path).unwrap();

    let genesis = fresh_process(&path, 0);
    assert_eq!(genesis["projection"]["turns"], json!([]));
    let before_branch = fresh_process(&path, 1);
    assert_eq!(
        before_branch["projection"]["turns"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(before_branch["projection"]["branches"], json!([]));

    let early = fresh_process(&path, 2);
    assert_eq!(early, fresh_process(&path, 2));
    assert_eq!(early["status"], "INCOMPLETE");
    assert_eq!(early["archive_observation_status"], "OBSERVED");
    assert_eq!(early["candidate_unchanged"], true);
    assert_eq!(
        early["candidate_sha256_before"],
        early["candidate_sha256_after"]
    );
    assert_eq!(
        early["owner"]["owner_uuid"],
        "32323232323232323232323232323232"
    );
    assert_eq!(early["owner"]["scope"], "project");
    assert_eq!(early["archive_head"], "7");
    assert_eq!(early["projection"]["archive_version"], "2");
    assert_eq!(early["projection"]["activity_cut"], "1");
    let first = &early["projection"]["turns"][0];
    assert_eq!(first["archive_version"], "1");
    assert_eq!(first["node_id"], "first");
    assert_eq!(first["parent_id"], Value::Null);
    assert_eq!(first["timestamp_ns"], "9007199254740993");
    assert_eq!(first["content_id"], BODY_HASH);
    assert_eq!(
        first["body"],
        json!({"availability": "present", "utf8_length": "20", "sha256": BODY_HASH})
    );
    assert_eq!(first["attachments"][0]["byte_length"], "3");
    assert_eq!(
        first["attachments"][0]["payload"]["sha256"],
        ATTACHMENT_HASH
    );

    let head = fresh_process(&path, 7);
    assert_eq!(head["projection"]["activity_cut"], "3");
    assert_eq!(head["projection"]["turns"][2]["parent_id"], "middle");
    let branches = head["projection"]["branch_revisions"].as_array().unwrap();
    let leaves = branches
        .iter()
        .map(|revision| revision["branch"]["leaf_node_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(leaves, ["first", "middle", "head"]);
    assert_eq!(branches[1]["archive_version"], "4");
    assert_eq!(head["projection"]["episodes"][0]["start_node_id"], "first");
    assert_eq!(head["projection"]["episodes"][0]["end_node_id"], "head");
    assert_eq!(head["projection"]["episodes"][0]["boundary"], "explicit");

    let future = fresh_process(&path, 8);
    assert_eq!(future["archive_observation_status"], "INCOMPLETE");
    assert_eq!(future["reason"], "archive_cut_unavailable");
    assert_eq!(future["projection"], Value::Null);
    assert_eq!(future["candidate_unchanged"], true);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn changed_intermediate_turn_is_visible_with_the_same_branch_head() {
    let directory = tempdir().unwrap();
    let expected = directory.path().join("expected.rel");
    let altered = directory.path().join("altered.rel");
    build(&expected, "original intermediate body", false);
    build(&altered, "altered intermediate body", false);
    let before = fs::read(&altered).unwrap();

    let left = fresh_process(&expected, 7);
    let right = fresh_process(&altered, 7);
    assert_eq!(left["archive_head"], right["archive_head"]);
    assert_eq!(
        left["projection"]["branches"],
        right["projection"]["branches"]
    );
    assert_eq!(
        left["projection"]["turns"][2],
        right["projection"]["turns"][2]
    );
    assert_ne!(
        left["projection"]["turns"][1]["body"],
        right["projection"]["turns"][1]["body"]
    );
    assert_ne!(left["projection_sha256"], right["projection_sha256"]);
    assert_eq!(fs::read(&altered).unwrap(), before);
}

#[test]
fn pruned_body_stays_unavailable_even_at_its_original_cut() {
    let directory = tempdir().unwrap();
    let source = directory.path().join("unpruned.rel");
    let target = directory.path().join("pruned.rel");
    build(&source, "original intermediate body", true);
    let plan = VerifiedOwnerCopyPlan::prepare(&source).unwrap();
    let grant = GraphAdmission::for_replay(OWNER, [], [], [], [], []);
    let receipt = prune_terminal_archive_copy(
        &plan,
        &source,
        &target,
        &["synthetic-conversation".into()],
        &grant,
    )
    .unwrap();
    let report = observe_archive_cut(CandidateKind::Rel, &target, 1);
    let body = &report["projection"]["turns"][0]["body"];
    assert_eq!(body["availability"], "pruned_terminal");
    let manifest = receipt
        .manifest_digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(body["prune_manifest_sha256"], manifest);
    assert_eq!(report["projection"]["turns"][0]["content_id"], BODY_HASH);
    assert_eq!(report["candidate_unchanged"], true);
}
