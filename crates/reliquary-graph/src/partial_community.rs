//! Explicit subset snapshots preserve imported memberships without inventing groups.
use super::*;

impl CommunityStore {
    /// Publish existing Communities that cover only part of the current graph.
    /// Normal derived partitions continue using `publish` and its exact-closure rule.
    pub fn publish_partial_snapshot(
        &mut self,
        container: &mut Container,
        graph: &GraphStore,
        snapshot: CommunitySnapshot,
    ) -> Result<(), GraphError> {
        self.publish_snapshot(container, graph, snapshot, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::{ContainerIdentity, ReliquaryScope};

    #[test]
    fn partial_memberships_reopen_without_weakening_complete_partitions() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("partial-community-{}.rel", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let owner = [41; 16];
        let a = [1; 32];
        let b = [2; 32];
        let other = [3; 32];
        let admission = GraphAdmission::for_changes(owner, [a, b, other], []);
        let mut c = Container::create(
            &path,
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            owner,
        )
        .unwrap();
        let mut graph = GraphStore::open(&c, &admission).unwrap();
        graph
            .put(
                &mut c,
                &admission,
                &[
                    RelationChange {
                        source: NodeRef::Memory(a),
                        target: NodeRef::Memory(b),
                        kind: RelationKind::Topical,
                        active: true,
                        origin: RelationOrigin::Dream,
                    },
                    RelationChange {
                        source: NodeRef::Memory(b),
                        target: NodeRef::Memory(other),
                        kind: RelationKind::Topical,
                        active: true,
                        origin: RelationOrigin::Dream,
                    },
                ],
            )
            .unwrap();
        let cid = community_id(owner, &[a, b]);
        let subset = CommunitySnapshot {
            generation: 1,
            graph_version: 2,
            communities: vec![Community {
                id: cid,
                members: vec![a, b],
            }],
        };
        let mut store = CommunityStore::open(&c, &graph).unwrap();
        assert!(store.publish(&mut c, &graph, subset.clone()).is_err());
        let foreign = [9; 32];
        assert!(
            store
                .publish_partial_snapshot(
                    &mut c,
                    &graph,
                    CommunitySnapshot {
                        generation: 1,
                        graph_version: 2,
                        communities: vec![Community {
                            id: community_id(owner, &[foreign]),
                            members: vec![foreign]
                        }]
                    }
                )
                .is_err()
        );
        store
            .publish_partial_snapshot(&mut c, &graph, subset.clone())
            .unwrap();
        assert!(store.has_partial_coverage());
        let frame = c
            .read_frames()
            .unwrap()
            .into_iter()
            .find(|f| f.payload.starts_with(PARTIAL_COMMUNITY_MAGIC))
            .unwrap();
        assert_eq!(
            community_graph_version_in_frame(&frame.payload).unwrap(),
            Some(2)
        );
        drop((store, graph, c));
        let mut c =
            Container::open_writer_as(&path, reliquary_container::FileKind::Reliquary).unwrap();
        let graph = GraphStore::open(&c, &admission).unwrap();
        let mut store = CommunityStore::open(&c, &graph).unwrap();
        assert_eq!(store.current(), Some(&subset));
        assert!(store.has_partial_coverage());
        store
            .rename(
                &mut c,
                &graph,
                CommunityRename {
                    request_key: [7; 32],
                    cut_digest: [8; 32],
                    id: cid,
                    expected_generation: 1,
                    graph_version: 2,
                    name: "Retained source name".into(),
                },
            )
            .unwrap();
        drop((store, graph, c));
        let c =
            Container::open_read_only_as(&path, reliquary_container::FileKind::Reliquary).unwrap();
        let graph = GraphStore::open(&c, &admission).unwrap();
        let store = CommunityStore::open(&c, &graph).unwrap();
        assert_eq!(store.current().unwrap().communities, subset.communities);
        assert!(store.has_partial_coverage());
        assert_eq!(store.user_name(cid), Some("Retained source name"));
        drop((store, graph, c));
        std::fs::remove_file(path).unwrap();
    }
}
