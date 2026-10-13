use super::*;
#[test]
fn historical_matrix_semantic_live_local_vs_native_propagation_and_stale_community() {
    for stale in [false, true] {
        let (path, mut cva, root) = historical_import_fixture(&format!("c9-semantic-{stale}.rel"));
        let sink = publish(&mut cva, "C9 sink", Some(("c1", "u0")), true);
        edge(
            &mut cva,
            root,
            sink,
            GraphRelationKind::Factual,
            GraphRelationOrigin::User,
        );
        let snapshot = cva.refresh_communities_leiden().unwrap();
        assert_eq!(
            snapshot.community_for(root).unwrap().id,
            snapshot.community_for(sink).unwrap().id
        );
        if stale {
            edge(
                &mut cva,
                root,
                sink,
                GraphRelationKind::Causal,
                GraphRelationOrigin::User,
            );
        }
        vectors(&mut cva, false);
        cva.sync().unwrap();
        let live_path = path.with_file_name("live.rel");
        fs::copy(&path, &live_path).unwrap();
        let mut live = Cva::open(&live_path).unwrap();
        live.set_semantic_freshness_enabled(true).unwrap();
        let untouched = live.freshness_record(sink);
        tail(&mut live);
        let generation = vectors(&mut live, true);
        let report = live.process_semantic_freshness(generation.id, 400).unwrap();
        assert_eq!(report.reinforced_memories, 1);
        assert_eq!(live.freshness_record(sink), untouched);
        assert_eq!(
            live.freshness_record_at(sink, 332).unwrap().unwrap().score,
            70
        );
        tail(&mut cva);
        vectors(&mut cva, true);
        let frozen = cva.freeze_historical_import().unwrap();
        let plan = frozen.events.as_ref().unwrap();
        assert!(plan.dream.events.is_empty());
        assert_eq!(plan.semantic.events.len(), 1);
        assert_eq!(plan.semantic.events[0].source_position, 332);
        assert_eq!(plan.semantic.events[0].effects.len(), 1);
        assert_eq!(plan.semantic.events[0].effects[0].memory_id, root);
        let prepared = frozen.prepare_native().unwrap();
        assert_eq!(prepared.records()[&sink].score, if stale { 67 } else { 72 });
        let expected = prepared.records().clone();
        let reopened = accept_and_reopen(&path, cva);
        for (id, record) in expected {
            assert_eq!(reopened.freshness_record(id), Some(record));
        }
        assert_eq!(
            reopened
                .freshness_record_at(sink, 332)
                .unwrap()
                .unwrap()
                .score,
            if stale { 67 } else { 72 }
        );
    }
}
#[test]
fn historical_matrix_dream_future_and_unsupported_bridges_transmit_without_credit() {
    for unsupported in [false, true] {
        let (path, mut cva, _) = historical_import_fixture(&format!("c9-bridge-{unsupported}.rel"));
        let root = publish(&mut cva, "C9 older root", Some(("c1", "u0")), true);
        let sink = publish(&mut cva, "C9 older sink", Some(("c1", "u0")), true);
        let source = historical_events::linked_source(&mut cva, root, false);
        tail(&mut cva);
        let bridge = publish(
            &mut cva,
            "C9 bridge",
            if unsupported {
                None
            } else {
                Some(("c3", "u149"))
            },
            true,
        );
        let untouched = cva.freshness_record(bridge);
        edge(
            &mut cva,
            root,
            bridge,
            GraphRelationKind::Factual,
            GraphRelationOrigin::User,
        );
        edge(
            &mut cva,
            bridge,
            sink,
            GraphRelationKind::Factual,
            GraphRelationOrigin::User,
        );
        let frozen = cva.freeze_historical_import().unwrap();
        let inputs = frozen.propagation.as_ref().unwrap();
        assert!(inputs.adjacency[&root].contains(&bridge));
        assert!(inputs.adjacency[&bridge].contains(&sink));
        let event = frozen
            .events
            .as_ref()
            .unwrap()
            .dream
            .events
            .iter()
            .find(|e| e.source_id == source)
            .unwrap();
        assert_eq!(event.propagation.position, 32);
        let result = crate::freshness::historical_propagation::propagate_historical_events(
            &[event.propagation.clone()],
            &inputs.adjacency,
            &inputs.memberships,
            &inputs.authorized,
            &inputs.availability,
            3,
            &FreshnessPolicy::default_v1(),
        )
        .unwrap();
        assert_eq!(result[0].candidates.get(&sink), Some(&30));
        assert!(!result[0].candidates.contains_key(&bridge));
        let prepared = frozen.prepare_native().unwrap();
        assert_eq!(prepared.records()[&sink].score, 70);
        let reopened = accept_and_reopen(&path, cva);
        assert_eq!(reopened.freshness_record(sink).unwrap().score, 70);
        if unsupported {
            assert_eq!(reopened.freshness_record(bridge), untouched);
        } else {
            assert_eq!(
                reopened.freshness_record(bridge).unwrap().admitted_at_turn,
                Some(331)
            );
        }
    }
}
