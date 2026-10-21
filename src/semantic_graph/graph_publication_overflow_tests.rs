use super::*;

#[test]
fn rebuilt_graph_local_overflow_installs_no_transaction_or_memory_graph_cut() {
    let mut store = GraphStore::empty();
    let source = SemanticNodeRef::memory(MemoryId([1; 32]));
    let target = SemanticNodeRef::memory(MemoryId([2; 32]));
    store
        .insert_node_rebuilt(GraphNodeRecord {
            semantic_node: source,
            node_id: arcana::NodeId(0),
        })
        .unwrap();
    store
        .insert_node_rebuilt(GraphNodeRecord {
            semantic_node: target,
            node_id: arcana::NodeId(1),
        })
        .unwrap();
    store.next_graph_version = u64::MAX;
    let transaction = [SemanticGraphRelation {
        source,
        target,
        kind: SemanticGraphRelationKind::Memory(crate::GraphRelationKind::Factual),
        active: true,
        origin: crate::GraphRelationOrigin::User,
        global_version: 7,
        graph_version: u64::MAX,
    }];
    assert!(matches!(
        store.insert_transaction_rebuilt(&transaction),
        Err(GraphError::GraphVersionExhausted)
    ));
    assert_eq!(store.next_graph_version, u64::MAX);
    assert_eq!(store.memory_graph_version, 0);
    assert_eq!(store.relation_mutations, 0);
    assert!(store.states.is_empty());
    assert!(store.transaction_global_versions.is_empty());
}
