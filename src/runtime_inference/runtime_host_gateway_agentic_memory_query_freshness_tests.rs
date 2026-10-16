use super::*;
use crate::{
    AcceptedMemoryUseReceipt, EmbeddingMode, FreshnessState, MemoryDraft, MemoryRef,
    ReliquaryRuntimeRoutes,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

struct DeclineBatch {
    calls: Arc<Mutex<Vec<Value>>>,
}

impl DecisionEndpoint for DeclineBatch {
    fn model(&self) -> &str {
        "agentic-query-decline-test"
    }

    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, DecisionEndpointError> {
        self.calls.lock().unwrap().push(state.clone());
        Ok(json!({"version": 1, "approved": []}))
    }
}

fn candidate_ids(state: &Value) -> Vec<String> {
    state["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| {
            candidate["memory"]["memory_id"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

fn legacy_draft() -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "Needle legacy Memory".into(),
        content: "needle evidence without a Freshness record".into(),
        scope: "test".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: "legacy-without-freshness".into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

#[test]
fn freshness_orders_complete_pool_and_keeps_missing_stale_and_dormant_candidates_eligible() {
    let (gateway, instance, calls) = super::fixture(false);
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    let owner = &envelope.owners[0];
    let embedding = gateway.agentic_query_embedding_route().unwrap();
    let query_vector = embedding
        .embed(EmbeddingMode::Query, &["needle".into()])
        .unwrap()
        .remove(0);
    let (raw_ranked, target) = gateway
        .with_rel(&owner.token, |runtime| {
            let pool = runtime
                .cva
                .collect_memory_candidate_pool("needle", &query_vector, owner.profile.unwrap())
                .unwrap();
            let mut semantic = pool
                .candidates
                .iter()
                .filter_map(|candidate| {
                    candidate
                        .evidence
                        .semantic_score
                        .map(|score| (candidate.memory_id, score))
                })
                .collect::<Vec<_>>();
            semantic.sort_by(|left, right| {
                right
                    .1
                    .total_cmp(&left.1)
                    .then_with(|| left.0.cmp(&right.0))
            });
            let target = semantic.last().unwrap().0;
            let rank = semantic.iter().position(|(id, _)| *id == target).unwrap() + 1;
            Ok((rank, target))
        })
        .unwrap();
    assert!(
        raw_ranked > 10,
        "fixture target should fall below raw top ten"
    );

    let (_owner_uuid, accepted_turn, legacy) = gateway
        .with_rel(&owner.token, |runtime| {
            let mut previous = None;
            for index in 0..1_700 {
                let id = format!("aging-{index}");
                runtime
                    .cva
                    .append_node(
                        id.clone(),
                        "agentic-aging".into(),
                        previous,
                        if index % 2 == 0 { "user" } else { "assistant" }.into(),
                        index,
                        "accepted activity advances the REL clock",
                    )
                    .unwrap();
                previous = Some(id);
            }
            let accepted_turn = runtime.cva.rel_turn_count();
            let owner_uuid = runtime.cva.owner_uuid().unwrap();
            runtime
                .cva
                .record_accepted_memory_use(
                    AcceptedMemoryUseReceipt {
                        owner_uuid,
                        accepted_turn,
                        use_id: "agentic-query-fixture-use".into(),
                        memories: vec![target],
                    },
                    1,
                )
                .unwrap();
            let (legacy, created) = crate::cva_memory_publish::publish_memory_parts(
                &runtime.cva.archive,
                &mut runtime.cva.memories,
                &mut runtime.cva.container,
                None,
                0,
                legacy_draft(),
            )
            .unwrap();
            assert!(created);
            runtime.cva.sync().unwrap();
            Ok((owner_uuid, accepted_turn, legacy.id))
        })
        .unwrap();

    let (score, state, missing_record) = gateway
        .with_rel(&owner.token, |runtime| {
            let grade = runtime
                .cva
                .ego_memory_freshness(&[target], accepted_turn)
                .unwrap()
                .remove(0);
            Ok((
                grade.score,
                grade.state,
                runtime.cva.freshness_record(legacy).is_none(),
            ))
        })
        .unwrap();
    assert_eq!(state, FreshnessState::Stale);
    assert!(score <= 0);
    assert!(missing_record, "the legacy fixture must remain unenrolled");

    let mut controller = AgenticMemoryController::new();
    let result = gateway
        .memory_query(&instance, &mut controller, "needle", "freshness-order")
        .unwrap();
    assert!(matches!(
        result.payload().outcome,
        AgenticMemoryOutcome::Memories(_)
    ));
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    let presented = calls[0]["candidates"].as_array().unwrap();
    assert_eq!(presented.len(), 10);
    let presented_ids = presented
        .iter()
        .map(|candidate| {
            candidate["memory"]["memory_id"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect::<BTreeSet<_>>();
    let target_hex = target
        .0
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let legacy_hex = legacy
        .0
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert!(presented_ids.contains(&target_hex));
    assert!(presented_ids.contains(&legacy_hex));
    let (dormant_ids, legacy_still_unenrolled) = gateway
        .with_rel(&owner.token, |runtime| {
            let ids = runtime.cva.memory_ids();
            let dormant_ids = runtime
                .cva
                .ego_memory_freshness(&ids, accepted_turn)
                .unwrap()
                .into_iter()
                .filter(|grade| grade.state == FreshnessState::Dormant)
                .map(|grade| {
                    grade
                        .memory_id
                        .0
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                })
                .collect::<BTreeSet<_>>();
            Ok((dormant_ids, runtime.cva.freshness_record(legacy).is_none()))
        })
        .unwrap();
    assert!(legacy_still_unenrolled);
    assert!(
        presented_ids.iter().any(|id| dormant_ids.contains(id)),
        "Dormant Memories remain eligible when the presentation batch has room"
    );
}

#[test]
fn equal_freshness_scores_use_owner_qualified_memory_id_order() {
    let (gateway, instance, calls) = super::fixture(false);
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    let expected = gateway
        .with_rel(&envelope.owners[0].token, |runtime| {
            let ids = runtime.cva.memory_ids();
            let grades = runtime
                .cva
                .ego_memory_freshness(&ids, envelope.owners[0].cut)
                .unwrap();
            assert!(grades.iter().all(|grade| grade.score == grades[0].score));
            let mut refs = ids
                .into_iter()
                .map(|memory_id| MemoryRef {
                    owner_id: envelope.owners[0].token.owner_id.clone(),
                    memory_id,
                })
                .collect::<Vec<_>>();
            refs.sort();
            Ok(refs.into_iter().take(10).collect::<Vec<_>>())
        })
        .unwrap()
        .into_iter()
        .map(|memory| {
            memory
                .memory_id
                .0
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    gateway
        .memory_query(
            &instance,
            &mut AgenticMemoryController::new(),
            "needle",
            "freshness-tie",
        )
        .unwrap();
    assert_eq!(candidate_ids(&calls.lock().unwrap()[0]), expected);
}

#[test]
fn rejected_new_batch_falls_back_to_raw_selected_matches() {
    let (gateway, instance, _) = super::fixture(false);
    let embedding = gateway.agentic_query_embedding_route().unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    gateway
        .set_routes(
            ReliquaryRuntimeRoutes::new(None, None, None, None, Some(embedding))
                .with_memory_retrieval_decision(Some(Arc::new(DeclineBatch {
                    calls: Arc::clone(&calls),
                }))),
        )
        .unwrap();
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    let selected = gateway
        .with_rel(&envelope.owners[0].token, |runtime| {
            Ok(MemoryRef {
                owner_id: envelope.owners[0].token.owner_id.clone(),
                memory_id: runtime.cva.memory_ids()[0],
            })
        })
        .unwrap();
    let mut controller = AgenticMemoryController::new();
    controller.selected_ids.insert(selected.clone());
    let result = gateway
        .memory_query(&instance, &mut controller, "needle", "declined-fallback")
        .unwrap();
    assert_eq!(
        result.payload().outcome,
        AgenticMemoryOutcome::AlreadyProvided(BTreeSet::from([selected.clone()]))
    );
    assert!(result.delivered().is_empty());
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(controller.selected_ids(), &BTreeSet::from([selected]));
}

#[test]
fn all_selected_fallback_does_not_require_a_decision_route() {
    let (gateway, instance, calls) = super::fixture(false);
    let embedding = gateway.agentic_query_embedding_route().unwrap();
    gateway
        .set_routes(ReliquaryRuntimeRoutes::new(
            None,
            None,
            None,
            None,
            Some(embedding),
        ))
        .unwrap();
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    let selected = gateway
        .with_rel(&envelope.owners[0].token, |runtime| {
            Ok(runtime
                .cva
                .memory_ids()
                .into_iter()
                .map(|memory_id| MemoryRef {
                    owner_id: envelope.owners[0].token.owner_id.clone(),
                    memory_id,
                })
                .collect::<BTreeSet<_>>())
        })
        .unwrap();
    let mut controller = AgenticMemoryController::new();
    controller.selected_ids = selected.clone();
    let prepared = gateway
        .memory_query(
            &instance,
            &mut controller,
            "needle",
            "all-selected-no-route",
        )
        .unwrap();
    assert_eq!(
        prepared.payload().outcome,
        AgenticMemoryOutcome::AlreadyProvided(selected)
    );
    assert!(calls.lock().unwrap().is_empty());
}
