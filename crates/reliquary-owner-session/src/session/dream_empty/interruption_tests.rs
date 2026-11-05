use super::initial_tests::{fixture_state, graph};
use super::*;
use std::{collections::BTreeMap, fs, io::Write};

#[test]
fn incomplete_initial_frame_requires_explicit_repair_and_retry_applies_all_effects_once() {
    for rel in [true, false] {
        let (path, owner, _, source) = fixture_state(rel, true);
        let admission;
        let record;
        if rel {
            let mut w = RelWriter::open_path(&path.0, graph(owner), [], []).unwrap();
            admission = w
                .begin_initial_dream_pass("interrupted".into(), source, 4)
                .unwrap();
            record = InitialDreamSettlement {
                owner,
                pass_id: "interrupted".into(),
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
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), []).unwrap();
            admission = w
                .begin_initial_dream_pass("interrupted".into(), source, 4)
                .unwrap();
            record = InitialDreamSettlement {
                owner,
                pass_id: "interrupted".into(),
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
        }
        let before = fs::read(&path.0).unwrap();
        let payload = record.encode().unwrap();
        let mut file = fs::OpenOptions::new().append(true).open(&path.0).unwrap();
        file.write_all(&(payload.len() as u64).to_le_bytes())
            .unwrap();
        file.write_all(&payload[..payload.len() / 2]).unwrap();
        file.sync_all().unwrap();
        drop(file);
        assert!(Container::open_writer(&path.0).is_err());
        if rel {
            assert!(
                RelSession::open_path(&path.0, &graph(owner), [], [admission.clone()]).is_err()
            );
        } else {
            assert!(PhySession::open_path(&path.0, &graph(owner), [admission.clone()]).is_err());
        }
        let mut repair = Container::open_for_repair(
            &path.0,
            if rel {
                FileKind::Reliquary
            } else {
                FileKind::Phylactery
            },
        )
        .unwrap();
        let plan = repair.plan_tail_repair().unwrap().unwrap();
        repair.repair_incomplete_tail(plan).unwrap();
        drop(repair);
        assert_eq!(before, fs::read(&path.0).unwrap());
        if rel {
            let mut w = RelWriter::open_path(&path.0, graph(owner), [], [admission]).unwrap();
            assert!(!w.session.common.dreams.journal().is_settled("interrupted"));
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
            assert!(
                w.commit_initial_dream_pass("interrupted", source, 0, 4, BTreeMap::new(), true)
                    .unwrap()
            );
            assert!(
                !w.commit_initial_dream_pass("interrupted", source, 0, 4, BTreeMap::new(), true)
                    .unwrap()
            );
            assert_eq!(
                w.session
                    .common
                    .memories
                    .history(source.id)
                    .unwrap()
                    .revisions
                    .len(),
                2
            );
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), [admission]).unwrap();
            assert!(!w.common.dreams.journal().is_settled("interrupted"));
            assert_eq!(
                w.common
                    .memories
                    .history(source.id)
                    .unwrap()
                    .revisions
                    .len(),
                1
            );
            assert!(
                w.commit_initial_dream_pass("interrupted", source, 0, 4, BTreeMap::new(), true)
                    .unwrap()
            );
            assert!(
                !w.commit_initial_dream_pass("interrupted", source, 0, 4, BTreeMap::new(), true)
                    .unwrap()
            );
            assert_eq!(
                w.common
                    .memories
                    .history(source.id)
                    .unwrap()
                    .revisions
                    .len(),
                2
            );
        }
    }
}
