use super::*;
use reliquary_container::{Container, ContainerIdentity, FileKind, ReliquaryScope};
use reliquary_memory_core::{
    MemoryBodyId, MemoryRecord, memory_body_bytes, memory_body_id, memory_id,
};
use std::{
    fs,
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
            "grouped-completion-{}-{}.rel",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn fixture() -> GroupedInsomniaCompletion {
    let episode_id = [4; 32];
    let body_id = memory_body_id("title", "body");
    let body_bytes = memory_body_bytes("title", "body");
    let record = MemoryRecord {
        id: memory_id("insomnia:episode:one"),
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
        source_node_id: Some("node-1".into()),
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: Some(episode_id),
        source_ref: None,
        source_time_ns: Some(10),
        temporal_inference: None,
        mutation_id: "insomnia:episode:one".into(),
        created_at_ns: 10,
        updated_at_ns: 20,
        memory_version: 1,
    };
    GroupedInsomniaCompletion {
        episode_id,
        attempt: 1,
        started_at_ns: 15,
        completed_at_ns: 20,
        transaction_time_ns: 25,
        extractor_model: "fixture".into(),
        extractor_version: "1".into(),
        rejected_count: 0,
        memory_ids: vec![record.id.0],
        external_memory_refs: vec![],
        global_version_start: 1,
        bodies: vec![GroupedMemoryBody {
            id: MemoryBodyId(body_id.0),
            bytes: body_bytes,
        }],
        memory_records: vec![reliquary_memory_core::encode_grouped_creation(&record).unwrap()],
        routing_metadata: vec![b"ROUTING1opaque".to_vec()],
        freshness_birth_turn: Some(3),
        freshness_policy_version: Some(1),
        freshness_birth_memory_ids: vec![record.id.0],
    }
}

#[test]
fn current_rel_grouped_completion_roundtrips_and_exposes_physical_clock() {
    let completion = fixture();
    let encoded = completion.encode().unwrap();
    assert!(encoded.starts_with(b"RELINSC7"));
    let decoded = GroupedInsomniaCompletion::decode(&encoded).unwrap();
    assert_eq!(decoded, completion);
    assert!(decoded.validate_replay_episode([4; 32]).is_err());
    let mut unrouted = decoded;
    unrouted.routing_metadata.clear();
    unrouted.validate_replay_episode([4; 32]).unwrap();
    assert!(unrouted.validate_replay_episode([5; 32]).is_err());
}

#[test]
fn relinsc7_field_order_matches_the_pinned_v7_carrier_contract() {
    let mut completion = fixture();
    completion
        .external_memory_refs
        .push(GroupedExternalMemoryRef {
            owner_id: "01010101010101010101010101010101".into(),
            memory_id: [7; 32],
        });
    let encoded = completion.encode().unwrap();
    let mut expected = Vec::new();
    expected.extend_from_slice(b"RELINSC7");
    expected.extend_from_slice(&completion.episode_id);
    expected.extend_from_slice(&completion.attempt.to_le_bytes());
    expected.extend_from_slice(&completion.started_at_ns.to_le_bytes());
    expected.extend_from_slice(&completion.completed_at_ns.to_le_bytes());
    expected.extend_from_slice(&completion.rejected_count.to_le_bytes());
    expected.extend_from_slice(&completion.global_version_start.to_le_bytes());
    expected.extend_from_slice(&(completion.memory_records.len() as u32).to_le_bytes());
    expected.extend_from_slice(&completion.transaction_time_ns.to_le_bytes());
    expected.push(1);
    ref_string(&mut expected, &completion.extractor_model);
    ref_string(&mut expected, &completion.extractor_version);
    ref_ids(&mut expected, &completion.memory_ids);
    expected.extend_from_slice(&(completion.external_memory_refs.len() as u32).to_le_bytes());
    for reference in &completion.external_memory_refs {
        ref_string(&mut expected, &reference.owner_id);
        expected.extend_from_slice(&reference.memory_id);
    }
    expected.extend_from_slice(&(completion.bodies.len() as u32).to_le_bytes());
    for body in &completion.bodies {
        expected.extend_from_slice(&body.id.0);
        ref_bytes(&mut expected, &body.bytes);
    }
    expected.extend_from_slice(&(completion.memory_records.len() as u32).to_le_bytes());
    for record in &completion.memory_records {
        ref_bytes(&mut expected, record);
    }
    expected.extend_from_slice(&(completion.routing_metadata.len() as u32).to_le_bytes());
    for metadata in &completion.routing_metadata {
        ref_bytes(&mut expected, metadata);
    }
    expected.extend_from_slice(&completion.freshness_birth_turn.unwrap().to_le_bytes());
    expected.extend_from_slice(&completion.freshness_policy_version.unwrap().to_le_bytes());
    ref_ids(&mut expected, &completion.freshness_birth_memory_ids);
    assert_eq!(encoded, expected);
}

fn ref_string(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
    out.extend_from_slice(text.as_bytes());
}
fn ref_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}
fn ref_ids(out: &mut Vec<u8>, ids: &[[u8; 32]]) {
    out.extend_from_slice(&(ids.len() as u32).to_le_bytes());
    for id in ids {
        out.extend_from_slice(id);
    }
}

#[test]
fn malformed_freshness_cohort_or_record_episode_rejects_before_encoding() {
    let mut completion = fixture();
    completion.freshness_birth_memory_ids.clear();
    assert!(completion.encode().is_err());

    let mut encoded = fixture().encode().unwrap();
    // Birth identity appears in the trailing cohort; alter it while retaining framing.
    let last = encoded.len() - 1;
    encoded[last] ^= 1;
    assert!(GroupedInsomniaCompletion::decode(&encoded).is_err());
}

#[test]
fn routed_replay_accepts_external_refs_without_folding_them_into_local_ids() {
    let mut completion = fixture();
    completion.routing_metadata.clear();
    completion
        .external_memory_refs
        .push(GroupedExternalMemoryRef {
            owner_id: "02020202020202020202020202020202".into(),
            memory_id: [9; 32],
        });
    let decoded = GroupedInsomniaCompletion::decode(&completion.encode().unwrap()).unwrap();
    decoded.validate_replay_episode([4; 32]).unwrap();
    assert_eq!(decoded.memory_ids, completion.memory_ids);
    assert_eq!(
        decoded.external_memory_refs,
        completion.external_memory_refs
    );
    assert!(decoded.validate_replay_episode([5; 32]).is_err());

    let mut overlap = completion.clone();
    overlap.external_memory_refs[0].memory_id = completion.memory_ids[0];
    assert!(overlap.encode().is_err());

    let mut empty_owner = completion;
    empty_owner.external_memory_refs[0].owner_id.clear();
    assert!(empty_owner.encode().is_err());

    let mut uppercase_owner = GroupedInsomniaCompletion {
        external_memory_refs: vec![GroupedExternalMemoryRef {
            owner_id: "ABABABABABABABABABABABABABABABAB".into(),
            memory_id: [9; 32],
        }],
        ..fixture()
    };
    assert!(uppercase_owner.encode().is_err());
    uppercase_owner.external_memory_refs[0].owner_id = "02020202020202020202020202020202".into();
    assert!(uppercase_owner.encode().is_ok());

    let mut completion = fixture();
    completion.routing_metadata = vec![b"ROUTING1opaque".to_vec()];
    let decoded = GroupedInsomniaCompletion::decode(&completion.encode().unwrap()).unwrap();
    assert!(decoded.validate_replay_episode([4; 32]).is_err());
}

