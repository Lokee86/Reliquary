use super::*;
use crate::{
    GraphRelationKind, GraphRelationOrigin, MemoryDraft, Phylactery, SemanticGraphRelationChange,
    SemanticGraphRelationKind, SemanticNodeRef,
};
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    std::fs::create_dir_all(&root).unwrap();
    root.join(format!("reliquary-{name}-{}", uuid::Uuid::new_v4()))
}

fn draft(name: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "user".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: name.into(),
        content: name.into(),
        scope: "private".into(),
        lifecycle_state: "extracted".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: name.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn relation(
    source: crate::MemoryId,
    target: crate::MemoryId,
    kind: GraphRelationKind,
) -> SemanticGraphRelationChange {
    SemanticGraphRelationChange {
        source: SemanticNodeRef::memory(source),
        target: SemanticNodeRef::memory(target),
        kind: SemanticGraphRelationKind::Memory(kind),
        active: true,
    }
}

type GraphShape = (
    bool,
    Vec<(
        SemanticNodeRef,
        SemanticNodeRef,
        SemanticGraphRelationKind,
        bool,
    )>,
    Option<i64>,
);

fn graph_shapes(owner: &mut Phylactery) -> Vec<GraphShape> {
    let chunks = owner.container.chunks().unwrap();
    let mut shapes = Vec::new();
    for chunk in chunks {
        let payload = owner.container.read(chunk).unwrap();
        let Some(version) =
            crate::graph_compact_codec::publication::version_at(chunk, &payload).unwrap()
        else {
            continue;
        };
        let mutation = owner.container.read(version.mutation).unwrap();
        let (single, changes) = owner
            .graph
            .decode_stored_transaction(&mutation)
            .unwrap()
            .unwrap();
        shapes.push((
            single,
            changes
                .into_iter()
                .map(|change| (change.source, change.target, change.kind, change.active))
                .collect(),
            owner.container.transaction_time_ns(version.global_version),
        ));
    }
    shapes
}

#[test]
fn native_graph_envelope_migration_preserves_transaction_shape_order_and_times() {
    let source_path = test_path("phy-envelope-migration-source");
    let output_path = test_path("phy-envelope-migration-output");
    let mut source = Phylactery::create_with_uuid(&source_path, [0x41; 16]).unwrap();
    let ids: Vec<_> = (0..4)
        .map(|index| {
            source
                .publish_memory(None, 0, draft(&format!("memory-{index}")))
                .unwrap()
                .0
                .id
        })
        .collect();

    let transactions = [
        (
            vec![relation(ids[0], ids[1], GraphRelationKind::Factual)],
            true,
            Some(101),
        ),
        (
            vec![relation(ids[1], ids[2], GraphRelationKind::Causal)],
            false,
            None,
        ),
        (
            vec![
                relation(ids[2], ids[3], GraphRelationKind::Topical),
                relation(ids[3], ids[0], GraphRelationKind::Factual),
            ],
            false,
            Some(103),
        ),
    ];
    for (ordinal, id) in ids.iter().enumerate() {
        source
            .container
            .append(&crate::graph_codec::encode_node(
                crate::graph_codec::GraphNodePayload {
                    semantic_node: SemanticNodeRef::memory(*id),
                    node_id: arcana::NodeId(ordinal as u32),
                },
            ))
            .unwrap();
    }
    for (index, (changes, single, timestamp)) in transactions.into_iter().enumerate() {
        let rows = changes
            .into_iter()
            .map(
                |change| crate::graph_compact_contract::GraphOrdinalMutation {
                    source: ids
                        .iter()
                        .position(|id| SemanticNodeRef::memory(*id) == change.source)
                        .unwrap() as u32,
                    target: ids
                        .iter()
                        .position(|id| SemanticNodeRef::memory(*id) == change.target)
                        .unwrap() as u32,
                    kind: change.kind,
                    active: change.active,
                    origin: GraphRelationOrigin::User,
                },
            )
            .collect::<Vec<_>>();
        use crate::graph_compact_codec::publication::{self, GraphPublication, GraphTransaction};
        let body = publication::encode(&GraphPublication {
            graph_version: index as u64 + 1,
            transaction: if single {
                GraphTransaction::Single(rows[0])
            } else {
                GraphTransaction::Batch(rows)
            },
        })
        .unwrap();
        source
            .container
            .set_next_transaction_time_override(timestamp);
        source.container.append_publication_envelope(&body).unwrap();
    }
    source.sync().unwrap();
    drop(source);
    let mut source = Phylactery::open(&source_path).unwrap();
    let source_shapes = graph_shapes(&mut source);
    assert_eq!(
        source_shapes,
        vec![
            (
                true,
                vec![(
                    SemanticNodeRef::memory(ids[0]),
                    SemanticNodeRef::memory(ids[1]),
                    SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
                    true,
                )],
                Some(101),
            ),
            (
                false,
                vec![(
                    SemanticNodeRef::memory(ids[1]),
                    SemanticNodeRef::memory(ids[2]),
                    SemanticGraphRelationKind::Memory(GraphRelationKind::Causal),
                    true,
                )],
                None,
            ),
            (
                false,
                vec![
                    (
                        SemanticNodeRef::memory(ids[2]),
                        SemanticNodeRef::memory(ids[3]),
                        SemanticGraphRelationKind::Memory(GraphRelationKind::Topical),
                        true,
                    ),
                    (
                        SemanticNodeRef::memory(ids[3]),
                        SemanticNodeRef::memory(ids[0]),
                        SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
                        true,
                    ),
                ],
                Some(103),
            )
        ]
    );
    drop(source);

    migrate(&source_path, &output_path, [0x42; 16]).unwrap();
    let mut migrated = Phylactery::open(&output_path).unwrap();
    assert_eq!(graph_shapes(&mut migrated), source_shapes);
}

#[test]
fn mixed_origin_graph_transaction_fails_before_creating_migration_output() {
    let source_path = test_path("phy-mixed-origin-source");
    let output_path = test_path("phy-mixed-origin-output");
    let mut source = Phylactery::create_legacy_typed(&source_path).unwrap();
    let first = source
        .publish_memory(None, 0, draft("mixed-a"))
        .unwrap()
        .0
        .id;
    let second = source
        .publish_memory(None, 0, draft("mixed-b"))
        .unwrap()
        .0
        .id;
    source
        .set_memory_relation(
            first,
            second,
            GraphRelationKind::Causal,
            true,
            source.graph_version(),
        )
        .unwrap();
    let mutations = [
        crate::graph_codec::GraphMutationPayload {
            source: SemanticNodeRef::memory(first),
            target: SemanticNodeRef::memory(second),
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
            active: true,
            origin: GraphRelationOrigin::Dream,
        },
        crate::graph_codec::GraphMutationPayload {
            source: SemanticNodeRef::memory(second),
            target: SemanticNodeRef::memory(first),
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
            active: true,
            origin: GraphRelationOrigin::User,
        },
    ];
    let carrier = source
        .graph
        .encode_compact_mutations(&mutations, false)
        .unwrap();
    let mutation_chunk = source.container.append(&carrier).unwrap();
    let global_version = source
        .container
        .allocate_version_with_transaction_time(Some(201))
        .unwrap();
    source
        .container
        .append(&crate::graph_codec::encode_version(
            crate::graph_codec::GraphVersionPayload {
                global_version,
                graph_version: source.graph_version() + 1,
                mutation: mutation_chunk,
            },
        ))
        .unwrap();
    source.sync().unwrap();
    drop(source);

    assert!(matches!(
        migrate(&source_path, &output_path, [0x43; 16]),
        Err(MigrationError::Operation(message)) if message == "graph transaction mixes relation origins"
    ));
    assert!(!output_path.exists());
}
