use super::*;

#[test]
fn rebuilt_graph_local_overflow_installs_no_transaction_or_memory_graph_cut() {
    let mut store = GraphStore::empty();
    let source = SemanticNodeRef::memory([1; 32]);
    let target = SemanticNodeRef::memory([2; 32]);
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

#[test]
fn rewrite_baseline_requires_historical_admission_without_resurrecting_terminal_nodes() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!(
        "baseline-{}-{}.cva",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut container = Container::create(&path).unwrap();
    let mut store = GraphStore::empty();
    let id = [0xab; 32];
    let before = std::fs::read(&path).unwrap();
    assert!(
        matches!(store.publish_rewrite_baseline(&mut container,&GraphAdmission::default(),[1;16],[2;16],[3;32],&[id]),Err(GraphError::MissingMemory(value)) if value==id)
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let facts = GraphAdmission::for_replay([], [id], [], [], []);
    store
        .publish_rewrite_baseline(&mut container, &facts, [1; 16], [2; 16], [3; 32], &[id])
        .unwrap();
    assert_eq!(store.rewrite_baseline_memories(), Some(&[id][..]));
    assert_eq!(store.stats().memory_nodes, 0);
    assert!(store.historical_memory_node_id(id).is_ok());
    assert!(store.node_id(id).is_err());
    drop(container);
    std::fs::remove_file(path).unwrap();
}
