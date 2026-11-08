use super::*;
#[test]
fn multi_pair_graph_memory_and_receipts_commit_in_one_frame_and_reopen_rel_phy() {
    for rel in [true, false] {
        for relation in ["topical", "supersedes", "duplicate_of"] {
            let (path, owner, source, candidates) = fixture(rel);
            begin(&path, owner, rel, source, candidates.clone());
            // Reopen derives admission from the durable frontier, without a retained caller token.
            let proposal = draft(
                &path,
                owner,
                rel,
                source.id,
                &candidates.iter().map(|c| c.id).collect::<Vec<_>>(),
                relation,
                "yes",
            );
            assert_eq!(proposal.classifications().len(), 2);
            let before = Container::open_read_only(&path.0)
                .unwrap()
                .read_frames()
                .unwrap()
                .len();
            assert!(publish(&path, owner, rel, [31; 32], &proposal).unwrap());
            let after = Container::open_read_only(&path.0)
                .unwrap()
                .read_frames()
                .unwrap()
                .len();
            assert_eq!(after, before + 1);
            assert!(settle(&path, owner, rel, source.id).unwrap());
            let before = fs::read(&path.0).unwrap();
            assert!(!publish(&path, owner, rel, [31; 32], &proposal).unwrap());
            assert!(!settle(&path, owner, rel, source.id).unwrap());
            assert_eq!(before, fs::read(&path.0).unwrap());
            let r;
            let p;
            let view = if rel {
                r = RelSession::open_path(&path.0, &graph(owner), [], []).unwrap();
                EntityOwnerSession::Rel(&r)
            } else {
                p = PhySession::open_path(&path.0, &graph(owner), []).unwrap();
                EntityOwnerSession::Phy(&p)
            };
            let history = view.memory_history(source.id).unwrap().unwrap();
            assert_eq!(history.revisions.len(), 2);
            let current = &history.revisions.last().unwrap().record;
            assert_eq!(
                current.lifecycle_state,
                if relation == "duplicate_of" {
                    "archived"
                } else {
                    "canonical"
                }
            );
            for candidate in &candidates {
                assert!(match &view {
                    EntityOwnerSession::Rel(s) => s.dream_pair_evaluated(source.id, candidate.id),
                    EntityOwnerSession::Phy(s) => s.dream_pair_evaluated(source.id, candidate.id),
                    EntityOwnerSession::MountedPhy(w) =>
                        w.dream_pair_evaluated(source.id, candidate.id),
                });
                if relation == "supersedes" {
                    let old = view.memory_history(candidate.id).unwrap().unwrap();
                    assert_eq!(
                        old.revisions.last().unwrap().record.superseded_by,
                        Some(source.id)
                    );
                }
            }
        }
    }
}
