use super::initial_tests::*;
use super::*;
use reliquary_graph::{RelationChange, RelationKind, RelationOrigin};
use std::fs;
#[test]
fn replay_rejects_foreign_stale_fabricated_and_incomplete_first_cycle_carriers() {
    for variant in 0..7 {
        let (path, owner, _, source) = fixture(true);
        let mut w = RelWriter::open_path(&path.0, graph(owner), [], []).unwrap();
        let admission = w
            .begin_initial_dream_pass("initial".into(), source, 4)
            .unwrap();
        let mut record = InitialDreamSettlement {
            owner,
            pass_id: "initial".into(),
            source: source.id,
            revision: 1,
            body: source.body_id,
            graph_version: 0,
            accepted_turn: 4,
            community_generation: None,
            memory_delta: None,
            roots: Default::default(),
            credit_roots: Default::default(),
            admit: true,
            freshness_effects: w
                .session
                .freshness
                .prepare_initial_admission(FreshnessMemoryId(source.id.0), 4, 4)
                .unwrap(),
        };
        match variant {
            0 => record.owner = [99; 16],
            1 => record.body.0 = [99; 32],
            2 => record.graph_version = 9,
            3 => record.accepted_turn = 3,
            4 => record.freshness_effects.clear(),
            5 => {
                record.roots.insert(MemoryId([99; 32]));
            }
            _ => record.community_generation = Some(1),
        }
        w.container.append(&record.encode().unwrap()).unwrap();
        w.container.sync().unwrap();
        drop(w);
        let before = fs::read(&path.0).unwrap();
        assert!(
            RelSession::open_path(&path.0, &graph(owner), [], [admission]).is_err(),
            "variant {variant}"
        );
        assert_eq!(before, fs::read(&path.0).unwrap());
    }
}

#[test]
fn user_edges_and_unreceipted_dream_rewires_are_never_initial_roots() {
    for origin in [RelationOrigin::User, RelationOrigin::Dream] {
        let (path, owner, older, source) = fixture(true);
        let mut w = RelWriter::open_path(&path.0, graph(owner), [], []).unwrap();
        let admission = w
            .begin_initial_dream_pass("initial".into(), source, 501)
            .unwrap();
        let change = RelationChange {
            source: NodeRef::Memory(source.id.0),
            target: NodeRef::Memory(older.id.0),
            kind: RelationKind::Topical,
            active: true,
            origin,
        };
        w.session
            .common
            .graph
            .put(
                &mut w.container,
                &GraphAdmission::for_changes(owner, [older.id.0, source.id.0], []),
                &[change],
            )
            .unwrap();
        w.container.sync().unwrap();
        drop(w);
        let mut w = RelWriter::open_path(&path.0, graph(owner), [], [admission]).unwrap();
        assert!(
            w.initial_dream_snapshot("initial")
                .unwrap()
                .roots
                .is_empty()
        );
        assert!(
            w.commit_empty_dream_pass("initial", source, 1, 501)
                .unwrap()
        );
        assert_eq!(
            w.session
                .freshness
                .record_at(FreshnessMemoryId(older.id.0), 502)
                .unwrap()
                .unwrap()
                .score,
            50
        );
        assert!(
            !w.session
                .freshness
                .replay_entries()
                .iter()
                .any(|e| matches!(e, reliquary_freshness_store::FreshnessReplayEntry::Event(_)))
        );
    }
}

#[test]
fn settlement_replay_rejects_begin_with_an_invented_graph_or_memory_prefix() {
    use crate::records::dream::dream_begin_codec::{BeginPopulation, BeginRecord, encode};
    for stale_graph in [true, false] {
        let (path, owner, older, source) = fixture(true);
        let mut c = Container::open_writer(&path.0).unwrap();
        let mut g = reliquary_graph::GraphStore::open(&c, &graph(owner)).unwrap();
        g.put(
            &mut c,
            &GraphAdmission::for_changes(owner, [older.id.0, source.id.0], []),
            &[RelationChange {
                source: NodeRef::Memory(source.id.0),
                target: NodeRef::Memory(older.id.0),
                kind: RelationKind::Topical,
                origin: RelationOrigin::User,
                active: true,
            }],
        )
        .unwrap();
        if !stale_graph {
            let mut m = MemoryStore::open(&c).unwrap();
            let mut draft = super::tests::memory();
            draft.mutation_id = "intervening-memory".into();
            m.publish(&mut c, None, 0, draft).unwrap();
        }
        let admission = DreamBeginAdmission {
            owner_uuid: owner,
            pass_id: "forged-prefix".into(),
            source_id: source.id,
            turn: 4,
            graph_version_before: if stale_graph { 0 } else { 1 },
            preexisting_ids: [older.id].into(),
            memory_cut: 2,
        };
        let begin = BeginRecord {
            owner_uuid: owner,
            pass_id: admission.pass_id.clone(),
            source_id: source.id,
            turn: 4,
            graph_version_before: admission.graph_version_before,
            population: BeginPopulation::ExplicitSet(admission.preexisting_ids.clone()),
        };
        c.append(&encode(&begin).unwrap()).unwrap();
        let freshness = FreshnessStore::open(&c).unwrap();
        let record = InitialDreamSettlement {
            owner,
            pass_id: admission.pass_id.clone(),
            source: source.id,
            revision: 1,
            body: source.body_id,
            graph_version: 1,
            accepted_turn: 4,
            community_generation: None,
            roots: Default::default(),
            credit_roots: Default::default(),
            admit: true,
            freshness_effects: freshness
                .prepare_initial_admission(FreshnessMemoryId(source.id.0), 4, 4)
                .unwrap(),
            memory_delta: None,
        };
        c.append(&record.encode().unwrap()).unwrap();
        c.sync().unwrap();
        drop(c);
        let before = fs::read(&path.0).unwrap();
        assert!(RelSession::open_path(&path.0, &graph(owner), [], [admission]).is_err());
        assert_eq!(before, fs::read(&path.0).unwrap());
    }
}
