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
            owner_id: "owner-rel".into(),
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
fn routed_replay_requires_external_owner_authentication() {
    let mut completion = fixture();
    completion
        .external_memory_refs
        .push(GroupedExternalMemoryRef {
            owner_id: "phy-owner".into(),
            memory_id: [9; 32],
        });
    let decoded = GroupedInsomniaCompletion::decode(&completion.encode().unwrap()).unwrap();
    assert!(decoded.validate_replay_episode([4; 32]).is_err());

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
    assert!(crate::RelSession::open(&container, &admission, [], []).is_ok());
    vectors
        .put(&mut container, &packed, &profiles, mapping([0x99; 32]))
        .unwrap();
    assert!(matches!(
        crate::RelSession::open(&container, &admission, [], []),
        Err(crate::OwnerSessionError::Record(_))
    ));
}
