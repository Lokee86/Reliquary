use super::*;
#[test]
fn initial_cycle_relation_survives_begin_interruption_and_settles_once_rel_phy() {
    for rel in [true, false] {
        let (path, owner, older, source) = fixture(rel);
        let admission = if rel {
            RelWriter::open_path(&path.0, graph(owner), [], [])
                .unwrap()
                .begin_initial_dream_pass("initial".into(), source, 4)
                .unwrap()
        } else {
            PhyWriter::open_path(&path.0, &graph(owner), [])
                .unwrap()
                .begin_initial_dream_pass("initial".into(), source, 4)
                .unwrap()
        };
        assert_eq!(admission.preexisting_ids, [older.id].into());
        let before = fs::read(&path.0).unwrap();
        if rel {
            assert_eq!(
                RelWriter::open_path(&path.0, graph(owner), [], [admission.clone()])
                    .unwrap()
                    .begin_initial_dream_pass("initial".into(), source, 4)
                    .unwrap(),
                admission
            );
        } else {
            assert_eq!(
                PhyWriter::open_path(&path.0, &graph(owner), [admission.clone()])
                    .unwrap()
                    .begin_initial_dream_pass("initial".into(), source, 4)
                    .unwrap(),
                admission
            );
        }
        assert_eq!(before, fs::read(&path.0).unwrap());
        let positive = draft(
            &path,
            owner,
            rel,
            source.id,
            &[older.id],
            &[admission.clone()],
            false,
        );
        if rel {
            let mut w =
                RelWriter::open_path(&path.0, graph(owner), [], [admission.clone()]).unwrap();
            commit_dream_relation(&mut DreamOwnerWriter::Rel(&mut w), owner, &positive).unwrap();
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), [admission.clone()]).unwrap();
            commit_dream_relation(&mut DreamOwnerWriter::Phy(&mut w), owner, &positive).unwrap();
        }
        // Resume after the relation was synced, before settlement. No classifier
        // candidate remains, but the original direct receipt must still be credited.
        let empty = draft(
            &path,
            owner,
            rel,
            source.id,
            &[],
            &[admission.clone()],
            false,
        );
        assert!(empty.classifications().is_empty());
        if rel {
            let mut w =
                RelWriter::open_path(&path.0, graph(owner), [], [admission.clone()]).unwrap();
            assert!(
                settle_initial_dream_cycle(
                    &mut DreamOwnerWriter::Rel(&mut w),
                    owner,
                    "initial",
                    &empty,
                    5
                )
                .unwrap()
            );
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), [admission.clone()]).unwrap();
            assert!(
                settle_initial_dream_cycle(
                    &mut DreamOwnerWriter::Phy(&mut w),
                    owner,
                    "initial",
                    &empty,
                    5
                )
                .unwrap()
            );
        }
        let before = fs::read(&path.0).unwrap();
        if rel {
            let mut w =
                RelWriter::open_path(&path.0, graph(owner), [], [admission.clone()]).unwrap();
            assert_eq!(
                w.initial_dream_snapshot("initial").unwrap().roots,
                [older.id].into()
            );
            assert!(
                !settle_initial_dream_cycle(
                    &mut DreamOwnerWriter::Rel(&mut w),
                    owner,
                    "initial",
                    &empty,
                    5
                )
                .unwrap()
            );
            assert!(
                w.begin_initial_dream_pass("rewire".into(), source, 9)
                    .is_err()
            );
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), [admission]).unwrap();
            assert!(w.initial_dream_snapshot("initial").unwrap().settled);
            assert!(
                !settle_initial_dream_cycle(
                    &mut DreamOwnerWriter::Phy(&mut w),
                    owner,
                    "initial",
                    &empty,
                    5
                )
                .unwrap()
            );
            assert!(
                w.begin_initial_dream_pass("rewire".into(), source, 9)
                    .is_err()
            );
        }
        assert_eq!(before, fs::read(&path.0).unwrap());
    }
}
