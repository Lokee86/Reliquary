use crate::{
    Cva, EpisodeBoundary, EpisodeConfig, EpisodeOrigin, GraphDirection, GraphError,
    GraphRelationChange, GraphRelationKind, GraphRelationOrigin, MemoryDraft, MemoryId,
    SemanticNodeRef,
};
use std::{fs, path::PathBuf};

fn path(name: &str) -> PathBuf {
    let n = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-graph-{n}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn episode(cva: &mut Cva) -> crate::Episode {
    cva.append_node(
        "u".into(),
        "c".into(),
        None,
        "user".into(),
        10,
        "connect memories",
    )
    .unwrap();
    cva.append_node(
        "a".into(),
        "c".into(),
        Some("u".into()),
        "assistant".into(),
        20,
        "ok",
    )
    .unwrap();
    cva.materialize_path_episodes(
        "c",
        "a",
        EpisodeConfig::default(),
        EpisodeOrigin::Live,
        Some((EpisodeBoundary::Inactivity, 30)),
    )
    .unwrap()
    .created
    .into_iter()
    .next()
    .unwrap()
}

fn memory(cva: &mut Cva, episode: &crate::Episode, id: &str) -> MemoryId {
    cva.publish_memory(
        None,
        0,
        MemoryDraft {
            category: "fact".into(),
            memory_type: "project".into(),
            authority_kind: "unknown".into(),
            temporal_status: "unknown".into(),
            title: id.into(),
            content: format!("memory {id}"),
            scope: "private".into(),
            lifecycle_state: "extracted".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: Some("u".into()),
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: Some(reliquary_memory_core::SourceEpisodeId((episode.id).0)),
            source_time_ns: None,
            mutation_id: id.into(),
            created_at_ns: 30,
            updated_at_ns: 30,
        },
    )
    .unwrap()
    .0
    .id
}

#[test]
fn graph_is_oriented_traversable_retractable_and_reopenable() {
    let file = path("graph.cva");
    let mut cva = Cva::create(&file).unwrap();
    let ep = episode(&mut cva);
    let a = memory(&mut cva, &ep, "a");
    let b = memory(&mut cva, &ep, "b");
    let c = memory(&mut cva, &ep, "c");

    cva.set_memory_relation(a, b, GraphRelationKind::Factual, true, 0)
        .unwrap()
        .unwrap();
    cva.set_memory_relation(b, c, GraphRelationKind::Causal, true, 1)
        .unwrap()
        .unwrap();
    assert!(
        cva.set_memory_relation(b, c, GraphRelationKind::Causal, true, 2)
            .unwrap()
            .is_none()
    );
    assert_eq!(cva.graph_version(), 2);
    assert_eq!(
        cva.graph.semantic_nodes(),
        vec![
            SemanticNodeRef::memory(a),
            SemanticNodeRef::memory(b),
            SemanticNodeRef::memory(c)
        ]
    );
    assert!(
        cva.graph_relations()
            .iter()
            .all(|relation| relation.origin == GraphRelationOrigin::Dream)
    );
    assert_eq!(
        cva.graph_neighbors(a, GraphDirection::Outgoing).unwrap()[0].memory_id,
        b
    );
    assert_eq!(
        cva.graph_neighbors(b, GraphDirection::Incoming).unwrap()[0].memory_id,
        a
    );
    let chain = cva.shortest_memory_path(a, c, 2).unwrap().unwrap();
    assert_eq!(chain.memories, vec![a, b, c]);
    assert_eq!(
        chain.relations,
        vec![GraphRelationKind::Factual, GraphRelationKind::Causal]
    );

    cva.set_memory_relation(a, b, GraphRelationKind::Factual, false, 2)
        .unwrap()
        .unwrap();
    assert!(cva.shortest_memory_path(a, c, 2).unwrap().is_none());
    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open(&file).unwrap();
    assert_eq!(reopened.graph_version(), 3);
    assert_eq!(reopened.graph_stats().nodes, 3);
    assert_eq!(reopened.graph_stats().relation_mutations, 3);
    assert_eq!(reopened.graph_stats().active_relations, 1);
    assert!(
        reopened
            .graph_neighbors(a, GraphDirection::Outgoing)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn relation_origin_can_be_user_authored_and_survives_reopen() {
    let file = path("origin.cva");
    let mut cva = Cva::create(&file).unwrap();
    let ep = episode(&mut cva);
    let a = memory(&mut cva, &ep, "a");
    let b = memory(&mut cva, &ep, "b");

    let dream = cva
        .set_memory_relation(a, b, GraphRelationKind::Factual, true, 0)
        .unwrap()
        .unwrap();
    assert_eq!(dream.origin, GraphRelationOrigin::Dream);

    let user = cva
        .set_memory_relation_with_origin(
            a,
            b,
            GraphRelationKind::Factual,
            true,
            GraphRelationOrigin::User,
            1,
        )
        .unwrap()
        .unwrap();
    assert_eq!(user.origin, GraphRelationOrigin::User);
    assert_eq!(cva.graph_version(), 2);
    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open(&file).unwrap();
    let relation = reopened.graph_relations().into_iter().next().unwrap();
    assert_eq!(relation.origin, GraphRelationOrigin::User);
    assert_eq!(reopened.graph_stats().relation_mutations, 2);
}

#[test]
fn graph_enforces_version_and_endpoint_rules() {
    let file = path("rules.cva");
    let mut cva = Cva::create(&file).unwrap();
    let ep = episode(&mut cva);
    let a = memory(&mut cva, &ep, "a");
    let b = memory(&mut cva, &ep, "b");
    cva.set_memory_relation(a, b, GraphRelationKind::Topical, true, 0)
        .unwrap();
    assert!(matches!(
        cva.set_memory_relation(a, b, GraphRelationKind::Topical, false, 0),
        Err(GraphError::RevisionConflict { .. })
    ));
    assert!(matches!(
        cva.set_memory_relation(a, a, GraphRelationKind::Factual, true, 1),
        Err(GraphError::SelfRelation)
    ));
}

#[test]
fn relation_batch_is_one_versioned_transaction_and_reopens_atomically() {
    let file = path("batch.cva");
    let mut cva = Cva::create(&file).unwrap();
    let ep = episode(&mut cva);
    let a = memory(&mut cva, &ep, "a");
    let b = memory(&mut cva, &ep, "b");

    let published = cva
        .set_memory_relations(
            &[
                GraphRelationChange {
                    source: a,
                    target: b,
                    kind: GraphRelationKind::Topical,
                    active: true,
                },
                GraphRelationChange {
                    source: b,
                    target: a,
                    kind: GraphRelationKind::Topical,
                    active: true,
                },
            ],
            0,
        )
        .unwrap();
    assert_eq!(published.len(), 2);
    assert_eq!(published[0].graph_version, 1);
    assert_eq!(published[1].graph_version, 1);
    assert_eq!(published[0].global_version, published[1].global_version);
    assert_eq!(cva.graph_version(), 1);
    assert_eq!(cva.graph_stats().relation_mutations, 2);
    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open(&file).unwrap();
    assert_eq!(reopened.graph_version(), 1);
    assert_eq!(reopened.graph_relations().len(), 2);
    assert_eq!(reopened.graph_stats().relation_mutations, 2);
}

#[test]
fn relation_batch_rejects_duplicate_relationship_identity() {
    let file = path("batch-duplicate.cva");
    let mut cva = Cva::create(&file).unwrap();
    let ep = episode(&mut cva);
    let a = memory(&mut cva, &ep, "a");
    let b = memory(&mut cva, &ep, "b");
    let change = GraphRelationChange {
        source: a,
        target: b,
        kind: GraphRelationKind::Factual,
        active: true,
    };
    assert!(matches!(
        cva.set_memory_relations(&[change, change], 0),
        Err(GraphError::DuplicateRelationChange)
    ));
    assert_eq!(cva.graph_version(), 0);
}

#[test]
fn pre_graph_cva_state_opens_as_empty_graph() {
    let memories = crate::memory_store::MemoryStore::empty();
    let entities = crate::entity_store::EntityStore::empty();
    let graph = crate::graph_replay::GraphReplay::new()
        .finish(&memories, &entities)
        .unwrap();
    assert_eq!(graph.graph_version(), 0);
    assert_eq!(graph.stats().nodes, 0);
}
