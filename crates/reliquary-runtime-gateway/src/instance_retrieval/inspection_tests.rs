use super::*;

struct PreviewDecision {
    invalid: AtomicBool,
}
impl DecisionEndpoint for PreviewDecision {
    fn model(&self) -> &str {
        "fixture-preview"
    }

    fn evaluate(&self, state: &Value, shape: &Value) -> Result<Value, DecisionEndpointError> {
        if state["operation"] == "refine_query_rewrite" {
            if self.invalid.load(Ordering::Relaxed) {
                return Ok(json!({"version": 1, "query": ""}));
            }
            assert_eq!(state["current_query"], "needle");
            return Ok(json!({"version": 1, "query": "needle first exact body"}));
        }
        Approve.evaluate(state, shape)
    }
}

#[test]
fn inspection_refines_and_repeats_without_pending_delivery_or_owner_writes() {
    let fixture = Fixture::new();
    fixture.gateway.quiesce_owner(fixture.owner).unwrap();
    let bytes_before = std::fs::read(&fixture.path).unwrap();
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
    let turn = alice
        .begin_memory_retrieval(fixture.owner, "inspect", RetrievalMode::Inspect)
        .unwrap();
    let endpoint = Arc::new(PreviewDecision {
        invalid: AtomicBool::new(false),
    });
    let route = MemoryRetrievalRoute::from_endpoint(endpoint.clone());
    let first = alice
        .evaluate_agentic_memory(
            &turn,
            &route,
            RetrievalAction::Query("needle".into()),
            || Ok(None),
        )
        .unwrap();
    assert!(first.prepared.is_none());
    let first = first.preview.unwrap();
    assert_eq!(first.memories[0].content, "needle first exact body");
    assert_eq!(first.memories[0].revision, 1);

    endpoint.invalid.store(true, Ordering::Relaxed);
    let refine = || {
        alice.evaluate_agentic_memory(
            &turn,
            &route,
            RetrievalAction::Refine("be precise".into()),
            || panic!("refine cannot embed"),
        )
    };
    assert!(refine().is_err());
    assert_eq!(turn.state.lock().unwrap().turn.query(), Some("needle"));
    endpoint.invalid.store(false, Ordering::Relaxed);
    let outcome = refine().unwrap();
    assert!(outcome.prepared.is_none());
    assert!(outcome.already_provided.is_empty());
    let refined = outcome.preview.unwrap();
    assert_eq!(refined.query, "needle first exact body");
    assert_eq!(refined.memories, first.memories);

    let anchor = parse_memory_id(&first.memories[0].memory_id).unwrap();
    let traversed = alice
        .evaluate_agentic_memory(
            &turn,
            &route,
            RetrievalAction::Traverse(vec![anchor]),
            || panic!("traverse cannot embed"),
        )
        .unwrap();
    assert_eq!(traversed.anchors[0].anchor, anchor);
    assert!(traversed.prepared.is_none());
    let host = Arc::new(Host::default());
    let router = connect(&fixture.gateway, host);
    assert!(matches!(
        alice.recover_agentic_memory_delivery(&turn, "unknown", &router),
        Err(GatewayError::InvalidPreparedRetrieval)
    ));
    let state = turn.state.lock().unwrap();
    assert!(state.turn.selected().is_empty());
    assert!(state.pending_use_id.is_none());
    drop(state);
    fixture.gateway.quiesce_owner(fixture.owner).unwrap();
    assert_eq!(bytes_before, std::fs::read(&fixture.path).unwrap());
}

#[test]
fn cancellation_invalidates_inflight_evaluation_and_old_preparation() {
    let fixture = Fixture::new();
    let alice = fixture.alice();
    let turn = alice
        .begin_memory_retrieval(fixture.owner, "cancel", RetrievalMode::Deliver)
        .unwrap();
    let query = || RetrievalAction::Query("needle".into());
    let cancelled = alice.evaluate_agentic_memory(&turn, &route(), query(), || {
        alice.cancel_memory_retrieval(&turn)?;
        Ok(None)
    });
    assert!(matches!(cancelled, Err(GatewayError::StaleRetrieval)));
    assert!(turn.state.lock().unwrap().turn.query().is_none());

    let prepare = || {
        alice
            .prepare_agentic_memory_delivery(&turn, &route(), query(), None, None)
            .unwrap()
            .prepared
            .unwrap()
    };
    let old = prepare();
    assert_eq!(
        alice
            .preview_agentic_memory_delivery(&turn, &old)
            .unwrap()
            .memories
            .len(),
        1
    );
    alice.cancel_memory_retrieval(&turn).unwrap();
    let current = prepare();
    assert_eq!(current.stable_id(), old.stable_id());
    assert!(matches!(
        alice.preview_agentic_memory_delivery(&turn, &old),
        Err(GatewayError::InvalidPreparedRetrieval)
    ));
    let host = Arc::new(Host::default());
    let router = connect(&fixture.gateway, host.clone());
    assert!(alice.deliver_agentic_memory(&turn, &old, &router).is_err());
    assert_eq!(host.calls.load(Ordering::Relaxed), 0);
    assert!(
        alice
            .deliver_agentic_memory(&turn, &current, &router)
            .unwrap()
    );
}