#[test]
fn container_replays_the_grouped_carriers_embedded_physical_range() {
    let path = Temp::new();
    let mut container = Container::create(
        &path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [3; 16],
    )
    .unwrap();
    let completion = fixture();
    container.append(&completion.encode().unwrap()).unwrap();
    container.sync().unwrap();
    drop(container);
    let reopened = Container::open_read_only_as(&path.0, FileKind::Reliquary).unwrap();
    assert_eq!(reopened.latest_version(), 1);
    assert_eq!(reopened.transaction_time_ns(1), Some(25));
}

#[test]
fn rel_session_replays_grouped_insomnia_receipt_instead_of_dropping_the_carrier() {
    let path = Temp::new();
    let mut container = Container::create(
        &path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [3; 16],
    )
    .unwrap();
    reliquary_vector_store::PackedVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    reliquary_vector_store::CompatibilityProfileStore::default()
        .initialize(&mut container)
        .unwrap();
    reliquary_vector_store::ArchiveVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    reliquary_vector_store::MemoryVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    reliquary_vector_store::VectorGenerationStore::empty()
        .initialize(&mut container)
        .unwrap();
    reliquary_memory_core::MemoryStore::create(&mut container).unwrap();
    let mut existing =
        reliquary_memory_core::decode_grouped_creation(&fixture().memory_records[0]).unwrap();
    existing.mutation_id = "previous-memory".into();
    existing.id = memory_id(&existing.mutation_id);
    container
        .append(&reliquary_memory_core::encode_grouped_creation(&existing).unwrap())
        .unwrap();
    reliquary_freshness_store::FreshnessStore::new(Some([3; 16]))
        .initialize(&mut container)
        .unwrap();
    container.sync().unwrap();
    let mut archive = reliquary_archive::Archive::empty();
    archive
        .accept_turn(
            &mut container,
            reliquary_archive::TurnInput {
                id: "node-1".into(),
                conversation_id: "conversation".into(),
                parent_id: None,
                role: "assistant".into(),
                principal_id: None,
                timestamp_ns: 12,
                content: "source".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
        )
        .unwrap();
    let episode_id = archive
        .put_episode(
            &mut container,
            "conversation",
            "node-1",
            "node-1",
            reliquary_archive::EpisodeOrigin::Live,
            reliquary_archive::EpisodeBoundary::CreateMemory,
            20,
        )
        .unwrap();
    let work_episode = reliquary_insomnia_store::EpisodeId(episode_id.0);
    let accepted_order = reliquary_insomnia_store::AcceptedEpisodeOrder {
        episode_id: work_episode,
        source_through_ns: 12,
        conversation_id: "conversation".into(),
        start_node_id: "node-1".into(),
    };
    let mut insomnia =
        reliquary_insomnia_store::InsomniaStore::open(&container, [accepted_order.clone()])
            .unwrap();
    insomnia
        .queue(
            &mut container,
            accepted_order.clone(),
            reliquary_insomnia_store::InsomniaPriority::Live,
            1,
        )
        .unwrap();
    let claim = insomnia
        .claim_next(&mut container, "test-worker", 2, 100)
        .unwrap()
        .unwrap();
    let token = claim.lease_token.unwrap();
    let mut completion = fixture();
    completion.episode_id = episode_id.0;
    completion.global_version_start = container.latest_version() + 1;
    completion.freshness_birth_turn = Some(1);
    completion.routing_metadata.clear();
    let mut memory =
        reliquary_memory_core::decode_grouped_creation(&completion.memory_records[0]).unwrap();
    memory.source_episode_id = Some(episode_id.0);
    memory.memory_version = 2;
    completion.memory_ids.push(existing.id.0);
    completion.memory_records[0] = reliquary_memory_core::encode_grouped_creation(&memory).unwrap();
    let mut stale_session = crate::RelSession::open(
        &container,
        &reliquary_graph::GraphAdmission::for_replay([3; 16], [], [], [], [], []),
        [accepted_order.clone()],
        [],
    )
    .unwrap();
    let mut untrusted_routed_completion = completion.clone();
    untrusted_routed_completion.memory_ids.push([99; 32]);
    untrusted_routed_completion
        .external_memory_refs
        .push(GroupedExternalMemoryRef {
            owner_id: "unverified-phy-owner".into(),
            memory_id: [99; 32],
        });
    let before_untrusted_route = container.latest_version();
    let mut absent_memory = completion.clone();
    absent_memory.memory_ids.push([98; 32]);
    assert!(
        stale_session
            .commit_grouped_insomnia_completion(&mut container, token, absent_memory, 20)
            .is_err()
    );
    assert_eq!(container.latest_version(), before_untrusted_route);
    assert!(
        stale_session
            .commit_grouped_insomnia_completion(
                &mut container,
                token,
                untrusted_routed_completion,
                20,
            )
            .is_err()
    );
    assert_eq!(container.latest_version(), before_untrusted_route);

    let foreign_path = Temp::new();
    let mut same_owner_other_file = Container::create(
        &foreign_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [3; 16],
    )
    .unwrap();
    let other_file_version = same_owner_other_file.latest_version();
    assert!(
        stale_session
            .commit_grouped_insomnia_completion(
                &mut same_owner_other_file,
                token,
                completion.clone(),
                20,
            )
            .is_err()
    );
    assert_eq!(same_owner_other_file.latest_version(), other_file_version);
    drop(same_owner_other_file);

    container.append(b"UNRELATED").unwrap();
    container.sync().unwrap();
    assert!(
        stale_session
            .commit_grouped_insomnia_completion(&mut container, token, completion.clone(), 20)
            .is_err()
    );
    drop(stale_session);

    let mut session = crate::RelSession::open(
        &container,
        &reliquary_graph::GraphAdmission::for_replay([3; 16], [], [], [], [], []),
        [accepted_order.clone()],
        [],
    )
    .unwrap();
    let version_before_preflight_failure = container.latest_version();
    assert!(
        session
            .commit_grouped_insomnia_completion(
                &mut container,
                reliquary_insomnia_store::InsomniaLeaseToken([0; 32]),
                completion.clone(),
                20,
            )
            .is_err()
    );
    assert_eq!(container.latest_version(), version_before_preflight_failure);
    session.inject_grouped_projection_failure();
    assert!(
        session
            .commit_grouped_insomnia_completion(&mut container, token, completion.clone(), 20)
            .is_err()
    );
    let version_after_synced_carrier = container.latest_version();
    assert!(
        session
            .commit_grouped_insomnia_completion(&mut container, token, completion.clone(), 20)
            .is_err()
    );
    assert_eq!(container.latest_version(), version_after_synced_carrier);
    // Reordering a valid Episode after the grouped carrier must not turn
    // future source evidence into authority at the earlier physical cut.
    let frames = container.read_frames().unwrap();
    let routed_path = Temp::new();
    let mut routed_copy = Container::create(
        &routed_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [3; 16],
    )
    .unwrap();
    for frame in &frames {
        if frame.payload.starts_with(b"RELINSC7") {
            let mut carrier = GroupedInsomniaCompletion::decode(&frame.payload).unwrap();
            carrier.external_memory_refs.push(GroupedExternalMemoryRef {
                owner_id: "04040404040404040404040404040404".into(),
                memory_id: [9; 32],
            });
            routed_copy.append(&carrier.encode().unwrap()).unwrap();
        } else {
            routed_copy.append(&frame.payload).unwrap();
        }
    }
    routed_copy.sync().unwrap();
    drop(routed_copy);
    let routed_copy = Container::open_read_only(&routed_path.0).unwrap();
    let routed_session = crate::RelSession::open(
        &routed_copy,
        &reliquary_graph::GraphAdmission::for_replay([3; 16], [], [], [], [], []),
        [accepted_order.clone()],
        [],
    )
    .unwrap();
    let routed_receipt = routed_session.insomnia_receipt(work_episode, 1).unwrap();
    assert_eq!(routed_receipt.memory_ids.len(), 2);
    assert_eq!(routed_receipt.external_memory_refs.len(), 1);
    assert_eq!(
        routed_receipt.external_memory_refs[0].owner_id,
        "04040404040404040404040404040404"
    );
    drop(routed_session);
    drop(routed_copy);

    let local_ref_path = Temp::new();
    let mut local_ref_copy = Container::create(
        &local_ref_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [3; 16],
    )
    .unwrap();
    for frame in &frames {
        if frame.payload.starts_with(b"RELINSC7") {
            let mut carrier = GroupedInsomniaCompletion::decode(&frame.payload).unwrap();
            carrier.external_memory_refs.push(GroupedExternalMemoryRef {
                owner_id: "03030303030303030303030303030303".into(),
                memory_id: [9; 32],
            });
            local_ref_copy.append(&carrier.encode().unwrap()).unwrap();
        } else {
            local_ref_copy.append(&frame.payload).unwrap();
        }
    }
    local_ref_copy.sync().unwrap();
    drop(local_ref_copy);
    let local_ref_copy = Container::open_read_only(&local_ref_path.0).unwrap();
    assert!(matches!(
        crate::RelSession::open(
            &local_ref_copy,
            &reliquary_graph::GraphAdmission::for_replay([3; 16], [], [], [], [], []),
            [accepted_order.clone()],
            [],
        ),
        Err(crate::OwnerSessionError::Record(message))
            if message.contains("local REL owner")
    ));
    drop(local_ref_copy);

    let forged_path = Temp::new();
    let mut forged = Container::create(
        &forged_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [3; 16],
    )
    .unwrap();
    let episode_frame = frames
        .iter()
        .find(|frame| frame.payload.starts_with(b"RELEPIS1"))
        .unwrap();
    for frame in &frames {
        if !frame.payload.starts_with(b"RELEPIS1") {
            forged.append(&frame.payload).unwrap();
        }
    }
    forged.append(&episode_frame.payload).unwrap();
    forged.sync().unwrap();
    drop(forged);
    let forged = Container::open_read_only(&forged_path.0).unwrap();
    let admission = reliquary_graph::GraphAdmission::for_replay([3; 16], [], [], [], [], []);
    assert!(
        matches!(crate::RelSession::open(&forged, &admission, [], []), Err(crate::OwnerSessionError::Record(message)) if message.contains("Episode is not admitted"))
    );
    let wrong_turn_path = Temp::new();
    let mut wrong_turn = Container::create(
        &wrong_turn_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [3; 16],
    )
    .unwrap();
    for frame in &frames {
        if frame.payload.starts_with(b"RELINSC7") {
            let mut carrier = GroupedInsomniaCompletion::decode(&frame.payload).unwrap();
            carrier.freshness_birth_turn = Some(2);
            wrong_turn.append(&carrier.encode().unwrap()).unwrap();
        } else {
            wrong_turn.append(&frame.payload).unwrap();
        }
    }
    wrong_turn.sync().unwrap();
    drop(wrong_turn);
    let wrong_turn = Container::open_read_only(&wrong_turn_path.0).unwrap();
    assert!(
        matches!(crate::RelSession::open(&wrong_turn, &admission, [], []), Err(crate::OwnerSessionError::Record(message)) if message.contains("Freshness turn differs"))
    );
    let future_memory_path = Temp::new();
    let mut future_memory = Container::create(
        &future_memory_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [3; 16],
    )
    .unwrap();
    let creation = frames
        .iter()
        .find(|frame| frame.payload.starts_with(b"RELMCRT1"))
        .unwrap();
    for frame in &frames {
        if !frame.payload.starts_with(b"RELMCRT1") {
            future_memory.append(&frame.payload).unwrap();
        }
    }
    future_memory.append(&creation.payload).unwrap();
    future_memory.sync().unwrap();
    drop(future_memory);
    let future_memory = Container::open_read_only(&future_memory_path.0).unwrap();
    assert!(
        matches!(crate::RelSession::open(&future_memory, &admission, [], []), Err(crate::OwnerSessionError::Record(message)) if message.contains("Memory outside its physical prefix"))
    );
    drop(session);
    drop(container);

    let mut reopened = Container::open_read_only_as(&path.0, FileKind::Reliquary).unwrap();
    let mut session = crate::RelSession::open(
        &reopened,
        &reliquary_graph::GraphAdmission::for_replay([3; 16], [], [], [], [], []),
        [accepted_order.clone()],
        [],
    )
    .unwrap();
    let receipt = session.insomnia_receipt(work_episode, 1).unwrap();
    assert_eq!(receipt.memory_ids.len(), 2);
    assert!(
        receipt
            .memory_ids
            .contains(&reliquary_insomnia_store::MemoryId(existing.id.0))
    );
    assert_eq!(receipt.completed_at_ns, 20);
    let existing_history = session.memory_history(existing.id).unwrap().unwrap();
    assert!(matches!(
        existing_history.revisions[0].payload,
        reliquary_memory_core::PayloadState::Available(_)
    ));
    assert!(
        session
            .memory_history(reliquary_memory_core::MemoryId(memory.id.0))
            .unwrap()
            .is_some()
    );
    assert!(
        session
            .commit_grouped_insomnia_completion(&mut reopened, token, completion.clone(), 20,)
            .is_err()
    );
    assert!(
        session
            .freshness_record(reliquary_freshness_store::FreshnessMemoryId(memory.id.0))
            .unwrap()
            .is_some()
    );
    drop(reopened);
    drop(session);
    let mut retry_writer = Container::open_writer(&path.0).unwrap();
    let mut replayed_session = crate::RelSession::open(
        &retry_writer,
        &reliquary_graph::GraphAdmission::for_replay([3; 16], [], [], [], [], []),
        [accepted_order],
        [],
    )
    .unwrap();
    assert_eq!(
        replayed_session
            .commit_grouped_insomnia_completion(&mut retry_writer, token, completion, 20,)
            .unwrap(),
        reliquary_insomnia_store::CompletionCheck::AlreadyAccepted
    );
}

#[test]
fn dream_pair_writers_reopen_and_retry_without_advancing_clock() {
    for phy in [false, true] {
        let path = Temp::new();
        let owner = [0x68; 16];
        if phy {
            drop(crate::PhyWriter::create_path(&path.0, owner).unwrap());
        } else {
            drop(crate::RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap());
        }
        let mut container = Container::open_writer(&path.0).unwrap();
        let mut first =
            reliquary_memory_core::decode_grouped_creation(&fixture().memory_records[0]).unwrap();
        first.mutation_id = "writer-pair-first".into();
        first.id = memory_id(&first.mutation_id);
        let mut second = first.clone();
        second.mutation_id = "writer-pair-second".into();
        second.id = memory_id(&second.mutation_id);
        second.memory_version = 2;
        container
            .append(&reliquary_memory_core::encode_grouped_creation(&first).unwrap())
            .unwrap();
        container
            .append(&reliquary_memory_core::encode_grouped_creation(&second).unwrap())
            .unwrap();
        let mut partial_mapping = b"RELDRM01".to_vec();
        partial_mapping.extend_from_slice(&1u32.to_le_bytes());
        partial_mapping.extend_from_slice(&first.id.min(second.id).0);
        container.append(&partial_mapping).unwrap();
        container.sync().unwrap();
        drop(container);
        let before_size = std::fs::metadata(&path.0).unwrap().len();
        let admission = reliquary_graph::GraphAdmission::for_replay(owner, [], [], [], [], []);
        if phy {
            let mut writer = crate::PhyWriter::open_path(&path.0, &admission, []).unwrap();
            assert!(writer.record_dream_pair(first.id, second.id).unwrap());
            assert!(!writer.record_dream_pair(second.id, first.id).unwrap());
            assert!(writer.record_dream_pair(first.id, first.id).is_err());
            assert!(
                writer
                    .record_dream_pair(first.id, reliquary_memory_core::MemoryId([0xff; 32]))
                    .is_err()
            );
        } else {
            let mut writer =
                crate::RelWriter::open_path(&path.0, admission.clone(), [], []).unwrap();
            assert!(writer.record_dream_pair(first.id, second.id).unwrap());
            assert!(!writer.record_dream_pair(second.id, first.id).unwrap());
            assert!(writer.record_dream_pair(first.id, first.id).is_err());
            assert!(
                writer
                    .record_dream_pair(first.id, reliquary_memory_core::MemoryId([0xff; 32]))
                    .is_err()
            );
        }
        let before = std::fs::read(&path.0).unwrap();
        assert!(before.len() as u64 > before_size);
        let container = Container::open_read_only(&path.0).unwrap();
        assert_eq!(container.latest_version(), 0);
        if phy {
            assert!(
                crate::PhySession::open(&container, &admission, [])
                    .unwrap()
                    .dream_pair_evaluated(second.id, first.id)
            );
        } else {
            assert!(
                crate::RelSession::open(&container, &admission, [], [])
                    .unwrap()
                    .dream_pair_evaluated(second.id, first.id)
            );
        }
        drop(container);
        assert_eq!(std::fs::read(&path.0).unwrap(), before);
    }
}

#[test]
fn dream_pair_history_is_symmetric_owner_bound_and_prefix_checked() {
    let path = Temp::new();
    let owner = [0x67; 16];
    let mut container = Container::create(&path.0, ContainerIdentity::phylactery(), owner).unwrap();
    let mut first =
        reliquary_memory_core::decode_grouped_creation(&fixture().memory_records[0]).unwrap();
    first.mutation_id = "pair-first".into();
    first.id = memory_id(&first.mutation_id);
    let mut second = first.clone();
    second.mutation_id = "pair-second".into();
    second.id = memory_id(&second.mutation_id);
    second.memory_version = 2;
    let (left, right) = if first.id < second.id {
        (first.id, second.id)
    } else {
        (second.id, first.id)
    };
    let mut mapping_left = b"RELDRM01".to_vec();
    mapping_left.extend_from_slice(&1u32.to_le_bytes());
    mapping_left.extend_from_slice(&left.0);
    let mut mapping_right = b"RELDRM01".to_vec();
    mapping_right.extend_from_slice(&2u32.to_le_bytes());
    mapping_right.extend_from_slice(&right.0);
    let mut pair = b"RELDRP02".to_vec();
    pair.extend_from_slice(&1u32.to_le_bytes());
    pair.extend_from_slice(&2u32.to_le_bytes());
    container
        .append(&reliquary_memory_core::encode_grouped_creation(&first).unwrap())
        .unwrap();
    container.append(&pair).unwrap();
    assert!(crate::session::dream_pairs::DreamPairs::open(&container).is_err());
    drop(container);
    let other = Temp::new();
    let mut container =
        Container::create(&other.0, ContainerIdentity::phylactery(), owner).unwrap();
    container
        .append(&reliquary_memory_core::encode_grouped_creation(&first).unwrap())
        .unwrap();
    container
        .append(&reliquary_memory_core::encode_grouped_creation(&second).unwrap())
        .unwrap();
    container.append(&mapping_left).unwrap();
    container.append(&mapping_right).unwrap();
    container.append(&pair).unwrap();
    container.sync().unwrap();
    drop(container);
    let container = Container::open_read_only(&other.0).unwrap();
    let history = crate::session::dream_pairs::DreamPairs::open(&container).unwrap();
    assert!(history.contains(first.id, second.id));
    assert!(history.contains(second.id, first.id));
    assert!(!history.contains(first.id, first.id));
    drop(container);
    let forged = Temp::new();
    let mut container =
        Container::create(&forged.0, ContainerIdentity::phylactery(), [9; 16]).unwrap();
    container.append(&pair).unwrap();
    assert!(crate::session::dream_pairs::DreamPairs::open(&container).is_err());
}

#[test]
fn open_import_epoch_fences_direct_grouped_completion() {
    let path = Temp::new();
    let owner = [0x61; 16];
    drop(crate::RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap());
    let mut container = Container::open_writer(&path.0).unwrap();
    let mut imports = crate::records::ImportEpochStore::open(&container).unwrap();
    let key = "bc".repeat(32);
    imports
        .append_start(
            &mut container,
            crate::records::ImportStart {
                format_version: 2,
                owner_uuid: owner,
                import_id: crate::records::import_id(owner, &key),
                import_key: key,
                source_namespace: "source/account".into(),
                source_format: "json-v1".into(),
                started_at_ns: 1,
                pre: crate::records::ImportWatermarks::default(),
            },
        )
        .unwrap();
    let admission = reliquary_graph::GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut session = crate::RelSession::open(&container, &admission, [], []).unwrap();
    let before = container.read_frames().unwrap().len();
    assert!(matches!(session.commit_grouped_insomnia_completion(
        &mut container, reliquary_insomnia_store::InsomniaLeaseToken([0; 32]), fixture(), 2),
        Err(crate::OwnerSessionError::Record(message)) if message.contains("fenced by an open import epoch")));
    assert_eq!(container.read_frames().unwrap().len(), before);
}

#[test]
fn owner_graph_reopens_principal_association_from_entity_history() {
    let path = Temp::new();
    let owner = [0x62; 16];
    drop(crate::RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap());
    let mut container = Container::open_writer(&path.0).unwrap();
    let record =
        reliquary_memory_core::decode_grouped_creation(&fixture().memory_records[0]).unwrap();
    container
        .append(&reliquary_memory_core::encode_grouped_creation(&record).unwrap())
        .unwrap();
    let mut perception = reliquary_perception_store::PerceptionStore::open(&container).unwrap();
    let principal = perception
        .publish_entity(
            &mut container,
            reliquary_perception_store::EntityDraft {
                canonical_name: "phy-00000000-0000-0000-0000-000000000123".into(),
                aliases: vec![],
                kind: reliquary_perception_store::PRINCIPAL_ENTITY_KIND.into(),
                summary: String::new(),
                mutation_id: "principal-one".into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
        )
        .unwrap();
    let admitted = reliquary_graph::GraphAdmission::for_changes(
        owner,
        [record.id.0],
        [(principal.id.0, true)],
    );
    let mut graph = reliquary_graph::GraphStore::open(&container, &admitted).unwrap();
    graph
        .put(
            &mut container,
            &admitted,
            &[reliquary_graph::RelationChange {
                source: reliquary_graph::NodeRef::Memory(record.id.0),
                target: reliquary_graph::NodeRef::Entity(principal.id.0),
                kind: reliquary_graph::RelationKind::PrincipalAssociation,
                active: true,
                origin: reliquary_graph::RelationOrigin::User,
            }],
        )
        .unwrap();
    assert!(
        graph
            .outgoing(reliquary_graph::NodeRef::Memory(record.id.0))
            .is_empty()
    );
    assert!(
        graph
            .incoming(reliquary_graph::NodeRef::Entity(principal.id.0))
            .is_empty()
    );
    assert!(graph.active_memory_nodes().is_empty());
    drop(container);
    let reopened = Container::open_read_only(&path.0).unwrap();
    let no_caller_proof = reliquary_graph::GraphAdmission::for_replay(owner, [], [], [], [], []);
    assert_eq!(
        crate::RelSession::open(&reopened, &no_caller_proof, [], [])
            .unwrap()
            .graph_version(),
        1
    );
}

#[test]
fn owner_session_accepts_graph_with_owner_backed_memory_endpoints() {
    let path = Temp::new();
    let owner = [0x63; 16];
    let mut container = Container::create(
        &path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    reliquary_vector_store::PackedVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    reliquary_vector_store::CompatibilityProfileStore::default()
        .initialize(&mut container)
        .unwrap();
    reliquary_vector_store::ArchiveVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    reliquary_vector_store::MemoryVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    reliquary_vector_store::VectorGenerationStore::empty()
        .initialize(&mut container)
        .unwrap();
    reliquary_memory_core::MemoryStore::create(&mut container).unwrap();
    let mut first =
        reliquary_memory_core::decode_grouped_creation(&fixture().memory_records[0]).unwrap();
    first.mutation_id = "graph-first".into();
    first.id = memory_id(&first.mutation_id);
    let mut second = first.clone();
    second.mutation_id = "graph-second".into();
    second.id = memory_id(&second.mutation_id);
    second.memory_version = 2;
    let mut body = b"RELMBOD1".to_vec();
    body.extend_from_slice(&first.body_id.0);
    body.extend_from_slice(&memory_body_bytes("title", "body"));
    container.append(&body).unwrap();
    container
        .append(&reliquary_memory_core::encode_grouped_creation(&first).unwrap())
        .unwrap();
    container
        .append(&reliquary_memory_core::encode_grouped_creation(&second).unwrap())
        .unwrap();
    let admission =
        reliquary_graph::GraphAdmission::for_changes(owner, [first.id.0, second.id.0], []);
    let mut graph = reliquary_graph::GraphStore::open(&container, &admission).unwrap();
    graph
        .put(
            &mut container,
            &admission,
            &[reliquary_graph::RelationChange {
                source: reliquary_graph::NodeRef::Memory(first.id.0),
                target: reliquary_graph::NodeRef::Memory(second.id.0),
                kind: reliquary_graph::RelationKind::Topical,
                active: true,
                origin: reliquary_graph::RelationOrigin::User,
            }],
        )
        .unwrap();
    let mut perception = reliquary_perception_store::PerceptionStore::open(&container).unwrap();
    let key = reliquary_perception_store::MemoryEntityMentionKey {
        memory_id: first.id.0,
        field: reliquary_perception_store::MemoryTextField::Content,
        start_byte: 0,
        end_byte: 4,
    };
    let status = reliquary_perception_store::MemoryEntityResolutionStatus::Unresolved {
        reason: reliquary_perception_store::EntityResolutionReason::InsufficientEvidence,
    };
    perception
        .put_mention_resolution(&mut container, key, 0, 1, status.clone())
        .unwrap();
    perception
        .put_mention_resolution(&mut container, key, 1, 2, status.clone())
        .unwrap();
    let mut members = vec![first.id.0, second.id.0];
    members.sort();
    let mut communities = reliquary_graph::CommunityStore::open(&container, &graph).unwrap();
    communities
        .publish(
            &mut container,
            &graph,
            reliquary_graph::CommunitySnapshot {
                generation: 1,
                graph_version: 1,
                communities: vec![reliquary_graph::Community {
                    id: reliquary_graph::community_id(owner, &members),
                    members,
                }],
            },
        )
        .unwrap();
    drop(container);
    let reopened = Container::open_read_only(&path.0).unwrap();
    let empty_admission = reliquary_graph::GraphAdmission::for_replay(owner, [], [], [], [], []);
    let session = crate::RelSession::open(&reopened, &empty_admission, [], []).unwrap();
    assert_eq!(session.graph_version(), 1);
    let frames = reopened.read_frames().unwrap();
    let graph_frame = frames
        .iter()
        .find(|frame| frame.payload.starts_with(b"RELGRF01"))
        .unwrap();
    let community_frame = frames
        .iter()
        .find(|frame| frame.payload.starts_with(b"RELCOM01"))
        .unwrap();
    let community_path = Temp::new();
    let mut early_community = Container::create(
        &community_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    for frame in &frames {
        if frame.object.offset == graph_frame.object.offset {
            early_community.append(&community_frame.payload).unwrap();
        }
        if frame.object.offset != community_frame.object.offset {
            early_community.append(&frame.payload).unwrap();
        }
    }
    early_community.sync().unwrap();
    drop(early_community);
    let early_community = Container::open_read_only(&community_path.0).unwrap();
    assert!(
        matches!(crate::RelSession::open(&early_community, &empty_admission, [], []), Err(crate::OwnerSessionError::Record(message)) if message.contains("Community references a Graph cut outside its physical prefix"))
    );
    let forged_path = Temp::new();
    let mut forged = Container::create(
        &forged_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    for frame in &frames {
        if frame.payload.starts_with(b"RELMCRT1")
            && reliquary_memory_core::decode_grouped_creation(&frame.payload)
                .unwrap()
                .id
                == first.id
        {
            forged.append(&graph_frame.payload).unwrap();
        }
        if !frame.payload.starts_with(b"RELGRF01") {
            forged.append(&frame.payload).unwrap();
        }
    }
    forged.sync().unwrap();
    drop(forged);
    let forged = Container::open_read_only(&forged_path.0).unwrap();
    assert!(
        matches!(crate::RelSession::open(&forged, &empty_admission, [], []), Err(crate::OwnerSessionError::Record(message)) if message.contains("Graph endpoint is absent from its physical prefix"))
    );
    let mention_path = Temp::new();
    let mut early_mention = Container::create(
        &mention_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    let mention = frames
        .iter()
        .find(|frame| frame.payload.starts_with(b"RELPER01"))
        .unwrap();
    for frame in &frames {
        if frame.payload.starts_with(b"RELMCRT1")
            && reliquary_memory_core::decode_grouped_creation(&frame.payload)
                .unwrap()
                .id
                == first.id
        {
            early_mention.append(&mention.payload).unwrap();
        }
        if frame.object.offset != mention.object.offset {
            early_mention.append(&frame.payload).unwrap();
        }
    }
    early_mention.sync().unwrap();
    drop(early_mention);
    let early_mention = Container::open_read_only(&mention_path.0).unwrap();
    assert!(
        matches!(crate::RelSession::open(&early_mention, &empty_admission, [], []), Err(crate::OwnerSessionError::Record(message)) if message.contains("Entity mention references a Memory outside its physical prefix"))
    );
    let dream = crate::DreamBeginAdmission {
        owner_uuid: owner,
        pass_id: "owner-backed".into(),
        source_id: first.id,
        turn: 0,
        graph_version_before: 1,
        preexisting_ids: [second.id].into_iter().collect(),
        memory_cut: 2,
    };
    assert!(crate::RelSession::open(&reopened, &empty_admission, [], [dream.clone()]).is_ok());
    let mut invalid = dream.clone();
    invalid.memory_cut = 3;
    assert!(crate::RelSession::open(&reopened, &empty_admission, [], [invalid]).is_err());
    let mut invalid = dream.clone();
    invalid.memory_cut = 1;
    assert!(crate::RelSession::open(&reopened, &empty_admission, [], [invalid]).is_err());
    let mut invalid = dream.clone();
    invalid.graph_version_before = 2;
    assert!(crate::RelSession::open(&reopened, &empty_admission, [], [invalid]).is_err());
    let mut invalid = dream;
    invalid
        .preexisting_ids
        .insert(reliquary_memory_core::MemoryId([0xef; 32]));
    assert!(crate::RelSession::open(&reopened, &empty_admission, [], [invalid]).is_err());
    assert_eq!(session.mention_revision(key, 1).unwrap().updated_at_ns, 1);
    assert_eq!(session.mention(key).unwrap().revision, 2);
    drop(session);
    drop(reopened);
    let mut writer = Container::open_writer(&path.0).unwrap();
    let mut perception = reliquary_perception_store::PerceptionStore::open(&writer).unwrap();
    perception
        .put_mention_resolution(
            &mut writer,
            reliquary_perception_store::MemoryEntityMentionKey {
                end_byte: 99,
                ..key
            },
            0,
            2,
            status,
        )
        .unwrap();
    assert!(crate::RelSession::open(&writer, &admission, [], []).is_err());
}

#[test]
fn memory_vector_mapping_requires_retained_owner_body_identity() {
    use lodestone_packed::{PackedVectors, ScalarType, VectorSchema};
    use reliquary_vector_store::{
        AcceptedMemoryMapping, CompatibilityProbeReference, EmbeddingMode, VectorNormalization,
    };
    let path = Temp::new();
    let owner = [0x66; 16];
    let mut container = Container::create(
        &path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    reliquary_memory_core::MemoryStore::create(&mut container).unwrap();
    let record =
        reliquary_memory_core::decode_grouped_creation(&fixture().memory_records[0]).unwrap();
    container
        .append(&reliquary_memory_core::encode_grouped_creation(&record).unwrap())
        .unwrap();
    let mut packed = reliquary_vector_store::PackedVectorStore::default();
    packed.initialize(&mut container).unwrap();
    let vector = PackedVectors::from_bytes(
        VectorSchema::new(1, ScalarType::F32).unwrap(),
        1.0f32.to_le_bytes().to_vec(),
    )
    .unwrap();
    let packed_id = packed.put(&mut container, vector).unwrap();
    let mut profiles = reliquary_vector_store::CompatibilityProfileStore::default();
    profiles.initialize(&mut container).unwrap();
    let mut profile = reliquary_vector_store::CompatibilityProfile {
        id: reliquary_vector_store::CompatibilityProfileId([0; 32]),
        dimensions: 1,
        normalization: VectorNormalization::None,
        probe_suite_version: 1,
        compatibility_policy_version: 2,
        references: [
            EmbeddingMode::Query,
            EmbeddingMode::Query,
            EmbeddingMode::Document,
            EmbeddingMode::Document,
        ]
        .into_iter()
        .map(|mode| CompatibilityProbeReference {
            mode,
            vector: vec![1.5],
        })
        .collect(),
    };
    profile.id = reliquary_vector_store::profile::compatibility_profile_id(&profile);
    profiles.put(&mut container, profile.clone()).unwrap();
    let mut vectors = reliquary_vector_store::MemoryVectorStore::default();
    vectors.initialize(&mut container).unwrap();
    reliquary_vector_store::ArchiveVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    reliquary_vector_store::VectorGenerationStore::empty()
        .initialize(&mut container)
        .unwrap();
    let mapping = |body| AcceptedMemoryMapping {
        profile: profile.id,
        packed: packed_id,
        packed_rows: 1,
        body_ids: vec![reliquary_vector_store::MemoryBodyId(body)],
    };
    vectors
        .put(
            &mut container,
            &packed,
            &profiles,
            mapping(record.body_id.0),
        )
        .unwrap();
    let admission = reliquary_graph::GraphAdmission::for_replay(owner, [], [], [], [], []);
    let session = crate::RelSession::open(&container, &admission, [], []).unwrap();
    assert_eq!(
        session
            .memory_vector_row(profile.id, record.body_id)
            .unwrap(),
        Some(vec![1.0])
    );
    assert_eq!(
        session
            .memory_vector_row(profile.id, reliquary_memory_core::MemoryBodyId([0x88; 32]))
            .unwrap(),
        None
    );
    assert!(
        session
            .memory_vector_row(
                reliquary_vector_store::CompatibilityProfileId([0x33; 32]),
                record.body_id
            )
            .is_err()
    );
    let frames = container.read_frames().unwrap();
    let mapping_frame = frames
        .iter()
        .find(|frame| frame.payload.starts_with(b"RELMVEC1"))
        .unwrap();
    let early_path = Temp::new();
    let mut early = Container::create(
        &early_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    for frame in &frames {
        if frame.payload.starts_with(b"RELMCRT1") {
            early.append(&mapping_frame.payload).unwrap();
        }
        if frame.object.offset != mapping_frame.object.offset {
            early.append(&frame.payload).unwrap();
        }
    }
    early.sync().unwrap();
    drop(early);
    let early = Container::open_read_only(&early_path.0).unwrap();
    assert!(
        matches!(crate::RelSession::open(&early, &admission, [], []), Err(crate::OwnerSessionError::Record(message)) if message.contains("Memory vector references a body outside its physical prefix"))
    );
    let early_profile_path = Temp::new();
    let mut early_profile = Container::create(
        &early_profile_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    let profile_frame = frames
        .iter()
        .find(|frame| frame.payload.starts_with(b"RELCPRO1"))
        .unwrap();
    for frame in &frames {
        if frame.object.offset != profile_frame.object.offset {
            early_profile.append(&frame.payload).unwrap();
        }
    }
    early_profile.append(&profile_frame.payload).unwrap();
    early_profile.sync().unwrap();
    drop(early_profile);
    let early_profile = Container::open_read_only(&early_profile_path.0).unwrap();
    assert!(
        matches!(crate::RelSession::open(&early_profile, &admission, [], []), Err(crate::OwnerSessionError::Record(message)) if message.contains("Memory vector references backing outside its physical prefix"))
    );
    vectors
        .put(&mut container, &packed, &profiles, mapping([0x99; 32]))
        .unwrap();
    assert!(matches!(
        crate::RelSession::open(&container, &admission, [], []),
        Err(crate::OwnerSessionError::Record(_))
    ));
}

#[test]
fn compact_dream_population_uses_physical_grouped_births_and_excludes_siblings() {
    use crate::records::dream::{
        dream_begin_codec::{BeginPopulation, BeginRecord, encode},
        population_digest,
    };
    use std::collections::BTreeSet;
    let path = Temp::new();
    let owner = [0x76; 16];
    let mut container = Container::create(
        &path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    let mut first = fixture();
    first.routing_metadata.clear();
    first.freshness_birth_turn = Some(1);
    first.global_version_start = 1;
    let first_id = reliquary_memory_core::decode_grouped_creation(&first.memory_records[0])
        .unwrap()
        .id;
    container.append(&first.encode().unwrap()).unwrap();
    container
        .commit_embedded_version_range_at(1, 1, Some(first.transaction_time_ns))
        .unwrap();
    let mut second = first.clone();
    second.attempt = 2;
    second.global_version_start = 2;
    let mut source =
        reliquary_memory_core::decode_grouped_creation(&first.memory_records[0]).unwrap();
    source.mutation_id = "dream-source".into();
    source.id = memory_id(&source.mutation_id);
    source.memory_version = 2;
    let mut sibling = source.clone();
    sibling.mutation_id = "dream-sibling".into();
    sibling.id = memory_id(&sibling.mutation_id);
    sibling.memory_version = 3;
    second.memory_records = vec![
        reliquary_memory_core::encode_grouped_creation(&source).unwrap(),
        reliquary_memory_core::encode_grouped_creation(&sibling).unwrap(),
    ];
    second.memory_ids = vec![source.id.0, sibling.id.0];
    second.freshness_birth_memory_ids = second.memory_ids.clone();
    container.append(&second.encode().unwrap()).unwrap();
    container
        .commit_embedded_version_range_at(2, 2, Some(second.transaction_time_ns))
        .unwrap();
    let expected = BTreeSet::from([first_id]);
    let admission = crate::DreamBeginAdmission {
        owner_uuid: owner,
        pass_id: "compact".into(),
        source_id: source.id,
        turn: 1,
        graph_version_before: 0,
        preexisting_ids: expected.clone(),
        memory_cut: 3,
    };
    let begin = BeginRecord {
        owner_uuid: owner,
        pass_id: "compact".into(),
        source_id: source.id,
        turn: 1,
        graph_version_before: 0,
        population: BeginPopulation::CompactCut {
            memory_cut: 3,
            digest: population_digest(&expected),
        },
    };
    container.append(&encode(&begin).unwrap()).unwrap();
    container.sync().unwrap();
    assert!(
        crate::session::dream_prefix::validate_compact_dream_prefix(
            &container,
            std::slice::from_ref(&admission)
        )
        .is_ok()
    );
    let mut forged = admission.clone();
    forged.preexisting_ids.insert(sibling.id);
    assert!(
        crate::session::dream_prefix::validate_compact_dream_prefix(&container, &[forged]).is_err()
    );
    let mut forged = admission;
    forged.memory_cut = 4;
    assert!(
        crate::session::dream_prefix::validate_compact_dream_prefix(&container, &[forged]).is_err()
    );
    drop(container);
    let reopened = Container::open_read_only(&path.0).unwrap();
    let admission = crate::DreamBeginAdmission {
        owner_uuid: owner,
        pass_id: "compact".into(),
        source_id: source.id,
        turn: 1,
        graph_version_before: 0,
        preexisting_ids: expected,
        memory_cut: 3,
    };
    assert!(
        crate::session::dream_prefix::validate_compact_dream_prefix(&reopened, &[admission])
            .is_ok()
    );
    let future_path = Temp::new();
    let mut future = Container::create(
        &future_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    future.append(&first.encode().unwrap()).unwrap();
    future
        .commit_embedded_version_range_at(1, 1, Some(first.transaction_time_ns))
        .unwrap();
    let future_begin = BeginRecord {
        owner_uuid: owner,
        pass_id: "future".into(),
        source_id: source.id,
        turn: 1,
        graph_version_before: 0,
        population: BeginPopulation::CompactCut {
            memory_cut: 1,
            digest: population_digest(&BTreeSet::new()),
        },
    };
    future.append(&encode(&future_begin).unwrap()).unwrap();
    future.append(&second.encode().unwrap()).unwrap();
    future
        .commit_embedded_version_range_at(2, 2, Some(second.transaction_time_ns))
        .unwrap();
    future.sync().unwrap();
    let future_admission = crate::DreamBeginAdmission {
        owner_uuid: owner,
        pass_id: "future".into(),
        source_id: source.id,
        turn: 1,
        graph_version_before: 0,
        preexisting_ids: BTreeSet::new(),
        memory_cut: 1,
    };
    assert!(
        matches!(crate::session::dream_prefix::validate_compact_dream_prefix(&future, &[future_admission]), Err(crate::OwnerSessionError::Record(message)) if message.contains("source is absent from its physical prefix"))
    );
    let explicit_path = Temp::new();
    let mut explicit = Container::create(
        &explicit_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    explicit.append(&first.encode().unwrap()).unwrap();
    explicit
        .commit_embedded_version_range_at(1, 1, Some(first.transaction_time_ns))
        .unwrap();
    let mut early_explicit = future_begin.clone();
    early_explicit.population = BeginPopulation::ExplicitSet(BTreeSet::new());
    explicit.append(&encode(&early_explicit).unwrap()).unwrap();
    explicit.append(&second.encode().unwrap()).unwrap();
    explicit
        .commit_embedded_version_range_at(2, 2, Some(second.transaction_time_ns))
        .unwrap();
    assert!(
        matches!(crate::session::dream_prefix::validate_compact_dream_prefix(&explicit, &[]), Err(crate::OwnerSessionError::Record(message)) if message.contains("source is absent from its physical prefix"))
    );
    let unsupported_path = Temp::new();
    let mut unsupported = Container::create(
        &unsupported_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    unsupported.append(&first.memory_records[0]).unwrap();
    unsupported.append(&second.memory_records[0]).unwrap();
    unsupported.append(&encode(&begin).unwrap()).unwrap();
    let unsupported_admission = crate::DreamBeginAdmission {
        owner_uuid: owner,
        pass_id: "compact".into(),
        source_id: source.id,
        turn: 1,
        graph_version_before: 0,
        preexisting_ids: BTreeSet::from([first_id]),
        memory_cut: 3,
    };
    assert!(
        matches!(crate::session::dream_prefix::validate_compact_dream_prefix(&unsupported, &[unsupported_admission]), Err(crate::OwnerSessionError::Record(message)) if message.contains("unsupported birth-prefix proof"))
    );
    let future_graph_path = Temp::new();
    let mut future_graph = Container::create(
        &future_graph_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    future_graph.append(&first.encode().unwrap()).unwrap();
    future_graph
        .commit_embedded_version_range_at(1, 1, Some(first.transaction_time_ns))
        .unwrap();
    future_graph.append(&second.encode().unwrap()).unwrap();
    future_graph
        .commit_embedded_version_range_at(2, 2, Some(second.transaction_time_ns))
        .unwrap();
    let mut early_graph_begin = begin.clone();
    early_graph_begin.graph_version_before = 1;
    future_graph
        .append(&encode(&early_graph_begin).unwrap())
        .unwrap();
    let graph_admission =
        reliquary_graph::GraphAdmission::for_changes(owner, [first_id.0, source.id.0], []);
    let mut graph = reliquary_graph::GraphStore::open(&future_graph, &graph_admission).unwrap();
    graph
        .put(
            &mut future_graph,
            &graph_admission,
            &[reliquary_graph::RelationChange {
                source: reliquary_graph::NodeRef::Memory(first_id.0),
                target: reliquary_graph::NodeRef::Memory(source.id.0),
                kind: reliquary_graph::RelationKind::Topical,
                active: true,
                origin: reliquary_graph::RelationOrigin::User,
            }],
        )
        .unwrap();
    assert!(
        matches!(crate::session::dream_prefix::validate_compact_dream_prefix(&future_graph, &[]), Err(crate::OwnerSessionError::Record(message)) if message.contains("Graph cut outside its physical prefix"))
    );
}

#[test]
fn compact_dream_uses_direct_birth_intent_and_recorded_global_order() {
    use crate::records::dream::{
        dream_begin_codec::{BeginPopulation, BeginRecord, encode},
        freshness_journal::DreamFreshnessJournal,
        population_digest,
    };
    use std::collections::BTreeSet;
    let path = Temp::new();
    let owner = [0x77; 16];
    let writer = crate::RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();
    drop(writer);
    let mut container = Container::open_writer(&path.0).unwrap();
    let earlier_global = container.reserve_version(None).unwrap();
    let later_global = container.reserve_version(None).unwrap();
    let mut source =
        reliquary_memory_core::decode_grouped_creation(&fixture().memory_records[0]).unwrap();
    source.mutation_id = "direct-source".into();
    source.id = memory_id(&source.mutation_id);
    source.memory_version = 1;
    let mut earlier = source.clone();
    earlier.mutation_id = "direct-earlier".into();
    earlier.id = memory_id(&earlier.mutation_id);
    earlier.memory_version = 2;
    let mut journal = DreamFreshnessJournal::new(Some(owner));
    let source_intent = journal
        .publication_intent_payload(source.id, &source.mutation_id, 1, false)
        .unwrap();
    let earlier_intent = journal
        .publication_intent_payload(earlier.id, &earlier.mutation_id, 1, false)
        .unwrap();
    container.append(&source_intent).unwrap();
    container.append(&earlier_intent).unwrap();
    container
        .append_bound_publication_envelope(
            later_global,
            &reliquary_memory_core::encode_grouped_creation(&source).unwrap(),
        )
        .unwrap();
    container
        .append_bound_publication_envelope(
            earlier_global,
            &reliquary_memory_core::encode_grouped_creation(&earlier).unwrap(),
        )
        .unwrap();
    let mut memory = reliquary_memory_core::MemoryStore::open(&container).unwrap();
    memory.tombstone(&mut container, earlier.id, 1).unwrap();
    let mut freshness = reliquary_freshness_store::FreshnessStore::open(&container).unwrap();
    freshness
        .create_birth(
            &mut container,
            reliquary_freshness_store::FreshnessMemoryId(source.id.0),
            1,
            true,
        )
        .unwrap();
    let mut imports = crate::records::ImportEpochStore::open(&container).unwrap();
    let key = "cd".repeat(32);
    let start = crate::records::ImportStart {
        format_version: 2,
        owner_uuid: owner,
        import_id: crate::records::import_id(owner, &key),
        import_key: key,
        source_namespace: "source/account".into(),
        source_format: "json-v1".into(),
        started_at_ns: 1,
        pre: crate::records::ImportWatermarks {
            memory_version: 3,
            ..Default::default()
        },
    };
    imports.append_start(&mut container, start.clone()).unwrap();
    use sha2::{Digest, Sha256};
    let settlement = crate::records::ImportFreshnessSettlement {
        owner_uuid: owner,
        import_id: start.import_id.clone(),
        start_digest: format!("{:x}", Sha256::digest(serde_json::to_vec(&start).unwrap())),
        receipt_digest: crate::records::hex_digest(b"fixture-receipt"),
        prefix_digest: crate::records::import_settlement::source_prefix_digest(&container).unwrap(),
        rel_turn: 1,
        global_version: container.latest_version(),
        memory_version: start.pre.memory_version,
        graph_version: start.pre.graph_version,
        freshness_state_digest: crate::records::hex_digest(b"accepted-freshness"),
        freshness_memory_count: 1,
        replay_source_authority: None,
    };
    imports
        .append_settlement(&mut container, settlement.clone())
        .unwrap();
    let completion_version = container.latest_version();
    imports
        .append_complete(
            &mut container,
            crate::records::ImportComplete {
                format_version: 2,
                owner_uuid: owner,
                import_id: start.import_id.clone(),
                start_digest: settlement.start_digest.clone(),
                receipt_digest: settlement.receipt_digest.clone(),
                completed_at_ns: 2,
                post: crate::records::ImportWatermarks {
                    global_version: completion_version,
                    ..start.pre
                },
                historical_freshness_settlement_digest: settlement.digest().unwrap(),
            },
        )
        .unwrap();
    let population = BTreeSet::from([earlier.id]);
    let admission = crate::DreamBeginAdmission {
        owner_uuid: owner,
        pass_id: "direct".into(),
        source_id: source.id,
        turn: 1,
        graph_version_before: 0,
        preexisting_ids: population.clone(),
        memory_cut: 3,
    };
    let begin = BeginRecord {
        owner_uuid: owner,
        pass_id: "direct".into(),
        source_id: source.id,
        turn: 1,
        graph_version_before: 0,
        population: BeginPopulation::CompactCut {
            memory_cut: 3,
            digest: population_digest(&population),
        },
    };
    container.append(&encode(&begin).unwrap()).unwrap();
    journal.ingest(&earlier_intent).unwrap();
    let later_consume = journal
        .consume_publication_intent_payload(earlier.id)
        .unwrap()
        .unwrap();
    container.append(&later_consume).unwrap();
    container.sync().unwrap();
    let frames = container.read_frames().unwrap();
    drop(container);
    let reopened = Container::open_read_only(&path.0).unwrap();
    let graph = reliquary_graph::GraphAdmission::for_replay(owner, [], [], [], [], []);
    assert!(crate::RelSession::open(&reopened, &graph, [], [admission.clone()]).is_ok());
    let open_import_path = Temp::new();
    let mut open_import = Container::create(
        &open_import_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    for frame in &frames {
        if !frame.payload.starts_with(b"RELIEPC1") && !frame.payload.starts_with(b"RELIEPC2") {
            open_import.append(&frame.payload).unwrap();
        }
    }
    assert!(matches!(
        crate::session::dream_prefix::validate_compact_dream_prefix(&open_import, &[admission.clone()]),
        Err(crate::OwnerSessionError::Record(message)) if message.contains("fenced by an open import epoch")
    ));
    let changed_cut_path = Temp::new();
    let mut changed_cut = Container::create(
        &changed_cut_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    let import_start = frames
        .iter()
        .find(|frame| frame.payload.starts_with(b"RELIEPS2"))
        .unwrap();
    for frame in &frames {
        if frame.payload.starts_with(b"RELMTMB1") {
            changed_cut.append(&import_start.payload).unwrap();
        }
        if !frame.payload.starts_with(b"RELIEPS2") {
            changed_cut.append(&frame.payload).unwrap();
        }
    }
    assert!(matches!(
        crate::session::dream_prefix::validate_compact_dream_prefix(&changed_cut, &[admission.clone()]),
        Err(crate::OwnerSessionError::Record(message)) if message.contains("unsupported birth-prefix proof")
    ));
    let wrong_clock_path = Temp::new();
    let mut wrong_clock = Container::create(
        &wrong_clock_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    for frame in &frames {
        let mut payload = frame.payload.clone();
        if payload.starts_with(b"RELMTMB1") {
            payload[48..56].copy_from_slice(&4u64.to_le_bytes());
        }
        wrong_clock.append(&payload).unwrap();
    }
    assert!(matches!(
        crate::session::dream_prefix::validate_compact_dream_prefix(&wrong_clock, &[admission.clone()]),
        Err(crate::OwnerSessionError::Record(message)) if message.contains("Memory history change differs")
    ));
    let wrong_owner_path = Temp::new();
    let mut wrong_owner = Container::create(
        &wrong_owner_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    for frame in &frames {
        let mut payload = frame.payload.clone();
        if payload.starts_with(b"RELFRS02") {
            payload[8..24].copy_from_slice(&[0x44; 16]);
        }
        wrong_owner.append(&payload).unwrap();
    }
    assert!(matches!(
        crate::session::dream_prefix::validate_compact_dream_prefix(&wrong_owner, &[admission.clone()]),
        Err(crate::OwnerSessionError::Record(message)) if message.contains("Freshness journal owner mismatch")
    ));
    let missing_intent_path = Temp::new();
    let mut missing = Container::create(
        &missing_intent_path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    for frame in &frames {
        if frame.payload != earlier_intent {
            missing.append(&frame.payload).unwrap();
        }
    }
    missing.sync().unwrap();
    drop(missing);
    let missing = Container::open_read_only(&missing_intent_path.0).unwrap();
    assert!(
        matches!(crate::RelSession::open(&missing, &graph, [], [admission]), Err(crate::OwnerSessionError::Record(message)) if message.contains("population differs from physical prefix"))
    );
}
