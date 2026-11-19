use d02_current_reader::{CandidateKind, observe_memory_history};
use reliquary_container::Container;
use reliquary_memory_core::{MemoryDraft, MemoryId, MemorySourceRef, MemoryStore};
use reliquary_owner_session::{
    GraphAdmission, PhyWriter, VerifiedOwnerCopyPlan, prune_terminal_memory_copy,
};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
use tempfile::tempdir;

const OWNER: [u8; 16] = [0x57; 16];
const SOURCE_OWNER: &str = "57575757575757575757575757575757";

fn draft(revision: usize, temporal_status: &str) -> MemoryDraft {
    MemoryDraft {
        title: "fixture memory title".into(),
        content: "same retained body".into(),
        category: "fact".into(),
        memory_type: "user".into(),
        authority_kind: "direct".into(),
        temporal_status: temporal_status.into(),
        scope: "user".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: Some(MemorySourceRef {
            owner_id: SOURCE_OWNER.into(),
            principal_id: Some("synthetic-principal".into()),
            source_episode_id: [0x34; 32],
            source_node_id: "source-node".into(),
            content_source_conversation_id: Some("source-conversation".into()),
            content_source_node_id: Some("source-node".into()),
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
        }),
        source_time_ns: Some(9_007_199_254_740_993),
        temporal_inference: None,
        mutation_id: format!("fixture-memory-mutation-{revision}"),
        created_at_ns: 9_007_199_254_740_993,
        updated_at_ns: 9_007_199_254_740_993 + revision as i64,
    }
}

fn build(path: &Path, intermediate_status: &str, tombstone: bool) -> MemoryId {
    drop(PhyWriter::create_path(path, OWNER).unwrap());
    let mut container = Container::open_writer(path).unwrap();
    let mut memories = MemoryStore::open(&container).unwrap();
    let id = memories
        .publish(&mut container, None, 0, draft(1, "current"))
        .unwrap()
        .revision
        .record
        .id;
    memories
        .publish(&mut container, Some(id), 1, draft(2, intermediate_status))
        .unwrap();
    memories
        .publish(&mut container, Some(id), 2, draft(3, "current"))
        .unwrap();
    if tombstone {
        memories.tombstone(&mut container, id, 3).unwrap();
    }
    container.sync().unwrap();
    id
}

fn fresh_process(path: &Path, id: MemoryId, cut: u64) -> Value {
    let id = id.0.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let result = Command::new(env!("CARGO_BIN_EXE_d02-current-reader"))
        .args(["--kind", "phy", "--candidate"])
        .arg(path)
        .args(["--memory-history", &format!("{id}:{cut}")])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stderr.is_empty(), "{:?}", result.stderr);
    serde_json::from_slice(&result.stdout).unwrap()
}

#[test]
fn distinguishes_genesis_intermediate_current_and_tombstone_cuts() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("history.phy");
    let id = build(&path, "past", true);
    let before = fs::read(&path).unwrap();

    let genesis = fresh_process(&path, id, 0);
    assert_eq!(genesis["projection"]["presence"], "absent_at_cut");
    assert_eq!(genesis["projection"]["history"], Value::Null);

    let early = fresh_process(&path, id, 1);
    assert_eq!(early, fresh_process(&path, id, 1));
    assert_eq!(early["status"], "INCOMPLETE");
    assert_eq!(early["memory_observation_status"], "OBSERVED");
    assert_eq!(early["memory_head"], "4");
    assert_eq!(early["owner"]["owner_uuid"], SOURCE_OWNER);
    assert_eq!(early["candidate_unchanged"], true);
    assert_eq!(
        early["candidate_sha256_before"],
        early["candidate_sha256_after"]
    );
    let first = &early["projection"]["history"]["revisions"][0];
    assert_eq!(first["revision"], "1");
    assert_eq!(first["memory_version"], "1");
    assert_eq!(first["source_ref"]["principal_id"], "synthetic-principal");
    assert_eq!(first["source_time_ns"], "9007199254740993");
    assert_eq!(
        first["payload"],
        json!({
            "availability": "present",
            "title_sha256": "c870e4b26e32c47d1e4150d35cf6f9470ac31f2696d7eb125e870f26f00f5a25",
            "title_utf8_length": "20",
            "content_sha256": "fd57ad403cc4ba4889edd895e1d11f565bd89f1169bb76339799bff2210e09bb",
            "content_utf8_length": "18",
        })
    );

    let mid = fresh_process(&path, id, 2);
    let revisions = mid["projection"]["history"]["revisions"]
        .as_array()
        .unwrap();
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[1]["temporal_status"], "past");
    assert_eq!(mid["projection"]["history"]["tombstone"], Value::Null);
    let terminal = fresh_process(&path, id, 4);
    assert_eq!(
        terminal["projection"]["history"]["tombstone"],
        json!({"memory_version": "4", "revision": "4", "standalone_origin": null}),
    );
    let future = fresh_process(&path, id, 5);
    assert_eq!(future["memory_observation_status"], "INCOMPLETE");
    assert_eq!(future["projection"], Value::Null);
    assert_eq!(future["reason"], "memory_cut_unavailable");
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn detects_metadata_change_in_intermediate_revision_with_same_head() {
    let directory = tempdir().unwrap();
    let original = directory.path().join("original.phy");
    let altered = directory.path().join("altered.phy");
    let id = build(&original, "past", false);
    assert_eq!(id, build(&altered, "current", false));
    let left = fresh_process(&original, id, 3);
    let right = fresh_process(&altered, id, 3);
    let left_revisions = &left["projection"]["history"]["revisions"];
    let right_revisions = &right["projection"]["history"]["revisions"];
    assert_eq!(left_revisions[2], right_revisions[2]);
    assert_ne!(left_revisions[1], right_revisions[1]);
    assert_ne!(left["projection_sha256"], right["projection_sha256"]);
}

