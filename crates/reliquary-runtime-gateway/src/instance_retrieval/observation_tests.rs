use super::*;
use reliquary_archive::{Archive, TurnInput};
use reliquary_memory_core::MemorySourceRef;
use reliquary_owner_session::model::MemoryTextField;
use reliquary_owner_session::observation::{
    ObservationBatch, ObservationCitation, ObservationDraft, ObservationId, ObservationOperation,
    ObservationPendingReason, ObservationPolicy, ObservationProfile, ObservationReceptor,
    ObservationReceptorKind, ObservationStanding, ObservationStatus, ObservationTimeScope,
};

const POLICY: ObservationPolicy = ObservationPolicy {
    version: 1,
    general_route: [2; 32],
    decision_route: [3; 32],
};

fn source(fixture: &Fixture) -> ObservationCitation {
    fixture.gateway.quiesce_owner(fixture.owner).unwrap();
    let owner = fixture.owner.as_bytes();
    let text = "The delivery has been approved.";
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
                    title: "Delivery".into(),
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
        Some("The delivery has been approved."),
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
                RetrievalAction::Query("zephyr".into()),
                None,
                None,
            )
            .unwrap()
            .prepared
            .unwrap()
    };
    let search = || {
        alice
            .search_rel_memories(fixture.owner, "zephyr", SearchMode::Lexical, Some(10), None)
            .unwrap()
    };
    let before = search();
    assert_eq!(before.observations.len(), 3);
    assert!(before.memories.is_empty());
    let host = Arc::new(Host::default());
    let router = connect(&fixture.gateway, host.clone());
    let accepted = alice
        .begin_memory_retrieval_turn(fixture.owner, "confirmed-observation")
        .unwrap();
    let accepted_token = prepare(&accepted);
    let accepted_id = accepted_token.stable_id().to_owned();
    assert!(
        !alice
            .deliver_agentic_memory(&accepted, &accepted_token, &router)
            .unwrap()
    );
    assert!(
        !alice
            .deliver_agentic_memory(&accepted, &accepted_token, &router)
            .unwrap()
    );
    let operation = host.operations.lock().unwrap()[&accepted_id].clone();
    let payload: Value = serde_json::from_slice(&operation.payload).unwrap();
    assert_eq!(payload["memories"], json!([]));
    assert_eq!(payload["observations"].as_array().unwrap().len(), 3);
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
    let after = search();
    assert_eq!(before.cut.memory_version, after.cut.memory_version);
    assert_eq!(before.cut.graph_version, after.cut.graph_version);
    assert_ne!(
        before.cut.observation_version,
        after.cut.observation_version
    );
    assert!(after.observations.is_empty());
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
        alice.deliver_agentic_memory(&pending_turn, &pending_delivery, &router),
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
        3
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
    let reaffirmed = search();
    assert_eq!(reaffirmed.observations.len(), 3);
    assert!(
        reaffirmed
            .observations
            .iter()
            .all(|hit| hit.claim.citation.revision == 1)
    );
}
