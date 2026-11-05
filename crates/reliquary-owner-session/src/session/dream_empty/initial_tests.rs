use super::tests::{Temp, memory};
use super::*;
use reliquary_graph::{RelationChange, RelationKind, RelationOrigin};
use std::{collections::BTreeMap, fs};

pub(super) fn fixture(rel: bool) -> (Temp, [u8; 16], DreamMemoryCut, DreamMemoryCut) {
    fixture_state(rel, false)
}
pub(super) fn fixture_state(
    rel: bool,
    extracted: bool,
) -> (Temp, [u8; 16], DreamMemoryCut, DreamMemoryCut) {
    let path = Temp::new();
    let owner = [211; 16];
    if rel {
        drop(
            RelWriter::create_path(&path.0, reliquary_container::ReliquaryScope::Project, owner)
                .unwrap(),
        );
    } else {
        drop(PhyWriter::create_path(&path.0, owner).unwrap());
    }
    let mut c = Container::open_writer(&path.0).unwrap();
    let mut m = MemoryStore::open(&c).unwrap();
    let older = m
        .publish(&mut c, None, 0, memory())
        .unwrap()
        .revision
        .record;
    let mut draft = memory();
    draft.mutation_id = "later-source".into();
    if extracted {
        draft.lifecycle_state = "extracted".into();
    }
    let source = m.publish(&mut c, None, 0, draft).unwrap().revision.record;
    if rel {
        let mut f = FreshnessStore::open(&c).unwrap();
        f.create_birth(&mut c, FreshnessMemoryId(older.id.0), 1, true)
            .unwrap();
        f.create_birth(&mut c, FreshnessMemoryId(source.id.0), 3, false)
            .unwrap();
    }
    c.sync().unwrap();
    (
        path,
        owner,
        DreamMemoryCut {
            id: older.id,
            revision: 1,
            body_id: older.body_id,
        },
        DreamMemoryCut {
            id: source.id,
            revision: 1,
            body_id: source.body_id,
        },
    )
}
pub(super) fn graph(owner: [u8; 16]) -> GraphAdmission {
    GraphAdmission::for_replay(owner, [], [], [], [], [])
}
fn cut(
    owner: [u8; 16],
    source: DreamMemoryCut,
    candidate: DreamMemoryCut,
) -> crate::DreamRelationCut {
    crate::DreamRelationCut {
        owner_id: owner,
        graph_version: 0,
        source,
        candidate,
        change: RelationChange {
            source: NodeRef::Memory(source.id.0),
            target: NodeRef::Memory(candidate.id.0),
            kind: RelationKind::Topical,
            active: true,
            origin: RelationOrigin::Dream,
        },
    }
}

#[test]
fn initial_relation_sync_recovers_roots_birth_admission_and_exactly_one_event() {
    for rel in [true, false] {
        let (path, owner, older, source) = fixture(rel);
        let admission;
        let mut record = InitialDreamSettlement {
            owner,
            pass_id: "initial".into(),
            source: source.id,
            revision: 1,
            body: source.body_id,
            graph_version: 1,
            accepted_turn: 501,
            community_generation: None,
            memory_delta: None,
            roots: [older.id].into(),
            credit_roots: [older.id].into(),
            admit: true,
            freshness_effects: vec![],
        };
        if rel {
            let mut w = RelWriter::open_path(&path.0, graph(owner), [], []).unwrap();
            admission = w
                .begin_initial_dream_pass("initial".into(), source, 501)
                .unwrap();
            w.commit_dream_grouped_relation(cut(owner, source, older))
                .unwrap();
            record.freshness_effects = w
                .session
                .freshness
                .prepare_initial_dream(
                    FreshnessMemoryId(source.id.0),
                    501,
                    501,
                    1,
                    None,
                    [(FreshnessMemoryId(older.id.0), 50)].into(),
                )
                .unwrap();
            w.container.append(&record.encode().unwrap()).unwrap();
            w.container.sync().unwrap();
            assert!(!w.session.common.dreams.journal().is_settled("initial"));
            assert_eq!(
                w.session
                    .freshness
                    .record_at(FreshnessMemoryId(older.id.0), 502)
                    .unwrap()
                    .unwrap()
                    .score,
                50
            );
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), []).unwrap();
            admission = w
                .begin_initial_dream_pass("initial".into(), source, 501)
                .unwrap();
            w.commit_dream_grouped_relation(cut(owner, source, older))
                .unwrap();
            w.container.append(&record.encode().unwrap()).unwrap();
            w.container.sync().unwrap();
            assert!(!w.common.dreams.journal().is_settled("initial"));
        }
        let before = fs::read(&path.0).unwrap();
        if rel {
            let mut w = RelWriter::open_path(&path.0, graph(owner), [], [admission]).unwrap();
            assert!(w.session.common.dreams.journal().is_settled("initial"));
            assert_eq!(
                w.session
                    .freshness
                    .record_at(FreshnessMemoryId(older.id.0), 502)
                    .unwrap()
                    .unwrap()
                    .score,
                100
            );
            let entries = w.session.freshness.replay_entries();
            assert_eq!(
                entries
                    .iter()
                    .filter(|e| matches!(
                        e,
                        reliquary_freshness_store::FreshnessReplayEntry::Birth { .. }
                    ))
                    .count(),
                2
            );
            assert_eq!(
                entries
                    .iter()
                    .filter(|e| matches!(
                        e,
                        reliquary_freshness_store::FreshnessReplayEntry::Admission { .. }
                    ))
                    .count(),
                1
            );
            assert_eq!(
                entries
                    .iter()
                    .filter(|e| matches!(
                        e,
                        reliquary_freshness_store::FreshnessReplayEntry::Event(_)
                    ))
                    .count(),
                1
            );
            assert!(
                !w.commit_initial_dream_pass("initial", source, 1, 501, BTreeMap::new(), false)
                    .unwrap()
            );
            assert!(
                w.begin_initial_dream_pass("rewire".into(), source, 502)
                    .is_err()
            );
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), [admission]).unwrap();
            assert!(w.common.dreams.journal().is_settled("initial"));
            assert!(
                !w.commit_initial_dream_pass("initial", source, 1, 501, BTreeMap::new(), false)
                    .unwrap()
            );
        }
        assert_eq!(before, fs::read(&path.0).unwrap());
    }
}