#[test]
fn pruned_memory_body_is_unavailable_with_manifest_at_original_cut() {
    let directory = tempdir().unwrap();
    let source = directory.path().join("unpruned.phy");
    let target = directory.path().join("pruned.phy");
    let id = build(&source, "past", true);
    let plan = VerifiedOwnerCopyPlan::prepare(&source).unwrap();
    let grant = GraphAdmission::for_replay(OWNER, [], [], [], [], []);
    let receipt = prune_terminal_memory_copy(&plan, &source, &target, &[id], &grant).unwrap();
    let original = observe_memory_history(CandidateKind::Phy, &source, id, 1);
    let pruned = observe_memory_history(CandidateKind::Phy, &target, id, 1);
    let old = &original["projection"]["history"]["revisions"][0];
    let new = &pruned["projection"]["history"]["revisions"][0];
    assert_eq!(old["body_id"], new["body_id"]);
    assert_eq!(old["payload"]["availability"], "present");
    assert_eq!(new["payload"]["availability"], "pruned_terminal");
    let digest = receipt
        .manifest_digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(new["payload"]["prune_manifest_sha256"], digest);
    assert_eq!(pruned["projection"]["history"]["tombstone"], Value::Null);
    assert_eq!(pruned["candidate_unchanged"], true);
}

fn envelope_copy(source: &Path, target: &Path, second_time: i64) {
    let original = Container::open_read_only(source).unwrap();
    let mut copy = Container::create(target, original.identity(), original.owner_id()).unwrap();
    let mut revision = 0;
    for frame in original.read_frames().unwrap() {
        if matches!(frame.payload.get(..8), Some(b"RELMCRT1" | b"RELMDLT1")) {
            revision += 1;
            if revision == 2 {
                let reserved = copy.reserve_version(Some(second_time)).unwrap();
                copy.append_bound_publication_envelope(reserved, &frame.payload)
                    .unwrap();
            } else {
                copy.append_publication_envelope(&frame.payload, Some(10 * revision))
                    .unwrap();
            }
        } else {
            copy.append(&frame.payload).unwrap();
        }
    }
    copy.sync().unwrap();
}

#[test]
fn wrapped_revisions_resolve_own_global_and_reserved_transaction_coordinates() {
    let directory = tempdir().unwrap();
    let unwrapped = directory.path().join("unwrapped.phy");
    let reference = directory.path().join("reference.phy");
    let altered = directory.path().join("altered.phy");
    let id = build(&unwrapped, "past", false);
    envelope_copy(&unwrapped, &reference, 9_007_199_254_740_993);
    envelope_copy(&unwrapped, &altered, 9_007_199_254_740_994);

    let expected = fresh_process(&reference, id, 2);
    let observed = fresh_process(&altered, id, 2);
    assert_eq!(expected["memory_observation_status"], "OBSERVED");
    let revisions = &expected["projection"]["history"]["revisions"];
    assert_eq!(revisions.as_array().unwrap().len(), 2);
    assert_eq!(
        revisions[0]["standalone_origin"],
        json!({"global_version": "1", "transaction_time_ns": "10", "bound_reservation": false}),
    );
    assert_eq!(
        revisions[1]["standalone_origin"],
        json!({
            "global_version": "2",
            "transaction_time_ns": "9007199254740993",
            "bound_reservation": true,
        }),
    );
    assert_ne!(expected["projection_sha256"], observed["projection_sha256"]);
}

#[test]
fn bulk_observation_preserves_native_routing_body_and_span() {
    use d02_current_reader::observe_memory_owner_history;
    use reliquary_memory_core::{
        MemoryRoutingField, MemoryRoutingMention, MemoryRoutingMetadata, encode_routing_metadata,
        memory_body_id,
    };
    use sha2::{Digest, Sha256};

    let directory = tempdir().unwrap();
    let unwrapped = directory.path().join("unwrapped.phy");
    let routed = directory.path().join("routed.phy");
    let id = build(&unwrapped, "past", false);
    envelope_copy(&unwrapped, &routed, -9_007_199_254_740_995);
    let body_id = memory_body_id("fixture memory title", "same retained body");
    let mut container = Container::open_writer(&routed).unwrap();
    let metadata = MemoryRoutingMetadata {
        memory_id: id,
        body_id,
        mentions: vec![MemoryRoutingMention {
            field: MemoryRoutingField::Title,
            start_byte: 0,
            end_byte: 7,
            text: "fixture".into(),
        }],
    };
    container
        .append(&encode_routing_metadata(&metadata).unwrap())
        .unwrap();
    container.sync().unwrap();
    drop(container);
    let observed = observe_memory_owner_history(CandidateKind::Phy, &routed);
    assert_eq!(observed["observation_status"], "OBSERVED", "{observed}");
    assert_eq!(observed["memory_head"], "3");
    let rows = observed["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 6);
    let routing = rows.iter().find(|row| row["family"] == "routing").unwrap();
    let as_hex = |bytes: [u8; 32]| {
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let value = json!({
        "memory_id": as_hex(id.0),
        "body_id": as_hex(body_id.0),
        "mentions": [{"field": 1, "start": 0, "end": 7, "text": "fixture"}],
    });
    let canonical = serde_json::to_vec(&value).unwrap();
    assert_eq!(
        routing["canonical_value_sha256"],
        format!("{:x}", Sha256::digest(canonical))
    );
}
