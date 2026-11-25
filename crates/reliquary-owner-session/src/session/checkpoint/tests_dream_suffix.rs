use super::*;
use crate::records::{GroupedInsomniaCompletion, GroupedMemoryBody, ImportEpochStatus};
use crate::session::{DreamBatchCut, DreamPairPublication, RelWriter};
use reliquary_archive::{Branch, ImportReceipt, TurnInput};
use reliquary_freshness_store::FreshnessMemoryId;
use reliquary_graph::{GraphAdmission, NodeRef, RelationChange, RelationKind, RelationOrigin};
use reliquary_memory_core::{
    MemoryBodyId, MemoryRecord, encode_grouped_creation, memory_body_bytes, memory_body_id,
    memory_id,
};

const OWNER: [u8; 16] = [93; 16];

#[test]
fn suspended_import_checkpoint_replays_initial_dream_suffix() {
    let directory = tempfile::tempdir().unwrap();
    let owner = directory.path().join("dream-suffix.rel");
    let private = directory.path().join("private");
    private_cache::test_directory(&private).unwrap();
    let slot = private.join("owner.checkpoint");
    let import_key = "d3".repeat(32);
    let conversation = "dream-import-conversation";
    let turn_id = "dream-import-turn";

    // Make a normal imported source and grouped completion, but leave the
    // import epoch suspended immediately before its initial Dream pass.
    let mut writer = RelWriter::create_current_path(&owner, OWNER).unwrap();
    let older = writer
        .session
        .common
        .memories
        .publish(
            &mut writer.container,
            None,
            0,
            reliquary_memory_core::MemoryDraft {
                lifecycle_state: "knowledge".into(),
                ..super::tests::draft()
            },
        )
        .unwrap()
        .revision
        .record;
    let older_cut = crate::DreamMemoryCut {
        id: older.id,
        revision: older.revision,
        body_id: older.body_id,
    };
    writer
        .session
        .freshness
        .create_birth(
            &mut writer.container,
            FreshnessMemoryId(older.id.0),
            0,
            true,
        )
        .unwrap();
    writer.container.sync().unwrap();
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    let ImportEpochStatus::Open(start) = writer
        .start_native_import(&import_key, "checkpoint/dream", "grouped-v1", 200)
        .unwrap()
    else {
        panic!("fixture import must be open");
    };
    writer
        .accept_native_import_turn(
            &start.import_id,
            TurnInput {
                id: turn_id.into(),
                conversation_id: conversation.into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("alice".into()),
                timestamp_ns: 200,
                content: "imported source evidence".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
            Branch {
                id: "main".into(),
                conversation_id: conversation.into(),
                leaf_node_id: turn_id.into(),
                canonical: true,
            },
        )
        .unwrap();
    let episode = writer
        .finalize_native_import_episode(&start.import_id, conversation, turn_id, turn_id, 201)
        .unwrap();
    writer
        .commit_native_import_receipt(
            &start.import_id,
            ImportReceipt {
                schema_version: 1,
                owner_uuid: OWNER,
                import_key: [0xd3; 32],
                source_namespace: "checkpoint/dream".into(),
                source_format: "grouped-v1".into(),
                source_digest: [0xd3; 32],
                conversations: vec![crate::model::ImportedConversation {
                    source_id: "source-conversation".into(),
                    conversation_id: conversation.into(),
                    title: Some("Imported Dream source".into()),
                    messages: vec![crate::model::ImportedMessage {
                        source_author: None,
                        source_id: "source-turn".into(),
                        revision: "1".into(),
                        source_parent_id: None,
                        replaces: None,
                        deleted: false,
                        node_id: Some(turn_id.into()),
                        fingerprint: [0x44; 32],
                        artifacts: vec![],
                    }],
                    branches: vec![crate::model::ImportedBranch {
                        source_id: "source-main".into(),
                        branch_id: "main".into(),
                        leaf_node_id: Some(turn_id.into()),
                        canonical: true,
                    }],
                }],
                receipt_digest: [0; 32],
            },
        )
        .unwrap();

    let claim = writer
        .claim_native_import_episode(&start.import_id, episode, "fixture", 202, 100)
        .unwrap();
    let id = memory_id("dream-suffix-memory");
    let body_id = memory_body_id("Dream candidate", "candidate memory body");
    let record = MemoryRecord {
        id,
        revision: 1,
        body_id,
        category: "fact".into(),
        memory_type: "semantic".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
        lifecycle_state: "extracted".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: Some(turn_id.into()),
        content_source_conversation_id: Some(conversation.into()),
        content_source_node_id: Some(turn_id.into()),
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: Some(episode.0),
        source_ref: None,
        source_time_ns: Some(200),
        temporal_inference: None,
        mutation_id: "dream-suffix-memory".into(),
        created_at_ns: 203,
        updated_at_ns: 203,
        memory_version: writer.read_session().memory_version() + 1,
    };
    writer
        .commit_native_import_completion_with_routed_receipts(
            &start.import_id,
            claim.work.lease_token.unwrap(),
            GroupedInsomniaCompletion {
                episode_id: episode.0,
                attempt: claim.work.attempt_count,
                started_at_ns: claim.started_at_ns,
                completed_at_ns: 204,
                transaction_time_ns: 204,
                extractor_model: "fixture".into(),
                extractor_version: "v1".into(),
                rejected_count: 0,
                memory_ids: vec![id.0],
                external_memory_refs: vec![],
                global_version_start: writer.next_global_version().unwrap(),
                bodies: vec![GroupedMemoryBody {
                    id: MemoryBodyId(body_id.0),
                    bytes: memory_body_bytes("Dream candidate", "candidate memory body"),
                }],
                memory_records: vec![encode_grouped_creation(&record).unwrap()],
                routing_metadata: vec![],
                freshness_birth_turn: Some(writer.read_session().rel_turn_count()),
                freshness_policy_version: Some(1),
                freshness_birth_memory_ids: vec![id.0],
            },
            204,
            &[],
        )
        .unwrap();
    let import_id = start.import_id;
    drop(writer);

    // Publish a checkpoint before the initial Dream's DRC/B/F records.
    let lease =
        Container::prepare_checkpoint_open(&owner, true, FileKind::Reliquary, Some(OWNER)).unwrap();
    let before_dream = lease.extent();
    let (container, session, mut checkpoint) =
        open_leased(lease, Some(slot.clone()), before_dream, true).unwrap();
    checkpoint
        .as_mut()
        .expect("writable checkpoint")
        .refresh(&container, &session, true);
    drop((session, container));

    let mut writer = RelWriter::open_path(
        &owner,
        GraphAdmission::for_changes(OWNER, [id.0, older.id.0], []),
        [],
        [],
    )
    .unwrap();
    let cut = crate::DreamMemoryCut {
        id,
        revision: 1,
        body_id: MemoryBodyId(body_id.0),
    };
    let (pass, publication) = RelWriter::native_import_dream_identity(&import_id, id);
    let turn = writer.read_session().rel_turn_count();
    writer
        .begin_native_import_dream_frontier(&import_id, pass.clone(), cut, vec![older_cut], turn)
        .unwrap();
    writer
        .commit_native_import_dream_batch(
            &import_id,
            &pass,
            DreamBatchCut {
                owner: OWNER,
                publication_id: publication,
                graph_version: 0,
                observed: vec![cut, older_cut],
                pairs: vec![DreamPairPublication {
                    source: id,
                    candidate: older.id,
                    kind: Some(RelationKind::Topical),
                    rejected: false,
                }],
                changes: vec![RelationChange {
                    source: NodeRef::Memory(id.0),
                    target: NodeRef::Memory(older.id.0),
                    kind: RelationKind::Topical,
                    active: true,
                    origin: RelationOrigin::Dream,
                }],
                lifecycles: vec![],
                updated_at_ns: 205,
            },
        )
        .unwrap();
    let principal = writer.session.freshness.policy().linkage_principal;
    writer
        .commit_native_import_initial_dream_pass(
            &import_id,
            &pass,
            cut,
            1,
            turn,
            [(older.id, principal)].into(),
            true,
        )
        .unwrap();
    let import_state = writer.read_session().import_status(&import_id).unwrap();
    assert!(matches!(import_state, ImportEpochStatus::Open(_)));
    drop(writer);

    // The checkpoint must remain anchored at the pre-Dream cut while the
    // suffix path restores Graph, Memory, Freshness and Dream state.
    let lease =
        Container::prepare_checkpoint_open(&owner, true, FileKind::Reliquary, Some(OWNER)).unwrap();
    let extent = lease.extent();
    let (container, warm, checkpoint) =
        open_leased(lease, Some(slot.clone()), extent, true).unwrap();
    assert_eq!(checkpoint.unwrap().attempted_extent, Some(before_dream));
    assert!(extent > before_dream);
    assert_eq!(warm.open_native_import_id(), Some(import_id.as_str()));
    assert_eq!(warm.common.graph.active_relations().count(), 1);
    assert!(warm.dream_pair_evaluated(id, older.id));
    assert!(warm.common.dreams.initial_settlement(&pass).is_some());
    assert_eq!(warm.current_memory(id).unwrap().record.revision, 2);
    let warm_frontier = warm.initial_dream_frontier(&pass).unwrap();
    assert_eq!(warm_frontier.candidates, [older_cut]);
    let warm_relations = warm.graph_relations();
    let warm_memory_version = warm.memory_version();
    let warm_graph_version = warm.graph_version();
    let memory_cut = warm.freshness.cut_through_turn(warm.rel_turn_count());
    let warm_freshness = warm.freshness.records_at(&memory_cut).unwrap();
    let current_cut = warm.current_historical_cut();
    let warm_proof = warm.historical_proof(OWNER, &current_cut).unwrap();
    drop((warm, container));

    let full = RelSession::open_path_current(&owner).unwrap();
    assert_eq!(full.open_native_import_id(), Some(import_id.as_str()));
    assert_eq!(full.memory_version(), warm_memory_version);
    assert_eq!(full.graph_version(), warm_graph_version);
    assert_eq!(full.graph_relations(), warm_relations);
    assert_eq!(full.initial_dream_frontier(&pass), Some(warm_frontier));
    let full_cut = full.freshness.cut_through_turn(full.rel_turn_count());
    assert_eq!(
        warm_freshness,
        full.freshness.records_at(&full_cut).unwrap()
    );
    assert_eq!(
        warm_proof,
        full.historical_proof(OWNER, &current_cut).unwrap()
    );
    drop(full);

    // A second physical settlement is corrupt even when every byte matches
    // the last legitimate settlement. A provisional restore cannot mask it.
    let mut container = Container::open_writer(&owner).unwrap();
    let mut tail = container.scan_frames_from(before_dream).unwrap();
    let mut settlement = None;
    while let Some(frame) = tail.next_frame().unwrap() {
        if frame.payload.starts_with(b"RELDRF02") {
            settlement = Some(frame.payload.to_vec());
        }
    }
    drop(tail);
    container.append(&settlement.unwrap()).unwrap();
    container.sync().unwrap();
    drop(container);
    let lease =
        Container::prepare_checkpoint_open(&owner, true, FileKind::Reliquary, Some(OWNER)).unwrap();
    let extent = lease.extent();
    assert!(open_leased(lease, Some(slot), extent, true).is_err());
    assert!(RelSession::open_path_current(&owner).is_err());
}
