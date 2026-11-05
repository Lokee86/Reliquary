use super::*;
#[test]
fn batch_refuses_cross_owner_stale_body_revision_graph_and_conflicting_retry_without_writes() {
    for rel in [true, false] {
        for variant in 0..5 {
            let (path, owner, cut) = fixture(rel);
            let mut bad = cut.clone();
            match variant {
                0 => bad.owner = [99; 16],
                1 => bad.graph_version = 1,
                2 => bad.observed[0].body_id.0 = [99; 32],
                3 => bad.observed[1].revision += 1,
                _ => bad.lifecycles[0].memory_id = MemoryId([99; 32]),
            };
            let before = fs::read(&path.0).unwrap();
            if rel {
                assert!(
                    RelWriter::open_path(&path.0, graph(owner), [], [])
                        .unwrap()
                        .commit_dream_batch(bad)
                        .is_err()
                );
            } else {
                assert!(
                    PhyWriter::open_path(&path.0, &graph(owner), [])
                        .unwrap()
                        .commit_dream_batch(bad)
                        .is_err()
                );
            }
            assert_eq!(before, fs::read(&path.0).unwrap());
            if rel {
                assert!(
                    RelWriter::open_path(&path.0, graph(owner), [], [])
                        .unwrap()
                        .commit_dream_batch(cut.clone())
                        .unwrap()
                );
            } else {
                assert!(
                    PhyWriter::open_path(&path.0, &graph(owner), [])
                        .unwrap()
                        .commit_dream_batch(cut.clone())
                        .unwrap()
                );
            }
            let before = fs::read(&path.0).unwrap();
            let mut conflict = cut;
            conflict.updated_at_ns += 1;
            if rel {
                assert!(
                    RelWriter::open_path(&path.0, graph(owner), [], [])
                        .unwrap()
                        .commit_dream_batch(conflict)
                        .is_err()
                );
            } else {
                assert!(
                    PhyWriter::open_path(&path.0, &graph(owner), [])
                        .unwrap()
                        .commit_dream_batch(conflict)
                        .is_err()
                );
            }
            assert_eq!(before, fs::read(&path.0).unwrap());
        }
    }
}
#[test]
fn batch_replay_rejects_fabricated_direct_receipts_and_unrelated_lifecycle_deltas() {
    for variant in 0..2 {
        let (path, owner, cut) = fixture(false);
        let mut w = PhyWriter::open_path(&path.0, &graph(owner), []).unwrap();
        let mut record = prepare(&w.common, owner, &cut).unwrap().unwrap();
        if variant == 0 {
            record.pairs[0].direct = false;
        } else {
            let n = record.lifecycles[0].delta.len();
            record.lifecycles[0].delta[n - 1] ^= 1;
        }
        w.container.append(&record.encode().unwrap()).unwrap();
        w.container.sync().unwrap();
        drop(w);
        let before = fs::read(&path.0).unwrap();
        assert!(PhySession::open_path(&path.0, &graph(owner), []).is_err());
        assert_eq!(before, fs::read(&path.0).unwrap());
    }
}
