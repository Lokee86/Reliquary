use super::*;
use reliquary_graph::{GraphStore, NodeRef, RelationChange, RelationKind, RelationOrigin};
#[test]
fn user_owned_active_and_inactive_relations_survive_batch_policy_without_dream_reinforcement() {
    for rel in [true, false] {
        for active in [true, false] {
            let (path, owner, source, candidates) = fixture(rel);
            let mut c = Container::open_writer(&path.0).unwrap();
            let mut g = GraphStore::open(&c, &graph(owner)).unwrap();
            g.put(
                &mut c,
                &GraphAdmission::for_changes(owner, [source.id.0, candidates[0].id.0], []),
                &[RelationChange {
                    source: NodeRef::Memory(source.id.0),
                    target: NodeRef::Memory(candidates[0].id.0),
                    kind: RelationKind::Topical,
                    origin: RelationOrigin::User,
                    active,
                }],
            )
            .unwrap();
            c.sync().unwrap();
            drop(c);
            begin(&path, owner, rel, source, candidates.clone());
            let proposal = draft(
                &path,
                owner,
                rel,
                source.id,
                &candidates.iter().map(|c| c.id).collect::<Vec<_>>(),
                "topical",
                "yes",
            );
            assert!(publish(&path, owner, rel, [36; 32], &proposal).unwrap());
            assert!(settle(&path, owner, rel, source.id).unwrap());
            let r;
            let p;
            let view = if rel {
                r = RelSession::open_path(&path.0, &graph(owner), [], []).unwrap();
                EntityOwnerSession::Rel(&r)
            } else {
                p = PhySession::open_path(&path.0, &graph(owner), []).unwrap();
                EntityOwnerSession::Phy(&p)
            };
            let user = view
                .graph_relation(
                    NodeRef::Memory(source.id.0),
                    NodeRef::Memory(candidates[0].id.0),
                    RelationKind::Topical,
                )
                .unwrap();
            assert_eq!(user.origin, RelationOrigin::User);
            assert_eq!(user.active, active);
            drop(view);
            if rel {
                let w = RelWriter::open_path(&path.0, graph(owner), [], []).unwrap();
                assert!(
                    !w.initial_dream_snapshot("initial")
                        .unwrap()
                        .roots
                        .contains(&candidates[0].id)
                );
            } else {
                let w = PhyWriter::open_path(&path.0, &graph(owner), []).unwrap();
                assert!(
                    !w.initial_dream_snapshot("initial")
                        .unwrap()
                        .roots
                        .contains(&candidates[0].id)
                );
            }
        }
    }
}
