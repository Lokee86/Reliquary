use super::*;
use reliquary_archive::{Archive, TurnInput};
use reliquary_memory_core::MemorySourceRef;
use reliquary_owner_session::model::MemoryTextField;
use reliquary_owner_session::observation::{
    ObservationBatch, ObservationCitation, ObservationClaimVector, ObservationDraft, ObservationId,
    ObservationOperation, ObservationPendingReason, ObservationPolicy, ObservationProfile,
    ObservationReceptor, ObservationReceptorKind, ObservationStanding, ObservationStatus,
    ObservationTimeScope,
};

const POLICY: ObservationPolicy = ObservationPolicy {
    version: 1,
    general_route: [2; 32],
    decision_route: [3; 32],
};

fn source(fixture: &Fixture) -> ObservationCitation {
    fixture.gateway.quiesce_owner(fixture.owner).unwrap();
    let owner = fixture.owner.as_bytes();
    let text = "The handoff has been approved.";
    let record = {
        let mut container = Container::open_writer(&fixture.path).unwrap();
        Archive::open(&container)
            .unwrap()
            .accept_turn(
                &mut container,
                TurnInput {
                    id: "source".into(),
                    conversation_id: "accepted".into(),
                    parent_id: None,
                    role: "user".into(),
                    principal_id: Some("alice".into()),
                    timestamp_ns: 300,
                    content: text.into(),
                    attachments: Vec::new(),
                    project_attachments: Vec::new(),
                },
            )
            .unwrap();
        let mut memories = MemoryStore::open(&container).unwrap();
        memories
            .publish_versioned_creation(
                &mut container,
                MemoryDraft {
                    title: "Handoff".into(),
                    content: text.into(),
                    category: "fact".into(),
                    memory_type: "semantic".into(),
                    authority_kind: "direct".into(),
                    temporal_status: "current".into(),
                    scope: "project".into(),
                    lifecycle_state: "knowledge".into(),
                    archived: false,
                    parent_id: None,
                    superseded_by: None,
                    source_node_id: Some("source".into()),
                    content_source_conversation_id: Some("accepted".into()),
                    content_source_node_id: Some("source".into()),
                    grounding_source_conversation_id: None,
                    grounding_source_node_id: None,
                    source_episode_id: Some([8; 32]),
                    source_ref: Some(MemorySourceRef {
                        owner_id: hex(&owner),
                        principal_id: Some("alice".into()),
                        source_episode_id: [8; 32],
                        source_node_id: "source".into(),
                        content_source_conversation_id: Some("accepted".into()),
                        content_source_node_id: Some("source".into()),
                        grounding_source_conversation_id: None,
                        grounding_source_node_id: None,
                    }),
                    source_time_ns: Some(300),
                    temporal_inference: None,
                    mutation_id: "observation-source".into(),
                    created_at_ns: 1,
                    updated_at_ns: 1,
                },
                1,
            )
            .unwrap()
            .revision
            .record
    };
    fixture
        .gateway
        .mount_writable_reliquary(
            &fixture.path,
            &GraphAdmission::for_replay(owner, [], [], [], [], []),
            [],
            [],
        )
        .unwrap();
    let history_generation = fixture
        .alice()
        .with_reliquary(fixture.owner, |rel| {
            rel.management_read().history_generation
        })
        .unwrap();
    ObservationCitation {
        owner_id: owner,
        history_generation,
        source: ObservationSource::Memory(record.id.0),
        revision: record.revision,
        body_id: record.body_id.0,
        field: MemoryTextField::Content,
        start_byte: 0,
        end_byte: text.len() as u32,
    }
}

fn publish(
    fixture: &Fixture,
    citation: ObservationCitation,
    fingerprint: u8,
    operation: ObservationOperation,
) -> reliquary_owner_session::observation::ObservationBatchReceipt {
    fixture
        .alice()
        .with_rel_writer(fixture.owner, |writer| {
            writer.publish_observation_batch(ObservationBatch {
                owner_id: citation.owner_id,
                history_generation: citation.history_generation,
                input_fingerprint: [fingerprint; 32],
                policy: POLICY,
                operations: vec![operation],
                progress: None,
            })
        })
        .unwrap()
        .unwrap()
}

