use super::*;
#[test]
fn synced_batch_replays_all_graph_lifecycle_and_pair_effects_before_any_projection_apply() {
    for rel in [true, false] {
        let (path, owner, cut) = fixture(rel);
        if rel {
            let mut w = RelWriter::open_path(&path.0, graph(owner), [], []).unwrap();
            let record = prepare(&w.session.common, owner, &cut).unwrap().unwrap();
            w.container.append(&record.encode().unwrap()).unwrap();
            w.container.sync().unwrap();
            assert_eq!(w.session.common.graph.graph_version(), 0);
            assert_eq!(w.session.common.memories.memory_version(), 3);
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), []).unwrap();
            let record = prepare(&w.common, owner, &cut).unwrap().unwrap();
            w.container.append(&record.encode().unwrap()).unwrap();
            w.container.sync().unwrap();
            assert_eq!(w.common.graph.graph_version(), 0);
            assert_eq!(w.common.memories.memory_version(), 3);
        }
        let before = fs::read(&path.0).unwrap();
        if rel {
            let mut w = RelWriter::open_path(&path.0, graph(owner), [], []).unwrap();
            assert_eq!(w.session.common.graph.graph_version(), 2);
            assert_eq!(w.session.common.memories.memory_version(), 6);
            for p in &cut.pairs {
                assert!(w.session.common.dream_pairs.contains(p.source, p.candidate));
                assert_eq!(
                    w.session
                        .common
                        .memories
                        .current(p.candidate)
                        .unwrap()
                        .record
                        .superseded_by,
                    Some(cut.observed[0].id)
                );
            }
            assert!(!w.commit_dream_batch(cut.clone()).unwrap());
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), []).unwrap();
            assert_eq!(w.common.graph.graph_version(), 2);
            assert_eq!(w.common.memories.memory_version(), 6);
            for p in &cut.pairs {
                assert!(w.common.dream_pairs.contains(p.source, p.candidate));
                assert_eq!(
                    w.common
                        .memories
                        .current(p.candidate)
                        .unwrap()
                        .record
                        .superseded_by,
                    Some(cut.observed[0].id)
                );
            }
            assert!(!w.commit_dream_batch(cut.clone()).unwrap());
        }
        assert_eq!(before, fs::read(&path.0).unwrap());
    }
}
#[test]
fn incomplete_batch_tail_never_exposes_partial_effects_and_retry_is_exactly_once() {
    use std::io::Write;
    for rel in [true, false] {
        let (path, owner, cut) = fixture(rel);
        let record = if rel {
            let w = RelWriter::open_path(&path.0, graph(owner), [], []).unwrap();
            prepare(&w.session.common, owner, &cut).unwrap().unwrap()
        } else {
            let w = PhyWriter::open_path(&path.0, &graph(owner), []).unwrap();
            prepare(&w.common, owner, &cut).unwrap().unwrap()
        };
        let before = fs::read(&path.0).unwrap();
        let bytes = record.encode().unwrap();
        let mut file = fs::OpenOptions::new().append(true).open(&path.0).unwrap();
        file.write_all(&(bytes.len() as u64).to_le_bytes()).unwrap();
        file.write_all(&bytes[..bytes.len() / 2]).unwrap();
        file.sync_all().unwrap();
        drop(file);
        assert!(Container::open_writer(&path.0).is_err());
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
            let mut w = RelWriter::open_path(&path.0, graph(owner), [], []).unwrap();
            assert!(w.commit_dream_batch(cut.clone()).unwrap());
            assert!(!w.commit_dream_batch(cut).unwrap());
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), []).unwrap();
            assert!(w.commit_dream_batch(cut.clone()).unwrap());
            assert!(!w.commit_dream_batch(cut).unwrap());
        }
    }
}
