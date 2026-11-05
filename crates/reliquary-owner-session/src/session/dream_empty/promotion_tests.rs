use super::initial_tests::{fixture_state, graph};
use super::*;
use std::{collections::BTreeMap, fs};

#[test]
fn synced_no_candidate_promotion_replays_with_admission_and_settlement_rel_phy() {
    for rel in [true, false] {
        let (path, owner, older, source) = fixture_state(rel, true);
        let admission;
        if rel {
            let mut w = RelWriter::open_path(&path.0, graph(owner), [], []).unwrap();
            admission = w
                .begin_initial_dream_pass("promotion".into(), source, 4)
                .unwrap();
            let record = InitialDreamSettlement {
                owner,
                pass_id: "promotion".into(),
                source: source.id,
                revision: 1,
                body: source.body_id,
                graph_version: 0,
                accepted_turn: 4,
                community_generation: None,
                roots: Default::default(),
                credit_roots: Default::default(),
                admit: true,
                freshness_effects: w
                    .session
                    .freshness
                    .prepare_initial_admission(FreshnessMemoryId(source.id.0), 4, 4)
                    .unwrap(),
                memory_delta: Some(
                    w.session
                        .common
                        .memories
                        .prepare_initial_promotion(
                            source.id,
                            1,
                            InitialDreamSettlement::promotion_mutation(source.id),
                        )
                        .unwrap(),
                ),
            };
            w.container.append(&record.encode().unwrap()).unwrap();
            w.container.sync().unwrap();
            assert_eq!(
                w.session
                    .common
                    .memories
                    .history(source.id)
                    .unwrap()
                    .revisions
                    .len(),
                1
            );
            assert!(!w.session.common.dreams.journal().is_settled("promotion"));
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), []).unwrap();
            admission = w
                .begin_initial_dream_pass("promotion".into(), source, 4)
                .unwrap();
            let record = InitialDreamSettlement {
                owner,
                pass_id: "promotion".into(),
                source: source.id,
                revision: 1,
                body: source.body_id,
                graph_version: 0,
                accepted_turn: 4,
                community_generation: None,
                roots: Default::default(),
                credit_roots: Default::default(),
                admit: true,
                freshness_effects: vec![],
                memory_delta: Some(
                    w.common
                        .memories
                        .prepare_initial_promotion(
                            source.id,
                            1,
                            InitialDreamSettlement::promotion_mutation(source.id),
                        )
                        .unwrap(),
                ),
            };
            w.container.append(&record.encode().unwrap()).unwrap();
            w.container.sync().unwrap();
            assert_eq!(
                w.common
                    .memories
                    .history(source.id)
                    .unwrap()
                    .revisions
                    .len(),
                1
            );
        }
        let before = fs::read(&path.0).unwrap();
        let c = Container::open_read_only(&path.0).unwrap();
        assert!(MemoryStore::open(&c).is_err());
        drop(c);
        if rel {
            let mut w =
                RelWriter::open_path(&path.0, graph(owner), [], [admission.clone()]).unwrap();
            let history = w.session.common.memories.history(source.id).unwrap();
            assert_eq!(history.revisions.len(), 2);
            assert_eq!(history.revisions[1].record.lifecycle_state, "knowledge");
            assert_eq!(
                history.revisions[0].record.body_id,
                history.revisions[1].record.body_id
            );
            assert_eq!(
                w.session
                    .freshness
                    .record_at(FreshnessMemoryId(source.id.0), 5)
                    .unwrap()
                    .unwrap()
                    .score,
                100
            );
            assert!(
                !w.commit_initial_dream_pass("promotion", source, 0, 4, BTreeMap::new(), true)
                    .unwrap()
            );
            assert_eq!(
                w.begin_initial_dream_pass("promotion".into(), source, 4)
                    .unwrap(),
                admission
            );
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), [admission.clone()]).unwrap();
            let history = w.common.memories.history(source.id).unwrap();
            assert_eq!(history.revisions.len(), 2);
            assert_eq!(history.revisions[1].record.lifecycle_state, "knowledge");
            assert!(w.common.dreams.journal().is_settled("promotion"));
            assert!(
                !w.commit_initial_dream_pass("promotion", source, 0, 4, BTreeMap::new(), true)
                    .unwrap()
            );
            assert_eq!(
                w.begin_initial_dream_pass("promotion".into(), source, 4)
                    .unwrap(),
                admission
            );
        }
        assert_eq!(before, fs::read(&path.0).unwrap());
        // Later graph work must not reopen the source's first cycle or reset
        // its existing admission, even when retried using the old graph cut.
        let mut c = Container::open_writer(&path.0).unwrap();
        let admission_graph = graph(owner);
        let mut g = reliquary_graph::GraphStore::open(&c, &admission_graph).unwrap();
        g.put(
            &mut c,
            &GraphAdmission::for_changes(owner, [source.id.0, older.id.0], []),
            &[reliquary_graph::RelationChange {
                source: NodeRef::Memory(source.id.0),
                target: NodeRef::Memory(older.id.0),
                kind: reliquary_graph::RelationKind::Factual,
                origin: reliquary_graph::RelationOrigin::User,
                active: true,
            }],
        )
        .unwrap();
        c.sync().unwrap();
        drop(c);
        let before = fs::read(&path.0).unwrap();
        if rel {
            let mut w =
                RelWriter::open_path(&path.0, graph(owner), [], [admission.clone()]).unwrap();
            assert!(
                !w.commit_initial_dream_pass("promotion", source, 0, 4, BTreeMap::new(), true)
                    .unwrap()
            );
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), [admission.clone()]).unwrap();
            assert!(
                !w.commit_initial_dream_pass("promotion", source, 0, 4, BTreeMap::new(), true)
                    .unwrap()
            );
        }
        assert_eq!(before, fs::read(&path.0).unwrap());
    }
}

#[test]
fn replay_refuses_promotion_delta_with_unrelated_mutation_identity() {
    let (path, owner, _, source) = fixture_state(false, true);
    let mut w = PhyWriter::open_path(&path.0, &graph(owner), []).unwrap();
    let admission = w
        .begin_initial_dream_pass("promotion".into(), source, 4)
        .unwrap();
    let record = InitialDreamSettlement {
        owner,
        pass_id: "promotion".into(),
        source: source.id,
        revision: 1,
        body: source.body_id,
        graph_version: 0,
        accepted_turn: 4,
        community_generation: None,
        roots: Default::default(),
        credit_roots: Default::default(),
        admit: true,
        freshness_effects: vec![],
        memory_delta: Some(
            w.common
                .memories
                .prepare_initial_promotion(source.id, 1, "fabricated".into())
                .unwrap(),
        ),
    };
    w.container.append(&record.encode().unwrap()).unwrap();
    w.container.sync().unwrap();
    drop(w);
    let before = fs::read(&path.0).unwrap();
    assert!(PhySession::open_path(&path.0, &graph(owner), [admission]).is_err());
    assert_eq!(before, fs::read(&path.0).unwrap());
}
