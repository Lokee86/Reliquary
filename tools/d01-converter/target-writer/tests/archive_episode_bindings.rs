use reliquary_archive::EpisodeId;
use reliquary_memory_core::{
    MemoryRecord, encode_grouped_creation, memory_body_bytes, memory_body_id, memory_id,
};
use reliquary_owner_session::{
    CompletionCheck, RelSession, RelWriter,
    records::{GroupedInsomniaCompletion, GroupedMemoryBody},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};

#[test]
fn versioned_episode_and_turns_admit_normal_grouped_work_after_reopen() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    fs::create_dir_all(&root).unwrap();
    let worktree = tempfile::tempdir_in(root).unwrap();
    let fixture_path = worktree.path().join("fixture.json");
    let candidate = worktree.path().join("episode.rel");
    let mut fixture: Value =
        serde_json::from_slice(include_bytes!("archive-bindings-transfer.json")).unwrap();
    let mut id = Sha256::new();
    for value in ["synthetic-conversation", "root", "reply"] {
        id.update((value.len() as u64).to_le_bytes());
        id.update(value);
    }
    let id: [u8; 32] = id.finalize().into();
    fixture["events"].as_array_mut().unwrap().extend([
        json!({"operation":"reserve", "global_version":"7", "transaction_time_ns":"-9"}),
        json!({
            "operation":"episode", "archive_version":"7",
            "clock":{"global_version":"7", "transaction_time_ns":"-9", "bound_reservation":true},
            "episode":{
                "id":hex::encode(id), "conversation":"synthetic-conversation",
                "start":"root", "end":"reply", "origin":"live", "boundary":"explicit",
                "source_through_ns":"9007199254741009", "finalized_at_ns":"9007199254741019"
            }
        }),
    ]);
    let bytes = serde_json::to_vec(&fixture).unwrap();
    fs::write(&fixture_path, &bytes).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_d01-synthetic-target-writer"))
        .arg("--synthetic-archive-bindings")
        .arg(&fixture_path)
        .arg(hex::encode(Sha256::digest(&bytes)))
        .arg(&candidate)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");

    let mut writer = RelWriter::open_path_current(&candidate).unwrap();
    let now = 9_007_199_254_741_030;
    let claim = writer
        .queue_and_claim_insomnia_episode(EpisodeId(id), "fixture-worker", now, 10)
        .unwrap();
    let mutation = "fixture-episode-memory";
    let memory = memory_id(mutation);
    let body_id = memory_body_id("fixture title", "fixture content");
    let completion = GroupedInsomniaCompletion {
        episode_id: id,
        attempt: claim.attempt_count,
        started_at_ns: now,
        completed_at_ns: now + 1,
        transaction_time_ns: now + 1,
        extractor_model: "fixture".into(),
        extractor_version: "1".into(),
        rejected_count: 0,
        memory_ids: vec![memory.0],
        external_memory_refs: vec![],
        global_version_start: 8,
        bodies: vec![GroupedMemoryBody {
            id: body_id,
            bytes: memory_body_bytes("fixture title", "fixture content"),
        }],
        memory_records: vec![
            encode_grouped_creation(&MemoryRecord {
                id: memory,
                revision: 1,
                body_id,
                category: "fact".into(),
                memory_type: "semantic".into(),
                authority_kind: "user".into(),
                temporal_status: "current".into(),
                scope: "project".into(),
                lifecycle_state: "extracted".into(),
                archived: false,
                parent_id: None,
                superseded_by: None,
                source_node_id: Some("root".into()),
                content_source_conversation_id: None,
                content_source_node_id: None,
                grounding_source_conversation_id: None,
                grounding_source_node_id: None,
                source_episode_id: Some(id),
                source_ref: None,
                source_time_ns: Some(9_007_199_254_741_009),
                temporal_inference: None,
                mutation_id: mutation.into(),
                created_at_ns: now,
                updated_at_ns: now,
                memory_version: 1,
            })
            .unwrap(),
        ],
        routing_metadata: vec![],
        freshness_birth_turn: Some(2),
        freshness_policy_version: Some(1),
        freshness_birth_memory_ids: vec![memory.0],
    };
    assert_eq!(
        writer
            .commit_grouped_insomnia_completion(claim.lease_token.unwrap(), completion, now + 1)
            .unwrap(),
        CompletionCheck::Apply
    );
    drop(writer);

    let reopened = RelSession::open_path_current(&candidate).unwrap();
    let episode = reopened.episode(EpisodeId(id)).unwrap();
    assert_eq!(episode.source_through_ns, 9_007_199_254_741_009);
    assert_eq!(
        reopened.archive_publication_bindings()[6].transaction_time_ns,
        Some(-9)
    );
    assert_eq!(reopened.rel_turn_count(), 2);
    assert_eq!(
        reopened
            .current_memory(memory)
            .unwrap()
            .record
            .source_episode_id,
        Some(id)
    );
}
