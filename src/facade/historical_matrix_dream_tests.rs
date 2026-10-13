use super::*;
use crate::freshness::historical_dream::HistoricalDreamCoverage;

#[test]
fn historical_matrix_native_verified_supersession_resolves_without_rewriting_memory_or_graph() {
    let (path, mut cva, _) = historical_import_fixture("c9-supersession-authority.rel");
    let old = publish(
        &mut cva,
        "Earlier supported state",
        Some(("c1", "u0")),
        true,
    );
    let next = publish(
        &mut cva,
        "Accepted replacement state",
        Some(("c2", "a7")),
        true,
    );
    edge(
        &mut cva,
        next,
        old,
        GraphRelationKind::Supersedes,
        GraphRelationOrigin::Dream,
    );
    let memory = cva.memory(old).unwrap();
    let mut changed = draft(memory.clone(), "c9-accepted-supersession");
    changed.archived = true;
    changed.lifecycle_state = "archived".into();
    changed.superseded_by = Some(next);
    cva.publish_memory(Some(old), memory.revision, changed)
        .unwrap();
    assert_eq!(
        cva.historical_dream_representatives(old, &BTreeSet::from([old]))
            .unwrap(),
        BTreeMap::from([(old, next)])
    );
    let graph = cva.semantic_graph_relations();
    let history = cva.memory_revision(old, 1).unwrap();
    tail(&mut cva);
    let mut reopened = accept_and_reopen(&path, cva);
    assert_eq!(reopened.semantic_graph_relations(), graph);
    assert_eq!(reopened.memory_revision(old, 1).unwrap(), history);
}

#[test]
fn historical_matrix_native_supersession_mismatch_and_cycle_remain_unresolved() {
    for cycle in [false, true] {
        let (path, mut cva, _) =
            historical_import_fixture(&format!("c9-supersession-invalid-{cycle}.rel"));
        let old = publish(&mut cva, "Unresolved old state", Some(("c1", "u0")), true);
        let next = publish(
            &mut cva,
            "Unresolved replacement state",
            Some(("c2", "a7")),
            true,
        );
        if cycle {
            edge(
                &mut cva,
                next,
                old,
                GraphRelationKind::Supersedes,
                GraphRelationOrigin::Dream,
            );
            edge(
                &mut cva,
                old,
                next,
                GraphRelationKind::Supersedes,
                GraphRelationOrigin::Dream,
            );
        }
        for (id, target) in if cycle {
            vec![(old, next), (next, old)]
        } else {
            vec![(old, next)]
        } {
            let memory = cva.memory(id).unwrap();
            let mut changed = draft(memory.clone(), &format!("c9-invalid-supersession:{id:?}"));
            changed.archived = true;
            changed.lifecycle_state = "archived".into();
            changed.superseded_by = Some(target);
            cva.publish_memory(Some(id), memory.revision, changed)
                .unwrap();
        }
        assert!(
            cva.historical_dream_representatives(old, &BTreeSet::from([old]))
                .is_err()
        );
        rejected(&mut cva, &path, "duplicate");
    }
}