#[test]
fn remount_invalidates_an_inspection_controller() {
    let fixture = Fixture::new();
    let alice = fixture.alice();
    let turn = alice
        .begin_memory_retrieval(fixture.owner, "remount", RetrievalMode::Inspect)
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
    assert!(matches!(
        alice.evaluate_agentic_memory(
            &turn,
            &route(),
            RetrievalAction::Query("needle".into()),
            || panic!("remounted turn cannot embed")
        ),
        Err(GatewayError::StaleRetrieval)
    ));
}

#[test]
fn projected_search_distinguishes_lexical_hybrid_and_semantic_routing() {
    let fixture = Fixture::new();
    let alice = fixture.alice();
    let endpoint = crate::g4_retrieval_acceptance::Embed {
        calls: Default::default(),
        dimensions: 2,
    };
    let profile = alice
        .publish_embedding_profile(fixture.owner, &endpoint)
        .unwrap();
    let snapshot = alice.retrieval_snapshot(fixture.owner, None).unwrap();
    let ids = snapshot
        .memories
        .iter()
        .map(|m| m.revision.record.id)
        .collect::<Vec<_>>();
    let prepared = alice
        .prepare_owner_memory_vectors(fixture.owner, profile, &ids, &endpoint)
        .unwrap();
    alice.publish_owner_memory_vectors(&prepared).unwrap();

    let lexical = alice
        .search_rel_memories(fixture.owner, "needle", SearchMode::Lexical, None, None)
        .unwrap();
    assert_eq!(lexical.memories.len(), 1);
    assert!(lexical.memories[0].semantic_score.is_none());

    let hybrid = alice
        .search_rel_memories(
            fixture.owner,
            "needle",
            SearchMode::Hybrid,
            Some(1),
            Some((profile, vec![0.0, 1.0])),
        )
        .unwrap();
    assert_eq!(hybrid.memories[0].content, "other second exact body");
    assert!(hybrid.memories[0].lexical_score.is_none());
    assert_eq!(hybrid.memories[0].semantic_score, Some(1.0));
    assert_eq!(hybrid.cut.profile_id, Some(hex(&profile.0)));
    assert!(hybrid.truncated);

    let semantic = alice
        .search_rel_memories(
            fixture.owner,
            "needle",
            SearchMode::Semantic,
            None,
            Some((profile, vec![0.0, 1.0])),
        )
        .unwrap();
    assert_eq!(semantic.semantic_routing, Some("global_exact"));
    assert!(semantic.fallback_reason.is_some());
    assert_eq!(semantic.memories.len(), 2);
    assert!(
        semantic
            .memories
            .iter()
            .all(|hit| hit.lexical_score.is_none() && hit.semantic_score == Some(1.0))
    );
    assert_eq!(semantic.memories_without_vectors, Some(0));

    let turn = alice
        .begin_memory_retrieval_turn(fixture.owner, "recover-profile")
        .unwrap();
    let prepared = alice
        .evaluate_agentic_memory(
            &turn,
            &route(),
            RetrievalAction::Query("needle".into()),
            || Ok(Some((profile, vec![0.0, 1.0]))),
        )
        .unwrap()
        .prepared
        .unwrap();
    let host = Arc::new(Host::default());
    let router = connect(&fixture.gateway, host.clone());
    host.deliver_memory(&MemoryDeliveryOperation {
        stable_id: prepared.use_id.clone(),
        payload_digest: prepared.digest,
        payload: prepared.payload.clone(),
    })
    .unwrap();
    assert!(
        alice
            .recover_agentic_memory_delivery(&turn, prepared.stable_id(), &router)
            .unwrap()
    );
    assert_eq!(turn.state.lock().unwrap().profile, Some(profile));
    assert_eq!(host.calls.load(Ordering::Relaxed), 1);
}
