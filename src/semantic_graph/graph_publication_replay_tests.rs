use crate::graph_codec::GraphMutationPayload;
use crate::graph_compact_codec::publication::{self, GraphPublication, GraphTransaction};
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::{GraphRelationKind, GraphRelationOrigin, SemanticGraphRelationKind, SemanticNodeRef};
#[test]
fn rel_and_phy_dispatch_admit_single_and_mixed_origin_batch_exactly_once() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("graph-native-open-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    macro_rules! seed {
        ($owner:expr) => {{
            let mut owner = $owner;
            let a = owner
                .publish_memory(
                    None,
                    0,
                    crate::entity_candidate_test_support::memory_draft("a", "a", "a body"),
                )
                .unwrap()
                .0
                .id;
            let b = owner
                .publish_memory(
                    None,
                    0,
                    crate::entity_candidate_test_support::memory_draft("b", "b", "b body"),
                )
                .unwrap()
                .0
                .id;
            owner
                .graph
                .set_relation_with_origin(
                    &mut owner.container,
                    &owner.memories,
                    a,
                    b,
                    GraphRelationKind::Factual,
                    true,
                    GraphRelationOrigin::User,
                    0,
                )
                .unwrap();
            let stable = GraphMutationPayload {
                source: SemanticNodeRef::memory(a).into(),
                target: SemanticNodeRef::memory(b).into(),
                kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
                active: false,
                origin: GraphRelationOrigin::User,
            };
            let compact = owner
                .graph
                .encode_compact_mutations(&[stable], true)
                .unwrap();
            let row = crate::graph_compact_codec::decode(&compact)
                .unwrap()
                .unwrap()[0];
            let body = publication::encode(&GraphPublication {
                graph_version: 2,
                transaction: GraphTransaction::Single(row),
            })
            .unwrap();
            owner.container.append_publication_envelope(&body).unwrap();
            let body = publication::encode(&GraphPublication {
                graph_version: 3,
                transaction: GraphTransaction::Batch(vec![
                    GraphOrdinalMutation {
                        origin: GraphRelationOrigin::Dream,
                        ..row
                    },
                    GraphOrdinalMutation {
                        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Topical),
                        active: true,
                        ..row
                    },
                ]),
            })
            .unwrap();
            owner
                .container
                .set_next_transaction_time_override(Some(-17));
            let (last, _) = owner.container.append_publication_envelope(&body).unwrap();
            owner.sync().unwrap();
            drop(owner);
            last
        }};
    }
    let rel = dir.join("test.rel");
    let phy = dir.join("test.phy");
    let last = seed!(crate::Cva::create_project(&rel).unwrap());
    let owner = crate::Cva::open_project(&rel).unwrap();
    assert_eq!(owner.graph.graph_version(), 3);
    assert_eq!(owner.graph.memory_graph_version(), 3);
    assert_eq!(owner.graph.stats().relation_mutations, 4);
    assert_eq!(owner.graph.stats().active_relations, 1);
    assert_eq!(owner.transaction_time_ns(last), Some(-17));
    drop(owner);
    let last = seed!(crate::Phylactery::create(&phy).unwrap());
    let owner = crate::Phylactery::open(&phy).unwrap();
    assert_eq!(owner.graph.graph_version(), 3);
    assert_eq!(owner.graph.memory_graph_version(), 3);
    assert_eq!(owner.graph.stats().relation_mutations, 4);
    assert_eq!(owner.graph.stats().active_relations, 1);
    assert_eq!(owner.transaction_time_ns(last), Some(-17));
    drop(owner);
    std::fs::remove_dir_all(dir).unwrap();
}
