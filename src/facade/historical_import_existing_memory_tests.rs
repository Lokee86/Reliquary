//! Step 27: genuine native historical Dream evidence can update an old Memory.
use super::*;

#[test]
fn native_import_dream_effect_updates_only_eligible_preexisting_memory() {
    let (path, mut cva, _) =
        setup_origin("historical-import-existing-memory.rel", EpisodeOrigin::Live);
    let old = process(&mut cva, 1, 110).created[0].id;
    cva.freshness_admit(old, cva.rel_turn_count()).unwrap();
    settle_initial_pass(&mut cva, old, 120);
    let unrelated = unrelated_memory(&mut cva);
    cva.freshness_admit(unrelated, cva.rel_turn_count())
        .unwrap();
    settle_initial_pass(&mut cva, unrelated, 121);

    // Age both eligible recipients below saturation before the import begins.
    let mut parent = None;
    for index in 0..600 {
        let id = format!("existing-memory-decay-{index}");
        cva.append_node(
            id.clone(),
            "live-decay".into(),
            parent,
            "user".into(),
            130 + index,
            "Activity ages preexisting Memories before import.",
        )
        .unwrap();
        parent = Some(id);
    }
    let base_cut = cva.rel_turn_count();
    let old_before = cva.freshness_record_at(old, base_cut).unwrap().unwrap();
    let unrelated_before = cva
        .freshness_record_at(unrelated, base_cut)
        .unwrap()
        .unwrap();
    let unrelated_resident_before = cva.freshness_record(unrelated);
    assert!(old_before.score < 100 && unrelated_before.score < 100);
    let pre_import_bytes = std::fs::read(&path).unwrap();

    cva.import_native(&imported_snapshot("existing-memory-dream"), 600)
        .unwrap();
    process_import_episode(&mut cva, 610);
    let scope = cva.import_epoch_scope().unwrap();
    assert_eq!(scope.cohort.len(), 1);
    assert!(!scope.cohort.contains(&old));
    assert!(!scope.cohort.contains(&unrelated));
    let source = *scope
        .cohort
        .iter()
        .next()
        .expect("import publishes a Memory");

    // Record a complete accepted first-cycle proof for the imported Memory,
    // with the eligible pre-import Memory as its strictly earlier Dream root.
    let pass = crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), source);
    let first_graph_version = cva.graph_version();
    cva.container.set_owner_write_scope(true);
    cva.begin_initial_dream_pass(
        pass.clone(),
        source,
        1,
        cva.rel_turn_count(),
        BTreeSet::from([old]),
        first_graph_version,
    )
    .unwrap();
    cva.set_memory_relation_with_origin(
        source,
        old,
        GraphRelationKind::Topical,
        true,
        GraphRelationOrigin::Dream,
        first_graph_version,
    )
    .unwrap();
    cva.reconcile_dream_lifecycle(source).unwrap();
    cva.accept_initial_dream_settlement(&pass, cva.rel_turn_count())
        .unwrap();
    crate::dream_freshness::settle_initial_pass(&mut cva, source, true, 620).unwrap();
    cva.container.set_owner_write_scope(false);

    let admission = input::admission_proof_scoped(&mut cva, Some(&scope)).unwrap();
    for id in [old, unrelated] {
        assert!(admission.memory_inputs[&id].credit_eligible);
        assert!(admission.memory_inputs[&id].availability.is_some());
        assert!(admission.initial.contains_key(&id));
    }

    let frozen = cva.freeze_historical_import().unwrap();
    let event_plan = frozen.events.as_ref().unwrap();
    assert!(event_plan.semantic.events.is_empty());
    assert_eq!(event_plan.dream.events.len(), 1);
    let event = &event_plan.dream.events[0];
    assert_eq!(event.source_id, source);
    assert_eq!(
        event.event_id,
        crate::freshness::historical_dream::historical_dream_event_id(
            cva.owner_uuid().unwrap(),
            frozen.import_id(),
            source,
        )
    );
    assert!(event.first_cycle_evidence_digest.is_some());
    assert_eq!(
        event.coverage,
        crate::freshness::historical_dream::HistoricalDreamCoverage::ActualFirstCycle
    );
    assert_eq!(event.propagation.roots, BTreeMap::from([(old, 50)]));
    assert_eq!(
        event.propagation.position,
        frozen.initial_records()[&source].admitted_at_turn.unwrap()
    );
    assert!(event.propagation.position > base_cut);
    assert!(old_before.admitted_at_turn.unwrap() < event.propagation.position);
    assert!(event.propagation.credit_excluded.contains(&source));
    let prepared = frozen.prepare_native().unwrap();
    let base = cva.historical_import_base(&scope).unwrap();
    assert_eq!(frozen.initial_records()[&old], base[&old]);
    assert!(!frozen.initial_records().contains_key(&unrelated));
    assert!(prepared.settlement.cohort.contains(&old));
    assert!(!prepared.settlement.cohort.contains(&unrelated));
    assert!(prepared.records().contains_key(&old));
    assert!(!prepared.records().contains_key(&unrelated));
    assert_eq!(base[&old], old_before);
    assert_eq!(base[&unrelated], unrelated_before);
    assert!(
        prepared
            .settlement
            .superseded
            .contains(&crate::dream_freshness::initial_pass_id(
                cva.owner_uuid().unwrap(),
                source,
            ))
    );

    // Compute the whole affected result and the no-Dream counterfactual from
    // the authenticated base cut. The unrelated Memory stays outside the
    // sparse replacement set and is checked at its own as-of cut below.
    let policy = FreshnessPolicy::default_v1();
    let mut expected = frozen.initial_records().clone();
    expected
        .get_mut(&old)
        .unwrap()
        .reinforce_with_policy(50, event.propagation.position, &policy)
        .unwrap();
    for record in expected.values_mut() {
        record
            .settle_with_policy(prepared.settlement.settlement_cut, &policy)
            .unwrap();
    }
    let mut counterfactual = frozen.initial_records().clone();
    for record in counterfactual.values_mut() {
        record
            .settle_with_policy(prepared.settlement.settlement_cut, &policy)
            .unwrap();
    }
    assert_eq!(prepared.records(), &expected);
    assert_ne!(expected[&old], counterfactual[&old]);
    assert!(expected[&old].score < 100);
    assert_eq!(expected[&old].admitted_at_turn, old_before.admitted_at_turn);
    assert!(expected[&old].score > counterfactual[&old].score);
    assert!(counterfactual[&old].score <= old_before.score);
    let mut expected_unrelated = base[&unrelated];
    expected_unrelated
        .settle_with_policy(prepared.settlement.settlement_cut, &policy)
        .unwrap();

    assert_eq!(cva.freshness_record(unrelated), unrelated_resident_before);
    assert!(cva.commit_historical_import(&prepared).unwrap());
    let accepted_bytes = std::fs::read(&path).unwrap();
    let old_settled = expected[&old];
    assert_eq!(
        cva.freshness_record_at(old, prepared.settlement.settlement_cut)
            .unwrap(),
        Some(old_settled)
    );
    assert_eq!(
        cva.freshness_record_at(unrelated, prepared.settlement.settlement_cut)
            .unwrap(),
        Some(expected_unrelated)
    );
    assert_eq!(cva.freshness_record(unrelated), unrelated_resident_before);
    assert!(accepted_bytes.starts_with(&pre_import_bytes));
    assert!(!cva.commit_historical_import(&prepared).unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), accepted_bytes);
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened
            .freshness_record_at(old, prepared.settlement.settlement_cut)
            .unwrap(),
        Some(old_settled)
    );
    assert_eq!(
        reopened
            .freshness_record_at(unrelated, prepared.settlement.settlement_cut)
            .unwrap(),
        Some(expected_unrelated)
    );
    assert_eq!(
        reopened.freshness_record(unrelated),
        unrelated_resident_before
    );
    assert!(
        !reopened
            .reconstruct_historical_freshness(Default::default())
            .unwrap()
            .committed
    );
    assert_eq!(std::fs::read(&path).unwrap(), accepted_bytes);
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}
