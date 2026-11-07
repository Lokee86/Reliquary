use super::*;
use crate::ArchiveVectorRow;
use crate::records::{GroupedInsomniaCompletion, GroupedMemoryBody};
use reliquary_archive::{Branch, TurnInput};
use reliquary_container::{ContainerIdentity, ReliquaryScope};
use reliquary_memory_core::{
    MemoryBodyId, MemoryDraft, MemoryRecord, MemorySourceRef, MemoryStore, encode_grouped_creation,
    memory_body_bytes, memory_body_id, memory_id,
};
use reliquary_vector_store::{
    CompatibilityProbeReference, CompatibilityProfile, CompatibilityProfileStore, EmbeddingMode,
    MemoryVectorStore, PackedVectorStore, VectorNormalization,
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&root).unwrap();
        Self(root.join(format!(
            "pruned-owner-{}-{}.phy",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn draft(mutation: &str, content: &str) -> MemoryDraft {
    MemoryDraft {
        title: "maintenance-content".into(),
        content: content.into(),
        category: "fact".into(),
        memory_type: "user".into(),
        authority_kind: "direct".into(),
        temporal_status: "past".into(),
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
        source_ref: None,
        source_time_ns: None,
        temporal_inference: None,
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn archive_profile() -> CompatibilityProfile {
    let mut profile = CompatibilityProfile {
        id: CompatibilityProfileId([0; 32]),
        dimensions: 2,
        normalization: VectorNormalization::L2,
        probe_suite_version: 1,
        compatibility_policy_version: 3,
        references: [
            EmbeddingMode::Query,
            EmbeddingMode::Query,
            EmbeddingMode::Document,
            EmbeddingMode::Document,
        ]
        .into_iter()
        .map(|mode| CompatibilityProbeReference {
            mode,
            vector: vec![1.0, 0.0],
        })
        .collect(),
    };
    profile.id = reliquary_vector_store::profile::compatibility_profile_id(&profile);
    profile
}

#[test]
fn terminal_memory_prune_removes_payload_reopens_and_rejects_live_or_tampered_history() {
    let source = Temp::new();
    let output = Temp::new();
    let forged = Temp::new();
    let owner = [0xa1; 16];
    let private = "sensitive terminal content ".repeat(350);
    let (tombstoned, live);
    {
        let mut container =
            Container::create(&source.0, ContainerIdentity::phylactery(), owner).unwrap();
        PackedVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        CompatibilityProfileStore::default()
            .initialize(&mut container)
            .unwrap();
        MemoryVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        let mut memories = MemoryStore::create(&mut container).unwrap();
        live = memories
            .publish(
                &mut container,
                None,
                0,
                draft("keep-live", "important active fact"),
            )
            .unwrap()
            .revision
            .record
            .id;
        tombstoned = memories
            .publish_versioned_creation(&mut container, draft("discard-later", &private), 1)
            .unwrap()
            .revision
            .record
            .id;
        memories.tombstone(&mut container, tombstoned, 1).unwrap();
        container.sync().unwrap();
    }
    let snapshot = fs::read(&source.0).unwrap();
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    assert!(prune_terminal_memory_copy(&plan, &source.0, &output.0, &[live], &grant).is_err());
    assert!(!output.0.exists());
    let report =
        prune_terminal_memory_copy(&plan, &source.0, &output.0, &[tombstoned], &grant).unwrap();
    assert_eq!(report.pruned_memories, 1);
    assert_eq!(fs::read(&source.0).unwrap(), snapshot);
    assert!(report.transformed_bytes < report.source_bytes);
    let transformed = fs::read(&output.0).unwrap();
    assert!(
        !transformed
            .windows(private.len())
            .any(|part| part == private.as_bytes())
    );
    drop(PhyWriter::open_path(&output.0, &grant, []).unwrap());
    let session = PhySession::open_path(&output.0, &grant, []).unwrap();
    let removed = session.memory_history(tombstoned).unwrap().unwrap();
    assert!(removed.tombstone.is_some());
    assert!(matches!(removed.revisions[0].payload, PayloadState::Absent));
    let retained = session.memory_history(live).unwrap().unwrap();
    assert!(matches!(
        retained.revisions[0].payload,
        PayloadState::Available(_)
    ));
    drop(session);
    fs::write(&forged.0, &transformed).unwrap();
    let mut bytes = fs::read(&forged.0).unwrap();
    let at = bytes
        .windows(8)
        .position(|window| window == b"RELHPR01")
        .unwrap();
    bytes[at + 30] ^= 1;
    fs::write(&forged.0, &bytes).unwrap();
    assert!(PhySession::open_path(&forged.0, &grant, []).is_err());
    let unsupported = Temp::new();
    let copy = Container::open_read_only(&output.0).unwrap();
    let mut invalid = Container::create(&unsupported.0, copy.identity(), owner).unwrap();
    for frame in copy.read_frames().unwrap() {
        if frame.payload.starts_with(b"RELHPR01") {
            let mut manifest = HistoryPruneManifest::decode(&frame.payload).unwrap();
            manifest.descriptors[0].target.domain = RetentionDomain::Archive;
            invalid.append(&manifest.encode().unwrap()).unwrap();
        } else {
            invalid.append(&frame.payload).unwrap();
        }
    }
    invalid.sync().unwrap();
    drop(invalid);
    drop(copy);
    assert!(PhySession::open_path(&unsupported.0, &grant, []).is_err());
    let appended = Temp::new();
    fs::write(&appended.0, &transformed).unwrap();
    let mut appended_writer = Container::open_writer(&appended.0).unwrap();
    appended_writer.append(b"RELMFMT1").unwrap();
    appended_writer.sync().unwrap();
    drop(appended_writer);
    assert!(PhySession::open_path(&appended.0, &grant, []).is_err());
    assert!(
        prune_terminal_memory_copy(&plan, &source.0, &output.0, &[tombstoned], &grant).is_err()
    );
    assert_eq!(fs::read(&output.0).unwrap(), transformed);
    let backup = Temp::new();
    let active_writer = PhyWriter::open_path(&source.0, &grant, []).unwrap();
    assert!(
        promote_terminal_memory_copy(&plan, &source.0, &backup.0, &[tombstoned], &grant).is_err()
    );
    assert!(!backup.0.exists());
    drop(active_writer);
    assert_eq!(fs::read(&source.0).unwrap(), snapshot);

    assert!(
        promote_terminal_memory_copy(&plan, &source.0, &output.0, &[tombstoned], &grant).is_err()
    );
    assert_eq!(fs::read(&source.0).unwrap(), snapshot);
    assert_eq!(fs::read(&output.0).unwrap(), transformed);
    assert!(
        promote_terminal_memory_copy(&plan, &source.0, &source.0, &[tombstoned], &grant).is_err()
    );
    promote_terminal_memory_copy(&plan, &source.0, &backup.0, &[tombstoned], &grant).unwrap();
    assert_eq!(fs::read(&backup.0).unwrap(), snapshot);
    let mut writer = PhyWriter::open_path(&source.0, &grant, []).unwrap();
    assert!(
        writer
            .common
            .memories
            .publish(
                &mut writer.container,
                None,
                0,
                draft("restore-pruned", &private)
            )
            .is_err()
    );
    writer
        .common
        .memories
        .publish_versioned_creation(
            &mut writer.container,
            draft("after-promotion", "new owner content"),
            42,
        )
        .unwrap();
    writer.container.sync().unwrap();
    drop(writer);
    let reopened = PhySession::open_path(&source.0, &grant, []).unwrap();
    assert!(matches!(
        reopened
            .memory_history(tombstoned)
            .unwrap()
            .unwrap()
            .revisions[0]
            .payload,
        PayloadState::Absent
    ));
    assert_eq!(fs::read(&backup.0).unwrap(), snapshot);
}

#[test]
fn rel_terminal_memory_prune_retains_validated_operations_and_relocates_vector_receipt() {
    let source = Temp::new();
    let output = Temp::new();
    let owner = [0xa9; 16];
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let memory;
    let profile;
    let generation;
    let episode;
    {
        let mut writer = RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap();
        writer
            .accept_turn_and_advance_branch(
                TurnInput {
                    id: "vector-source-turn".into(),
                    conversation_id: "vector-source".into(),
                    parent_id: None,
                    role: "user".into(),
                    principal_id: Some("maintenance-principal".into()),
                    timestamp_ns: 10,
                    content: "retained archive vector source".into(),
                    attachments: vec![],
                    project_attachments: vec![],
                },
                Branch {
                    id: "main".into(),
                    conversation_id: "vector-source".into(),
                    leaf_node_id: "vector-source-turn".into(),
                    canonical: true,
                },
            )
            .unwrap();
        memory = writer
            .session
            .common
            .memories
            .publish_versioned_creation(
                &mut writer.container,
                draft(
                    "terminal-memory",
                    &"terminal body retained only as proof ".repeat(120),
                ),
                1,
            )
            .unwrap()
            .revision
            .record
            .id;
        writer
            .session
            .common
            .memories
            .tombstone(&mut writer.container, memory, 1)
            .unwrap();
        episode = writer
            .finalize_episode(
                "vector-source",
                "vector-source-turn",
                "vector-source-turn",
                reliquary_archive::EpisodeOrigin::Live,
                reliquary_archive::EpisodeBoundary::Explicit,
                11,
            )
            .unwrap();
        let claim = writer
            .claim_next_insomnia_input("maintenance-worker", 12, 100, 1)
            .unwrap()
            .unwrap();
        assert_eq!(claim.episode.id, episode);

        profile = writer
            .publish_compatibility_profile(archive_profile())
            .unwrap();
        let input = writer.archive_vector_input(profile).unwrap();
        let rows = input
            .sources()
            .iter()
            .map(|fragment| ArchiveVectorRow {
                fragment_id: fragment.fragment_id,
                max_archive_version: fragment.max_archive_version,
                vector: vec![1.0, 0.0],
            })
            .collect::<Vec<_>>();
        generation = writer
            .publish_archive_vector_batch(owner, profile, input.source_archive_version(), &rows)
            .unwrap()
            .unwrap();
        writer.container.sync().unwrap();
    }

    let source_container = Container::open_read_only(&source.0).unwrap();
    let work_history = reliquary_insomnia_store::InsomniaStore::work_history(
        &source_container,
        reliquary_insomnia_store::EpisodeId(episode.0),
    )
    .unwrap();
    let old_receipt = source_container
        .read_frames()
        .unwrap()
        .iter()
        .find(|frame| frame.payload.starts_with(b"RELVGRC1"))
        .and_then(|frame| {
            reliquary_vector_store::generation::generation_receipt_target(&frame.payload).unwrap()
        })
        .unwrap();
    drop(source_container);

    let source_bytes = fs::read(&source.0).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let report =
        prune_terminal_memory_copy(&plan, &source.0, &output.0, &[memory], &grant).unwrap();
    assert_eq!(report.pruned_memories, 1);
    assert!(report.transformed_bytes < report.source_bytes);
    assert_eq!(fs::read(&source.0).unwrap(), source_bytes);

    let output_container = Container::open_read_only(&output.0).unwrap();
    let new_receipt = output_container
        .read_frames()
        .unwrap()
        .iter()
        .find(|frame| frame.payload.starts_with(b"RELVGRC1"))
        .and_then(|frame| {
            reliquary_vector_store::generation::generation_receipt_target(&frame.payload).unwrap()
        })
        .unwrap();
    assert_ne!(new_receipt.offset, old_receipt.offset);
    assert_eq!(new_receipt.length, old_receipt.length);
    assert_eq!(
        reliquary_insomnia_store::InsomniaStore::work_history(
            &output_container,
            reliquary_insomnia_store::EpisodeId(episode.0),
        )
        .unwrap(),
        work_history
    );
    drop(output_container);

    let reopened = RelSession::open_path(&output.0, &grant, [], []).unwrap();
    assert_eq!(
        reopened.generations.current_generation(profile),
        Some(generation)
    );
    let mapping = reopened
        .archive_vectors
        .get(generation.archive_vector_id)
        .unwrap();
    assert_eq!(
        reopened
            .common
            .packed_vectors
            .read_f32_row(mapping.packed_vector_id, 0)
            .unwrap(),
        vec![1.0, 0.0]
    );
    let history = reopened.memory_history(memory).unwrap().unwrap();
    assert!(history.tombstone.is_some());
    assert!(matches!(history.revisions[0].payload, PayloadState::Absent));
}

#[test]
fn rel_terminal_memory_prune_reclaims_body_embedded_in_grouped_insomnia_carrier() {
    let source = Temp::new();
    let output = Temp::new();
    let no_manifest = Temp::new();
    let forged_proof = Temp::new();
    let owner = [0xb7; 16];
    let secret = "grouped terminal secret that must be removed ".repeat(900);
    let successor_content = "source-grounded successor retained after terminal pruning";
    let conversation = "g4-grouped-prune";
    let node = "g4-grouped-prune-source";
    let mut writer = RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap();
    writer
        .accept_turn_and_advance_branch(
            TurnInput {
                id: node.into(),
                conversation_id: conversation.into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("maintenance-principal".into()),
                timestamp_ns: 10,
                content: "The accepted source supports two extracted memories.".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
            Branch {
                id: "main".into(),
                conversation_id: conversation.into(),
                leaf_node_id: node.into(),
                canonical: true,
            },
        )
        .unwrap();
    let episode = writer
        .finalize_episode(
            conversation,
            node,
            node,
            reliquary_archive::EpisodeOrigin::Live,
            reliquary_archive::EpisodeBoundary::Explicit,
            11,
        )
        .unwrap();
    let claim = writer
        .claim_next_insomnia_input("maintenance-worker", 12, 100, 1)
        .unwrap()
        .unwrap();
    let old_mutation = "g4-grouped-prune-terminal";
    let new_mutation = "g4-grouped-prune-successor";
    let old_id = memory_id(old_mutation);
    let successor_id = memory_id(new_mutation);
    let old_body = memory_body_id("terminal grouped title", &secret);
    let successor_body = memory_body_id("retained grouped title", successor_content);
    let first_memory_version = writer.read_session().memory_version() + 1;
    let record = |id, body_id, mutation: &str, version| MemoryRecord {
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
        source_node_id: Some(node.into()),
        content_source_conversation_id: Some(conversation.into()),
        content_source_node_id: Some(node.into()),
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: Some(episode.0),
        source_ref: Some(MemorySourceRef {
            owner_id: owner.iter().map(|byte| format!("{byte:02x}")).collect(),
            principal_id: Some("maintenance-principal".into()),
            source_episode_id: episode.0,
            source_node_id: node.into(),
            content_source_conversation_id: Some(conversation.into()),
            content_source_node_id: Some(node.into()),
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
        }),
        source_time_ns: Some(10),
        temporal_inference: None,
        mutation_id: mutation.into(),
        created_at_ns: 12,
        updated_at_ns: 12,
        memory_version: version,
    };
    let old_record = record(old_id, old_body, old_mutation, first_memory_version);
    let successor_record = record(
        successor_id,
        successor_body,
        new_mutation,
        first_memory_version + 1,
    );
    let grouped_records = vec![
        encode_grouped_creation(&old_record).unwrap(),
        encode_grouped_creation(&successor_record).unwrap(),
    ];
    assert!(matches!(
        writer
            .session
            .common
            .memories
            .preflight_grouped_completion(Vec::new(), grouped_records.clone()),
        Err(reliquary_memory_core::MemoryError::Corrupt(
            "grouped creation has no body backing"
        ))
    ));
    let completion_at = 13;
    writer
        .commit_grouped_insomnia_completion(
            claim.work.lease_token.unwrap(),
            GroupedInsomniaCompletion {
                episode_id: episode.0,
                attempt: claim.work.attempt_count,
                started_at_ns: claim.started_at_ns,
                completed_at_ns: completion_at,
                transaction_time_ns: completion_at,
                extractor_model: "g4-maintenance-fixture".into(),
                extractor_version: "v1".into(),
                rejected_count: 0,
                memory_ids: vec![old_id.0, successor_id.0],
                external_memory_refs: vec![],
                global_version_start: writer.next_global_version().unwrap(),
                bodies: vec![
                    GroupedMemoryBody {
                        id: MemoryBodyId(old_body.0),
                        bytes: memory_body_bytes("terminal grouped title", &secret),
                    },
                    GroupedMemoryBody {
                        id: MemoryBodyId(successor_body.0),
                        bytes: memory_body_bytes("retained grouped title", successor_content),
                    },
                ],
                memory_records: grouped_records,
                routing_metadata: vec![],
                freshness_birth_turn: Some(writer.read_session().rel_turn_count()),
                freshness_policy_version: Some(1),
                freshness_birth_memory_ids: vec![old_id.0, successor_id.0],
            },
            completion_at,
        )
        .unwrap();
    let observed = [successor_id, old_id].map(|id| {
        let record = writer.read_session().current_memory(id).unwrap().record;
        crate::DreamMemoryCut {
            id,
            revision: record.revision,
            body_id: record.body_id,
        }
    });
    writer
        .commit_dream_batch(crate::DreamBatchCut {
            owner,
            publication_id: [0xd3; 32],
            graph_version: writer.read_session().graph_version(),
            observed: observed.to_vec(),
            pairs: vec![crate::DreamPairPublication {
                source: successor_id,
                candidate: old_id,
                kind: Some(reliquary_graph::RelationKind::Supersedes),
                rejected: false,
            }],
            changes: vec![reliquary_graph::RelationChange {
                source: reliquary_graph::NodeRef::Memory(successor_id.0),
                target: reliquary_graph::NodeRef::Memory(old_id.0),
                kind: reliquary_graph::RelationKind::Supersedes,
                active: true,
                origin: reliquary_graph::RelationOrigin::Dream,
            }],
            lifecycles: vec![
                crate::DreamLifecyclePublication {
                    memory_id: successor_id,
                    lifecycle_state: "canonical".into(),
                    archived: false,
                    superseded_by: None,
                },
                crate::DreamLifecyclePublication {
                    memory_id: old_id,
                    lifecycle_state: "archived".into(),
                    archived: true,
                    superseded_by: Some(successor_id),
                },
            ],
            updated_at_ns: 14,
        })
        .unwrap();
    let old_current = writer.read_session().current_memory(old_id).unwrap().record;
    let successor_current = writer
        .read_session()
        .current_memory(successor_id)
        .unwrap()
        .record;
    writer
        .retire_memory(crate::MemoryRetirementRequest {
            owner_id: owner,
            memory_id: old_id,
            memory_revision: old_current.revision,
            body_id: old_current.body_id,
            superseded_by: successor_id,
            successor_revision: successor_current.revision,
            successor_body_id: successor_current.body_id,
        })
        .unwrap();
    writer.container.sync().unwrap();
    drop(writer);

    let admission = GraphAdmission::for_replay(owner, [old_id.0, successor_id.0], [], [], [], []);
    let source_bytes = fs::read(&source.0).unwrap();
    let source_container = Container::open_read_only(&source.0).unwrap();
    let source_frames = source_container.read_frames().unwrap();
    assert!(
        source_frames
            .iter()
            .any(|frame| frame.payload.starts_with(b"RELINSC7"))
    );
    assert!(
        !source_frames
            .iter()
            .any(|frame| frame.payload.starts_with(b"RELMBOD1")
                && frame.payload.get(8..40) == Some(&old_body.0))
    );
    let work_history = reliquary_insomnia_store::InsomniaStore::work_history(
        &source_container,
        reliquary_insomnia_store::EpisodeId(episode.0),
    )
    .unwrap();
    let source_rel = RelSession::open_path(&source.0, &admission, [], []).unwrap();
    let source_target = source_rel.memory_history(old_id).unwrap().unwrap();
    let source_successor = source_rel.memory_history(successor_id).unwrap().unwrap();
    let source_receipt = source_rel
        .insomnia_receipt(reliquary_insomnia_store::EpisodeId(episode.0), 1)
        .unwrap()
        .clone();
    drop(source_rel);
    drop(source_container);

    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let report =
        prune_terminal_memory_copy(&plan, &source.0, &output.0, &[old_id], &admission).unwrap();
    assert_eq!(report.pruned_memories, 1);
    assert!(report.transformed_bytes < report.source_bytes);
    assert_eq!(fs::read(&source.0).unwrap(), source_bytes);
    let output_bytes = fs::read(&output.0).unwrap();
    assert!(
        !output_bytes
            .windows(secret.len())
            .any(|part| part == secret.as_bytes())
    );
    let output_container = Container::open_read_only(&output.0).unwrap();
    let output_completion = output_container
        .read_frames()
        .unwrap()
        .into_iter()
        .find(|frame| frame.payload.starts_with(b"RELINSC7"))
        .map(|frame| GroupedInsomniaCompletion::decode(&frame.payload).unwrap())
        .unwrap();
    assert!(
        !output_completion
            .bodies
            .iter()
            .any(|body| body.id == MemoryBodyId(old_body.0))
    );
    assert!(
        output_completion
            .bodies
            .iter()
            .any(|body| body.id == MemoryBodyId(successor_body.0))
    );
    let output_work_history = reliquary_insomnia_store::InsomniaStore::work_history(
        &output_container,
        reliquary_insomnia_store::EpisodeId(episode.0),
    )
    .unwrap();
    assert_eq!(output_work_history, work_history);
    drop(output_container);

    let reopened = RelSession::open_path(&output.0, &admission, [], []).unwrap();
    let pruned_history = reopened.memory_history(old_id).unwrap().unwrap();
    assert_eq!(
        pruned_history
            .revisions
            .iter()
            .map(|revision| &revision.record)
            .collect::<Vec<_>>(),
        source_target
            .revisions
            .iter()
            .map(|revision| &revision.record)
            .collect::<Vec<_>>(),
    );
    assert_eq!(pruned_history.tombstone, source_target.tombstone);
    assert!(
        pruned_history
            .revisions
            .iter()
            .all(|revision| matches!(revision.payload, PayloadState::Absent))
    );
    let retained_history = reopened.memory_history(successor_id).unwrap().unwrap();
    assert_eq!(
        retained_history
            .revisions
            .iter()
            .map(|revision| &revision.record)
            .collect::<Vec<_>>(),
        source_successor
            .revisions
            .iter()
            .map(|revision| &revision.record)
            .collect::<Vec<_>>(),
    );
    let PayloadState::Available(body) = &retained_history.revisions[0].payload else {
        panic!("untargeted grouped Memory body must remain available")
    };
    assert_eq!(body.content, successor_content);
    assert_eq!(
        reopened.insomnia_receipt(reliquary_insomnia_store::EpisodeId(episode.0), 1),
        Some(&source_receipt)
    );
    drop(reopened);

    let pruned_container = Container::open_read_only(&output.0).unwrap();
    let mut without_manifest =
        Container::create(&no_manifest.0, pruned_container.identity(), owner).unwrap();
    for frame in pruned_container.read_frames().unwrap() {
        if !frame.payload.starts_with(b"RELHPR01") {
            without_manifest.append(&frame.payload).unwrap();
        }
    }
    without_manifest.sync().unwrap();
    drop(without_manifest);
    drop(pruned_container);
    assert!(RelSession::open_path(&no_manifest.0, &admission, [], []).is_err());

    let pruned_container = Container::open_read_only(&output.0).unwrap();
    let mut tampered =
        Container::create(&forged_proof.0, pruned_container.identity(), owner).unwrap();
    for frame in pruned_container.read_frames().unwrap() {
        if frame.payload.starts_with(b"RELHPR01") {
            let mut manifest = HistoryPruneManifest::decode(&frame.payload).unwrap();
            manifest.descriptors[0].retained_structure_digest[0] ^= 1;
            tampered.append(&manifest.encode().unwrap()).unwrap();
        } else {
            tampered.append(&frame.payload).unwrap();
        }
    }
    tampered.sync().unwrap();
    drop(tampered);
    drop(pruned_container);
    assert!(RelSession::open_path(&forged_proof.0, &admission, [], []).is_err());
}

#[test]
fn shared_bodies_require_all_terminal_references_and_unknown_frames_block_pruning() {
    for identity in [
        ContainerIdentity::phylactery(),
        ContainerIdentity::reliquary(reliquary_container::ReliquaryScope::Connection),
    ] {
        let source = Temp::new();
        let target = Temp::new();
        let owner = [0xa2; 16];
        let mut container = Container::create(&source.0, identity, owner).unwrap();
        super::super::create::initialize_owner(&mut container).unwrap();
        if identity.file_kind == FileKind::Reliquary {
            reliquary_archive::Archive::open(&container)
                .unwrap()
                .accept_turn(
                    &mut container,
                    reliquary_archive::TurnInput {
                        id: "turn".into(),
                        conversation_id: "conversation".into(),
                        parent_id: None,
                        role: "user".into(),
                        principal_id: None,
                        timestamp_ns: 1,
                        content: "preserve Archive provenance".into(),
                        attachments: vec![],
                        project_attachments: vec![],
                    },
                )
                .unwrap();
        }
        let mut memories = MemoryStore::open(&container).unwrap();
        let body = "identical shared body ".repeat(80);
        let first = memories
            .publish(&mut container, None, 0, draft("shared-a", &body))
            .unwrap()
            .revision
            .record
            .id;
        let second = memories
            .publish(&mut container, None, 0, draft("shared-b", &body))
            .unwrap()
            .revision
            .record
            .id;
        memories.tombstone(&mut container, first, 1).unwrap();
        memories.tombstone(&mut container, second, 1).unwrap();
        container.sync().unwrap();
        drop(memories);
        drop(container);
        let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
        let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
        assert!(prune_terminal_memory_copy(&plan, &source.0, &target.0, &[first], &grant).is_err());
        assert!(!target.0.exists());
        let shared_copy = Temp::new();
        let report =
            prune_terminal_memory_copy(&plan, &source.0, &shared_copy.0, &[first, second], &grant)
                .unwrap();
        assert_eq!(report.pruned_memories, 2);
        let state = match identity.file_kind {
            FileKind::Reliquary => {
                let session = RelSession::open_path(&shared_copy.0, &grant, [], []).unwrap();
                assert_eq!(session.rel_turn_count(), 1);
                session.common
            }
            FileKind::Phylactery => {
                PhySession::open_path(&shared_copy.0, &grant, [])
                    .unwrap()
                    .common
            }
        };
        for id in [first, second] {
            let history = state.memories.history(id).unwrap();
            assert!(history.tombstone.is_some());
            assert!(matches!(history.revisions[0].payload, PayloadState::Absent));
        }
        if identity.file_kind == FileKind::Reliquary {
            let replacement = Temp::new();
            let backup = Temp::new();
            let original = fs::read(&source.0).unwrap();
            fs::write(&replacement.0, &original).unwrap();
            let replacement_plan = VerifiedOwnerCopyPlan::prepare(&replacement.0).unwrap();
            promote_terminal_memory_copy(
                &replacement_plan,
                &replacement.0,
                &backup.0,
                &[first, second],
                &grant,
            )
            .unwrap();
            assert_eq!(fs::read(&backup.0).unwrap(), original);
            let writer = RelWriter::open_path(&replacement.0, grant.clone(), [], []).unwrap();
            assert_eq!(writer.read_session().rel_turn_count(), 1);
        }
        let mut writer = Container::open_writer(&source.0).unwrap();
        writer.append(b"UNKNWN01-opaque-reference").unwrap();
        writer.sync().unwrap();
        drop(writer);
        let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
        assert!(
            prune_terminal_memory_copy(&plan, &source.0, &target.0, &[first, second], &grant)
                .is_err()
        );
        assert!(!target.0.exists());
    }
}