#[test]
fn historical_matrix_native_duplicate_and_supersession_authority_compose_after_reopen() {
    let (path, mut cva, _) = historical_import_fixture("c9-composed-authority.rel");
    let alias = publish(&mut cva, "Retired alias", Some(("c1", "u0")), true);
    let old = publish(&mut cva, "Retired representative", Some(("c2", "a7")), true);
    let next = publish(&mut cva, "Retired replacement", Some(("c1", "u0")), true);
    let peer = publish(&mut cva, "Replacement duplicate", Some(("c2", "a7")), true);
    let terminal = publish(
        &mut cva,
        "Current accepted authority",
        Some(("c1", "u0")),
        true,
    );
    edge(
        &mut cva,
        alias,
        old,
        GraphRelationKind::DuplicateOf,
        GraphRelationOrigin::Dream,
    );
    edge(
        &mut cva,
        next,
        old,
        GraphRelationKind::Supersedes,
        GraphRelationOrigin::Dream,
    );
    edge(
        &mut cva,
        next,
        peer,
        GraphRelationKind::DuplicateOf,
        GraphRelationOrigin::Dream,
    );
    edge(
        &mut cva,
        terminal,
        peer,
        GraphRelationKind::Supersedes,
        GraphRelationOrigin::Dream,
    );
    for (id, successor) in [
        (alias, None),
        (old, Some(next)),
        (next, None),
        (peer, Some(terminal)),
    ] {
        let memory = cva.memory(id).unwrap();
        let mut changed = draft(memory.clone(), &format!("c9-composed-authority:{id:?}"));
        changed.archived = true;
        changed.lifecycle_state = "archived".into();
        changed.superseded_by = successor;
        cva.publish_memory(Some(id), memory.revision, changed)
            .unwrap();
    }
    let graph = cva.semantic_graph_relations();
    let first = cva.memory_revision(alias, 1).unwrap();
    assert_eq!(
        cva.historical_dream_representatives(alias, &BTreeSet::from([alias]))
            .unwrap(),
        BTreeMap::from([(alias, terminal)])
    );
    tail(&mut cva);
    let mut reopened = accept_and_reopen(&path, cva);
    assert_eq!(
        reopened
            .historical_dream_representatives(alias, &BTreeSet::from([alias]))
            .unwrap(),
        BTreeMap::from([(alias, terminal)])
    );
    assert_eq!(reopened.semantic_graph_relations(), graph);
    assert_eq!(reopened.memory_revision(alias, 1).unwrap(), first);
}
#[test]
fn historical_matrix_native_no_link_and_missing_proof_never_invent_final_graph_fallback() {
    for legacy_marker in [false, true] {
        let (path, mut cva, _) =
            historical_import_fixture(&format!("c9-dream-proof-{legacy_marker}.rel"));
        let root = publish(&mut cva, "C9 older root", Some(("c1", "u0")), true);
        let source = publish(
            &mut cva,
            "C9 proof source",
            Some(("c2", "a7")),
            !legacy_marker,
        );
        if legacy_marker {
            let pass = crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), source);
            let payload = cva.dream_freshness.replay_settled_payload(&pass).unwrap();
            cva.container.append(&payload).unwrap();
            cva.dream_freshness.ingest(&payload).unwrap();
            assert!(cva.initial_dream_pass_settled(&pass));
            assert!(
                !cva.historical_completed_initial_dream_passes()
                    .unwrap()
                    .contains_key(&source)
            );
        }
        edge(
            &mut cva,
            source,
            root,
            GraphRelationKind::Factual,
            GraphRelationOrigin::Dream,
        );
        tail(&mut cva);
        let frozen = cva.freeze_historical_import().unwrap();
        let batch = frozen.events.as_ref().unwrap();
        assert!(batch.dream.events.is_empty());
        assert_eq!(
            batch.dream.coverage[&source],
            if legacy_marker {
                HistoricalDreamCoverage::Unsupported
            } else {
                HistoricalDreamCoverage::VerifiedNoLink
            }
        );
        assert_eq!(frozen.prepare_native().unwrap().records()[&root].score, 67);
        let reopened = accept_and_reopen(&path, cva);
        assert_eq!(reopened.freshness_record(root).unwrap().score, 67);
    }
}
#[test]
fn historical_matrix_native_duplicate_alias_contributes_one_actual_event_and_unadmitted_projection()
{
    let (path, mut cva, target) = historical_import_fixture("c9-duplicate.rel");
    let alias = historical_events::linked_source(&mut cva, target, true);
    let frozen = cva.freeze_historical_import().unwrap();
    let batch = frozen.events.as_ref().unwrap();
    assert_eq!(batch.dream.events.len(), 1);
    assert_eq!(batch.dream.events[0].source_id, alias);
    assert_eq!(
        batch.dream.events[0].propagation.roots,
        BTreeMap::from([(target, 50)])
    );
    assert!(
        batch.dream.events[0]
            .propagation
            .credit_excluded
            .contains(&alias)
    );
    let expected = frozen.prepare_native().unwrap().records().clone();
    let reopened = accept_and_reopen(&path, cva);
    assert_eq!(
        reopened.freshness_record(alias),
        Some(FreshnessRecord::created())
    );
    for (id, record) in expected {
        assert_eq!(reopened.freshness_record(id), Some(record));
    }
}
#[test]
fn historical_matrix_native_unresolved_duplicate_authority_rejects_without_projection() {
    let (path, mut cva, _) = historical_import_fixture("c9-ambiguous-duplicate.rel");
    let root = publish(
        &mut cva,
        "C9 unresolved duplicate",
        Some(("c1", "u0")),
        true,
    );
    let other = publish(&mut cva, "C9 ambiguous canonical", Some(("c1", "u0")), true);
    edge(
        &mut cva,
        root,
        other,
        GraphRelationKind::DuplicateOf,
        GraphRelationOrigin::Dream,
    );
    for id in [root, other] {
        let memory = cva.memory(id).unwrap();
        let mut changed = draft(memory.clone(), &format!("c9-archive:{id:?}"));
        changed.archived = true;
        cva.publish_memory(Some(id), memory.revision, changed)
            .unwrap();
    }
    rejected(&mut cva, &path, "duplicate");
}
