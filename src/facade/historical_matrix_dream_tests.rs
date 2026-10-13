use super::*;
use crate::freshness::historical_dream::HistoricalDreamCoverage;
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