fn create_observation(
    fixture: &Fixture,
    support: ObservationCitation,
    fingerprint: u8,
    proposition: &str,
) -> (ObservationId, ObservationCitation) {
    let draft = ObservationDraft {
        proposition: proposition.into(),
        profile: ObservationProfile::Ordinary,
        status: ObservationStatus::Observed,
        time_scope: ObservationTimeScope::Current,
        validity: None,
        citations: vec![support],
        reviewed: Vec::new(),
        receptors: [
            ObservationReceptorKind::Support,
            ObservationReceptorKind::Challenge,
            ObservationReceptorKind::Qualification,
            ObservationReceptorKind::Change,
        ]
        .into_iter()
        .map(|kind| ObservationReceptor {
            kind,
            description: "A trial receptor".into(),
            embedding_profile: [45; 32],
            vector: vec![1.0],
        })
        .collect(),
    };
    let receipt = publish(
        fixture,
        support,
        fingerprint,
        ObservationOperation::Create { draft },
    );
    let id = receipt.revisions[0].0;
    let claim = fixture
        .alice()
        .inspect_observation(fixture.owner, id, None)
        .unwrap()
        .unwrap()
        .claim;
    (id, claim.citation)
}

#[test]
fn mixed_delivery_confirms_once_and_records_selected_memory_use() {
    let fixture = Fixture::new();
    let support = source(&fixture);
    let (_, observation) = create_observation(
        &fixture,
        support,
        10,
        "Needle work includes an approved delivery.",
    );
    let alice = fixture.alice();
    let turn = alice
        .begin_memory_retrieval_turn(fixture.owner, "mixed-delivery")
        .unwrap();
    let token = alice
        .prepare_agentic_memory_delivery(
            &turn,
            &route(),
            RetrievalAction::Query("needle other".into()),
            None,
            None,
        )
        .unwrap()
        .prepared
        .unwrap();
    let preview = alice
        .preview_agentic_memory_delivery(&turn, &token)
        .unwrap();
    assert_eq!(preview.memories.len(), 2);
    assert_eq!(preview.observations.len(), 1);
    assert_eq!(preview.observations[0].claim.citation, observation);

    let host = Arc::new(Host::default());
    host.pending.store(true, Ordering::Relaxed);
    let router = connect(&fixture.gateway, host.clone());
    assert!(matches!(
        alice.deliver_agentic_memory_confirmed(&turn, &token, &router),
        Err(GatewayError::InvalidHandoffReceipt)
    ));
    assert!(matches!(
        alice.deliver_agentic_memory_confirmed(&turn, &token, &router),
        Err(GatewayError::HostStatusUnknown)
    ));
    host.pending.store(false, Ordering::Relaxed);
    assert_eq!(
        alice
            .deliver_agentic_memory_confirmed(&turn, &token, &router)
            .unwrap(),
        RetrievalDeliveryOutcome::Confirmed {
            recorded_memory_use: true,
        }
    );
    assert_eq!(
        alice
            .deliver_agentic_memory_confirmed(&turn, &token, &router)
            .unwrap(),
        RetrievalDeliveryOutcome::Replay
    );
    assert_eq!(host.calls.load(Ordering::Relaxed), 1);
    let receipt = alice
        .with_reliquary(fixture.owner, |rel| {
            rel.accepted_external_use(token.stable_id()).cloned()
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        receipt
            .sources
            .iter()
            .map(|id| hex(&id.0))
            .collect::<Vec<_>>(),
        preview
            .memories
            .iter()
            .map(|memory| memory.memory_id.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        turn.state.lock().unwrap().provided_observations,
        [observation].into()
    );
}

#[test]
fn observation_standing_fences_new_delivery_and_preserves_confirmed_history() {
    let fixture = Fixture::new();
    let support = source(&fixture);
    let (first, first_cite) =
        create_observation(&fixture, support, 10, "Zephyr delivery was approved.");
    let (second, second_cite) =
        create_observation(&fixture, first_cite, 11, "Zephyr has an approved delivery.");
    let (third, _) = create_observation(
        &fixture,
        second_cite,
        12,
        "Zephyr work includes an approved delivery.",
    );
    let (_, independent) =
        create_observation(&fixture, support, 15, "Zephyr handoff was approved.");
    let alice = fixture.alice();
    let bob = fixture.gateway.instance("bob", [], []).unwrap();
    assert!(matches!(
        bob.inspect_observation(fixture.owner, first, None),
        Err(GatewayError::UnauthorizedRel { .. })
    ));
    let page = alice.list_observations(fixture.owner, None, 1).unwrap();
    let cursor = page.next.unwrap();
    assert_eq!(
        alice
            .list_observations(fixture.owner, Some(&cursor), 1)
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(
        alice
            .inspect_observation_evidence(fixture.owner, first, 1, support)
            .unwrap()
            .as_deref(),
        Some("The handoff has been approved."),
    );
    assert_eq!(
        alice
            .observation_history(fixture.owner, first, None, 1)
            .unwrap()
            .items
            .len(),
        1
    );

    let prepare = |turn: &MemoryRetrievalTurn| {
        alice
            .prepare_agentic_memory_delivery(
                turn,
                &route(),
                RetrievalAction::Query("zephyr delivery".into()),
                None,
                None,
            )
            .unwrap()
            .prepared
            .unwrap()
    };
    let search = |limit| {
        alice
            .search_rel_memories(
                fixture.owner,
                "zephyr delivery",
                SearchMode::Lexical,
                Some(limit),
                None,
            )
            .unwrap()
    };
    let before = search(10);
    assert_eq!(before.observations.len(), 4);
    assert!(before.memories.is_empty());
    let inspecting = alice
        .begin_memory_retrieval(
            fixture.owner,
            "inspection-disclosure",
            RetrievalMode::Inspect,
        )
        .unwrap();
    let observation_version = before.cut.observation_version.unwrap();
    alice
        .accept_inspected_observations(&inspecting, observation_version, &[independent])
        .unwrap();
    assert_eq!(
        inspecting.state.lock().unwrap().provided_observations,
        BTreeSet::from([independent])
    );
    let host = Arc::new(Host::default());
    let router = connect(&fixture.gateway, host.clone());
    let accepted = alice
        .begin_memory_retrieval_turn(fixture.owner, "confirmed-observation")
        .unwrap();
    let accepted_token = prepare(&accepted);
    let accepted_id = accepted_token.stable_id().to_owned();
    assert_eq!(
        alice
            .deliver_agentic_memory_confirmed(&accepted, &accepted_token, &router)
            .unwrap(),
        RetrievalDeliveryOutcome::Confirmed {
            recorded_memory_use: false,
        }
    );
    assert_eq!(
        alice
            .deliver_agentic_memory_confirmed(&accepted, &accepted_token, &router)
            .unwrap(),
        RetrievalDeliveryOutcome::Replay
    );
    let rebuilt = alice
        .begin_memory_retrieval_turn(fixture.owner, "confirmed-observation")
        .unwrap();
    let rebuilt_token = prepare(&rebuilt);
    assert_eq!(rebuilt_token.stable_id(), accepted_id);
    assert_eq!(
        alice
            .deliver_agentic_memory_confirmed(&rebuilt, &rebuilt_token, &router)
            .unwrap(),
        RetrievalDeliveryOutcome::Replay
    );
    assert!(
        !alice
            .deliver_agentic_memory(&accepted, &accepted_token, &router)
            .unwrap()
    );
    let operation = host.operations.lock().unwrap()[&accepted_id].clone();
    let payload: Value = serde_json::from_slice(&operation.payload).unwrap();
    assert_eq!(payload["memories"], json!([]));
    assert_eq!(payload["observations"].as_array().unwrap().len(), 4);
    assert_eq!(host.calls.load(Ordering::Relaxed), 1);
    let pending_turn = alice
        .begin_memory_retrieval_turn(fixture.owner, "pending-observation")
        .unwrap();
    let pending_delivery = prepare(&pending_turn);

    publish(
        &fixture,
        support,
        13,
        ObservationOperation::Pending {
            id: first,
            expected_revision: 1,
            reason: ObservationPendingReason::ChangedSupport,
            evidence: Vec::new(),
        },
    );
    let after = search(1);
    assert_eq!(before.cut.memory_version, after.cut.memory_version);
    assert_eq!(before.cut.graph_version, after.cut.graph_version);
    assert_ne!(
        before.cut.observation_version,
        after.cut.observation_version
    );
    assert_eq!(after.observations.len(), 1);
    assert_eq!(after.observations[0].claim.citation, independent);
    assert!(matches!(
        alice.accept_inspected_observations(&inspecting, observation_version, &[first_cite]),
        Err(GatewayError::StaleRetrieval)
    ));
    assert!(
        alice
            .list_observations(fixture.owner, Some(&cursor), 1)
            .is_err()
    );
    for id in [second, third] {
        let row = alice
            .inspect_observation(fixture.owner, id, None)
            .unwrap()
            .unwrap();
        assert_eq!(
            row.standing,
            ObservationStanding::Pending(ObservationPendingReason::UpstreamReview)
        );
        assert!(!row.claim.proposition.is_empty());
    }
    assert!(matches!(
        alice.deliver_agentic_memory_confirmed(&pending_turn, &pending_delivery, &router),
        Err(GatewayError::StaleRetrieval)
    ));
    assert_eq!(host.calls.load(Ordering::Relaxed), 1);
    let reopened_turn = alice
        .begin_memory_retrieval_turn(fixture.owner, "confirmed-observation")
        .unwrap();
    assert!(
        !alice
            .recover_agentic_memory_delivery(&reopened_turn, &accepted_id, &router)
            .unwrap()
    );
    assert_eq!(
        reopened_turn
            .state
            .lock()
            .unwrap()
            .provided_observations
            .len(),
        4
    );
    assert!(
        alice
            .with_reliquary(fixture.owner, |rel| rel
                .accepted_external_use(&accepted_id)
                .is_none())
            .unwrap()
    );

    publish(
        &fixture,
        support,
        14,
        ObservationOperation::Reaffirm {
            id: first,
            expected_revision: 1,
            reviewed: Vec::new(),
        },
    );
    let reaffirmed = search(10);
    assert_eq!(reaffirmed.observations.len(), 4);
    assert!(
        reaffirmed
            .observations
            .iter()
            .all(|hit| hit.claim.citation.revision == 1)
    );
}

#[test]
fn committed_source_removal_fences_prepared_and_retained_returns() {
    let fixture = Fixture::new();
    let support = source(&fixture);
    let (first, upstream) = create_observation(&fixture, support, 18, "A handoff was approved.");
    let (second, _) =
        create_observation(&fixture, upstream, 19, "The approved handoff can proceed.");
    let alice = fixture.alice();
    let prepare = |turn: &MemoryRetrievalTurn| {
        alice
            .prepare_agentic_memory_delivery(
                turn,
                &route(),
                RetrievalAction::Query("needle approved handoff".into()),
                None,
                None,
            )
            .unwrap()
            .prepared
            .unwrap()
    };
    let turn = alice
        .begin_memory_retrieval_turn(fixture.owner, "before-source-removal")
        .unwrap();
    let token = prepare(&turn);
    let preview = alice
        .preview_agentic_memory_delivery(&turn, &token)
        .unwrap();
    assert!(!preview.memories.is_empty());
    assert_eq!(preview.observations.len(), 2);

    let host = Arc::new(Host::default());
    let router = connect(&fixture.gateway, host.clone());
    let confirmed = alice
        .begin_memory_retrieval_turn(fixture.owner, "confirmed-copy")
        .unwrap();
    let confirmed_token = prepare(&confirmed);
    alice
        .deliver_agentic_memory_confirmed(&confirmed, &confirmed_token, &router)
        .unwrap();

    assert!(
        alice
            .with_rel_writer(fixture.owner, |writer| writer
                .tombstone_conversation("accepted"))
            .unwrap()
            .unwrap()
    );
    assert!(matches!(
        alice.preview_agentic_memory_delivery(&turn, &token),
        Err(GatewayError::StaleRetrieval)
    ));
    assert!(matches!(
        alice.deliver_agentic_memory_confirmed(&turn, &token, &router),
        Err(GatewayError::StaleRetrieval)
    ));
    let recovery = alice
        .begin_memory_retrieval_turn(fixture.owner, "confirmed-copy")
        .unwrap();
    // Accounting may replay an existing receipt. It cannot return claim bodies.
    if alice
        .recover_agentic_memory_delivery(&recovery, confirmed_token.stable_id(), &router)
        .is_ok()
    {
        assert!(
            recovery
                .state
                .lock()
                .unwrap()
                .provided_observations
                .is_empty()
        );
    }
    assert_eq!(host.calls.load(Ordering::Relaxed), 1);
    let search = alice
        .search_rel_memories(
            fixture.owner,
            "needle approved handoff",
            SearchMode::Lexical,
            Some(20),
            None,
        )
        .unwrap();
    assert!(!search.memories.is_empty());
    assert!(search.observations.is_empty());
    for id in [first, second] {
        let historical = alice
            .inspect_observation(fixture.owner, id, Some(1))
            .unwrap()
            .unwrap();
        assert!(historical.claim.proposition.is_empty());
    }
    assert!(
        alice
            .inspect_observation_evidence(fixture.owner, first, 1, support)
            .unwrap()
            .is_none()
    );
}

#[test]
fn paraphrase_uses_the_shared_memory_search_until_observation_review() {
    let fixture = Fixture::new();
    let support = source(&fixture);
    let (id, citation) = create_observation(&fixture, support, 16, "Zephyr delivery was approved.");
    let alice = fixture.alice();
    let endpoint = crate::g4_retrieval_acceptance::Embed {
        calls: Default::default(),
        dimensions: 2,
    };
    let profile = alice
        .publish_embedding_profile(fixture.owner, &endpoint)
        .unwrap();
    let memories: Vec<_> = alice
        .retrieval_snapshot(fixture.owner, None)
        .unwrap()
        .memories
        .iter()
        .map(|row| row.revision.record.id)
        .collect();
    let vectors = alice
        .prepare_owner_memory_vectors(fixture.owner, profile, &memories, &endpoint)
        .unwrap();
    alice.publish_owner_memory_vectors(&vectors).unwrap();
    alice
        .with_rel_writer(fixture.owner, |writer| {
            writer.publish_observation_claim_vectors(
                profile,
                vec![ObservationClaimVector {
                    citation,
                    vector: vec![1.0, 0.0],
                }],
            )
        })
        .unwrap()
        .unwrap();
    fixture.gateway.quiesce_owner(fixture.owner).unwrap();
    fixture
        .gateway
        .mount_writable_reliquary(
            &fixture.path,
            &GraphAdmission::for_replay(fixture.owner.as_bytes(), [], [], [], [], []),
            [],
            [],
        )
        .unwrap();
    let alice = fixture.alice();

    // "Needle" supplies a competing lexical Memory. None of these words
    // occur in the Observation. Its claim vector must win the same limit.
    let paraphrase = "needle freight authorization";
    let search = |mode, embedding| {
        alice
            .search_rel_memories(fixture.owner, paraphrase, mode, Some(1), embedding)
            .unwrap()
    };
    let embedding = || Some((profile, vec![1.0, 0.0]));
    let lexical = search(SearchMode::Lexical, None);
    assert!(lexical.observations.is_empty());
    assert_eq!(lexical.memories.len(), 1);

    let before = search(SearchMode::Hybrid, embedding());
    assert!(before.memories.is_empty());
    assert!(before.truncated);
    let [observation] = before.observations.as_slice() else {
        panic!("expected only the Observation to win the shared result limit");
    };
    assert_eq!(observation.lexical_score, 0.0);
    assert_eq!(observation.semantic_score, Some(1.0));
    assert_eq!(observation.claim.citation, citation);
    assert_eq!(observation.claim.support, vec![support]);
    let semantic = search(SearchMode::Semantic, embedding());
    assert_eq!(semantic.observations[0].claim.citation, citation);

    let recall = |turn_id| {
        let turn = alice
            .begin_memory_retrieval(fixture.owner, turn_id, RetrievalMode::Inspect)
            .unwrap();
        alice
            .evaluate_agentic_memory(
                &turn,
                &route(),
                RetrievalAction::Query(paraphrase.into()),
                || Ok(embedding()),
            )
            .unwrap()
            .preview
            .unwrap()
    };
    assert_eq!(
        recall("paraphrase-current").observations[0].claim.support,
        vec![support]
    );

    publish(
        &fixture,
        support,
        17,
        ObservationOperation::Pending {
            id,
            expected_revision: citation.revision,
            reason: ObservationPendingReason::ChangedSupport,
            evidence: Vec::new(),
        },
    );
    let after = search(SearchMode::Hybrid, embedding());
    assert!(after.observations.is_empty());
    assert_eq!(after.memories.len(), 1);
    assert!(
        search(SearchMode::Semantic, embedding())
            .observations
            .is_empty()
    );
    assert!(recall("paraphrase-in-review").observations.is_empty());
}
