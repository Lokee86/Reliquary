use super::*;
#[test]
fn initial_cycle_no_link_and_no_candidates_settle_without_relation_or_bonus() {
    for rel in [true, false] {
        for classified_none in [true, false] {
            let (path, owner, older, source) = fixture(rel);
            let admission = if rel {
                RelWriter::open_path(&path.0, graph(owner), [], [])
                    .unwrap()
                    .begin_initial_dream_pass("no-link".into(), source, 4)
                    .unwrap()
            } else {
                PhyWriter::open_path(&path.0, &graph(owner), [])
                    .unwrap()
                    .begin_initial_dream_pass("no-link".into(), source, 4)
                    .unwrap()
            };
            if classified_none {
                let none = draft(
                    &path,
                    owner,
                    rel,
                    source.id,
                    &[older.id],
                    &[admission.clone()],
                    true,
                );
                if rel {
                    let mut w =
                        RelWriter::open_path(&path.0, graph(owner), [], [admission.clone()])
                            .unwrap();
                    commit_dream_zero_link(&mut DreamOwnerWriter::Rel(&mut w), owner, &none)
                        .unwrap();
                } else {
                    let mut w =
                        PhyWriter::open_path(&path.0, &graph(owner), [admission.clone()]).unwrap();
                    commit_dream_zero_link(&mut DreamOwnerWriter::Phy(&mut w), owner, &none)
                        .unwrap();
                }
            }
            let empty = draft(
                &path,
                owner,
                rel,
                source.id,
                &[],
                &[admission.clone()],
                false,
            );
            if rel {
                let mut w =
                    RelWriter::open_path(&path.0, graph(owner), [], [admission.clone()]).unwrap();
                assert!(
                    commit_empty_dream_cycle(
                        &mut DreamOwnerWriter::Rel(&mut w),
                        owner,
                        "no-link",
                        &empty,
                        4
                    )
                    .unwrap()
                );
                assert!(
                    w.initial_dream_snapshot("no-link")
                        .unwrap()
                        .roots
                        .is_empty()
                );
            } else {
                let mut w =
                    PhyWriter::open_path(&path.0, &graph(owner), [admission.clone()]).unwrap();
                assert!(
                    commit_empty_dream_cycle(
                        &mut DreamOwnerWriter::Phy(&mut w),
                        owner,
                        "no-link",
                        &empty,
                        4
                    )
                    .unwrap()
                );
                assert!(
                    w.initial_dream_snapshot("no-link")
                        .unwrap()
                        .roots
                        .is_empty()
                );
            }
            let before = fs::read(&path.0).unwrap();
            if rel {
                let mut w =
                    RelWriter::open_path(&path.0, graph(owner), [], [admission.clone()]).unwrap();
                assert!(
                    !commit_empty_dream_cycle(
                        &mut DreamOwnerWriter::Rel(&mut w),
                        owner,
                        "no-link",
                        &empty,
                        4
                    )
                    .unwrap()
                );
            } else {
                let mut w =
                    PhyWriter::open_path(&path.0, &graph(owner), [admission.clone()]).unwrap();
                assert!(
                    !commit_empty_dream_cycle(
                        &mut DreamOwnerWriter::Phy(&mut w),
                        owner,
                        "no-link",
                        &empty,
                        4
                    )
                    .unwrap()
                );
            }
            assert_eq!(before, fs::read(&path.0).unwrap());
        }
    }
}

#[test]
fn initial_cycle_rejects_wrong_authority_and_stale_cuts_before_writing() {
    for rel in [true, false] {
        let (path, owner, older, source) = fixture(rel);
        let admission = if rel {
            RelWriter::open_path(&path.0, graph(owner), [], [])
                .unwrap()
                .begin_initial_dream_pass("negative".into(), source, 4)
                .unwrap()
        } else {
            PhyWriter::open_path(&path.0, &graph(owner), [])
                .unwrap()
                .begin_initial_dream_pass("negative".into(), source, 4)
                .unwrap()
        };
        let empty = draft(
            &path,
            owner,
            rel,
            source.id,
            &[],
            &[admission.clone()],
            false,
        );
        let positive = draft(
            &path,
            owner,
            rel,
            source.id,
            &[older.id],
            &[admission.clone()],
            false,
        );
        let before = fs::read(&path.0).unwrap();
        {
            let mut r;
            let mut p;
            let mut writer = if rel {
                r = RelWriter::open_path(&path.0, graph(owner), [], [admission.clone()]).unwrap();
                DreamOwnerWriter::Rel(&mut r)
            } else {
                p = PhyWriter::open_path(&path.0, &graph(owner), [admission.clone()]).unwrap();
                DreamOwnerWriter::Phy(&mut p)
            };
            assert_eq!(
                settle_initial_dream_cycle(&mut writer, [99; 16], "negative", &empty, 4),
                Err(DreamWorkflowError::OwnerMismatch)
            );
            assert_eq!(
                settle_initial_dream_cycle(&mut writer, owner, "negative", &positive, 4),
                Err(DreamWorkflowError::UnsupportedPublication)
            );
            let mut wrong = empty.clone();
            wrong.graph_version += 1;
            assert_eq!(
                settle_initial_dream_cycle(&mut writer, owner, "negative", &wrong, 4),
                Err(DreamWorkflowError::StaleSnapshot)
            );
            wrong = empty.clone();
            wrong.observed_memories[0].body_id.0 = [99; 32];
            assert_eq!(
                settle_initial_dream_cycle(&mut writer, owner, "negative", &wrong, 4),
                Err(DreamWorkflowError::StaleSnapshot)
            );
            wrong = empty.clone();
            wrong.observed_memories[0].revision = 2;
            assert_eq!(
                settle_initial_dream_cycle(&mut writer, owner, "negative", &wrong, 4),
                Err(DreamWorkflowError::StaleSnapshot)
            );
            assert!(settle_initial_dream_cycle(&mut writer, owner, "negative", &empty, 3).is_err());
        }
        assert_eq!(before, fs::read(&path.0).unwrap());
        if rel {
            let mut w =
                RelWriter::open_path(&path.0, graph(owner), [], [admission.clone()]).unwrap();
            assert!(
                settle_initial_dream_cycle(
                    &mut DreamOwnerWriter::Rel(&mut w),
                    owner,
                    "negative",
                    &empty,
                    4
                )
                .unwrap()
            );
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), [admission.clone()]).unwrap();
            assert!(
                settle_initial_dream_cycle(
                    &mut DreamOwnerWriter::Phy(&mut w),
                    owner,
                    "negative",
                    &empty,
                    4
                )
                .unwrap()
            );
        }
    }
}
