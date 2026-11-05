use super::initial_tests::{fixture_state, graph};
use super::*;
use std::{collections::BTreeMap, fs};

#[test]
fn later_no_candidate_cycle_uses_population_after_an_earlier_atomic_promotion() {
    for rel in [true, false] {
        let (path, owner, older, source) = fixture_state(rel, true);
        let first;
        let later;
        let mut draft = super::tests::memory();
        draft.mutation_id = "next-first-cycle".into();
        draft.lifecycle_state = "extracted".into();
        if rel {
            let mut w = RelWriter::open_path(&path.0, graph(owner), [], []).unwrap();
            first = w
                .begin_initial_dream_pass("first".into(), source, 4)
                .unwrap();
            w.commit_initial_dream_pass("first", source, 0, 4, BTreeMap::new(), true)
                .unwrap();
            later = w
                .session
                .common
                .memories
                .publish(&mut w.container, None, 0, draft)
                .unwrap()
                .revision
                .record;
            w.container.sync().unwrap();
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), []).unwrap();
            first = w
                .begin_initial_dream_pass("first".into(), source, 4)
                .unwrap();
            w.commit_initial_dream_pass("first", source, 0, 4, BTreeMap::new(), true)
                .unwrap();
            later = w
                .common
                .memories
                .publish(&mut w.container, None, 0, draft)
                .unwrap()
                .revision
                .record;
            w.container.sync().unwrap();
        }
        let cut = DreamMemoryCut {
            id: later.id,
            revision: 1,
            body_id: later.body_id,
        };
        let second;
        if rel {
            let mut w = RelWriter::open_path(&path.0, graph(owner), [], [first.clone()]).unwrap();
            second = w.begin_initial_dream_pass("second".into(), cut, 5).unwrap();
            assert_eq!(second.preexisting_ids, [older.id, source.id].into());
            w.commit_initial_dream_pass("second", cut, 0, 5, BTreeMap::new(), true)
                .unwrap();
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), [first.clone()]).unwrap();
            second = w.begin_initial_dream_pass("second".into(), cut, 5).unwrap();
            assert_eq!(second.preexisting_ids, [older.id, source.id].into());
            w.commit_initial_dream_pass("second", cut, 0, 5, BTreeMap::new(), true)
                .unwrap();
        }
        let before = fs::read(&path.0).unwrap();
        if rel {
            let mut w = RelWriter::open_path(&path.0, graph(owner), [], [first, second]).unwrap();
            assert!(w.session.common.dreams.journal().is_settled("second"));
            assert_eq!(
                w.session
                    .common
                    .memories
                    .history(later.id)
                    .unwrap()
                    .revisions
                    .len(),
                2
            );
            assert!(
                !w.commit_initial_dream_pass("second", cut, 0, 5, BTreeMap::new(), true)
                    .unwrap()
            );
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), [first, second]).unwrap();
            assert!(w.common.dreams.journal().is_settled("second"));
            assert_eq!(
                w.common.memories.history(later.id).unwrap().revisions.len(),
                2
            );
            assert!(
                !w.commit_initial_dream_pass("second", cut, 0, 5, BTreeMap::new(), true)
                    .unwrap()
            );
        }
        assert_eq!(before, fs::read(&path.0).unwrap());
    }
}
