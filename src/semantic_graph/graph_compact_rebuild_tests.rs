use crate::graph_codec::GraphMutationPayload;
use crate::graph_codec::{
    GraphNodePayload, GraphVersionPayload, encode_format, encode_node, encode_version,
};
use crate::graph_compact_codec::{
    encode_batch as encode_compact_batch, encode_mutation as encode_compact_mutation,
};
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::{
    Cva, GraphRelationKind, GraphRelationOrigin, MemoryDraft, MemoryId, Phylactery,
    SemanticGraphRelationKind, SemanticNodeRef,
};
use arcana::NodeId;
use std::{fs, path::PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let root = std::env::current_dir().unwrap().join(".test-tmp");
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!("graph-compact-rebuild-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Snapshot {
    graph_version: u64,
    memory_graph_version: u64,
    stats: crate::GraphStats,
    relations: Vec<crate::SemanticGraphRelation>,
    clocks: Vec<Option<i64>>,
    graph_cuts: Vec<CutSnapshot>,
    memory_graph_cuts: Vec<CutSnapshot>,
}

type CutSnapshot = (
    std::collections::BTreeMap<
        (SemanticNodeRef, SemanticNodeRef, SemanticGraphRelationKind),
        crate::SemanticGraphRelation,
    >,
    u64,
);

fn history_cut(
    history: &crate::historical_graph_fingerprint::GraphHistory,
    cut: u64,
    memory: bool,
) -> CutSnapshot {
    let cut = history.at(cut, memory).unwrap();
    (cut.states, cut.memory_graph_version)
}

fn draft(name: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "unknown".into(),
        temporal_status: "unknown".into(),
        title: name.into(),
        content: format!("body {name}"),
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
        mutation_id: format!("create-{name}"),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn create_memories_rel(owner: &mut Cva) -> (MemoryId, MemoryId) {
    let a = owner.publish_memory(None, 0, draft("a")).unwrap().0.id;
    let b = owner.publish_memory(None, 0, draft("b")).unwrap().0.id;
    (a, b)
}

fn create_memories_phy(owner: &mut Phylactery) -> (MemoryId, MemoryId) {
    let a = owner.publish_memory(None, 0, draft("a")).unwrap().0.id;
    let b = owner.publish_memory(None, 0, draft("b")).unwrap().0.id;
    (a, b)
}

fn publish_ticket(container: &mut crate::Container, graph_version: u64) -> u64 {
    container
        .allocate_version_with_transaction_time(Some(50_000 + graph_version as i64))
        .unwrap()
}

fn append_mapping(container: &mut crate::Container, id: MemoryId, ordinal: u32) {
    container
        .append(&encode_node(GraphNodePayload {
            semantic_node: SemanticNodeRef::memory(id),
            node_id: NodeId(ordinal),
        }))
        .unwrap();
}

fn append_marker(container: &mut crate::Container, mutation: crate::ObjectRef, graph_version: u64) {
    let global_version = publish_ticket(container, graph_version);
    container
        .append(&encode_version(GraphVersionPayload {
            global_version,
            graph_version,
            mutation,
        }))
        .unwrap();
}

fn legacy(mutation: GraphOrdinalMutation, a: MemoryId, b: MemoryId) -> Vec<u8> {
    let source = if mutation.source == 0 { a } else { b };
    let target = if mutation.target == 0 { a } else { b };
    crate::graph_codec::encode_mutation(GraphMutationPayload {
        source: SemanticNodeRef::memory(source),
        target: SemanticNodeRef::memory(target),
        kind: mutation.kind,
        active: mutation.active,
        origin: mutation.origin,
    })
}

fn snapshot_rel(owner: &Cva) -> Snapshot {
    let history =
        crate::historical_graph_fingerprint::GraphHistory::read(&owner.container, &owner.graph)
            .unwrap();
    Snapshot {
        graph_version: owner.graph_version(),
        memory_graph_version: owner.memory_graph_version(),
        stats: owner.graph_stats(),
        relations: owner.semantic_graph_relations(),
        clocks: (owner.container.latest_version().saturating_sub(2)
            ..=owner.container.latest_version())
            .map(|v| owner.container.transaction_time_ns(v))
            .collect(),
        graph_cuts: (0..=3)
            .map(|cut| history_cut(&history, cut, false))
            .collect(),
        memory_graph_cuts: (0..=3)
            .map(|cut| history_cut(&history, cut, true))
            .collect(),
    }
}

fn snapshot_phy(owner: &Phylactery) -> Snapshot {
    let history =
        crate::historical_graph_fingerprint::GraphHistory::read(&owner.container, &owner.graph)
            .unwrap();
    Snapshot {
        graph_version: owner.graph_version(),
        memory_graph_version: owner.memory_graph_version(),
        stats: owner.graph_stats(),
        relations: owner.semantic_graph_relations(),
        clocks: (owner.container.latest_version().saturating_sub(2)
            ..=owner.container.latest_version())
            .map(|v| owner.container.transaction_time_ns(v))
            .collect(),
        graph_cuts: (0..=3)
            .map(|cut| history_cut(&history, cut, false))
            .collect(),
        memory_graph_cuts: (0..=3)
            .map(|cut| history_cut(&history, cut, true))
            .collect(),
    }
}

fn build_rel(path: &std::path::Path, compact: bool) -> Snapshot {
    let mut owner = Cva::create(path).unwrap();
    let (a, b) = create_memories_rel(&mut owner);
    let changes = [
        GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
            active: true,
            origin: GraphRelationOrigin::User,
        },
        GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
            active: true,
            origin: GraphRelationOrigin::User,
        },
        GraphOrdinalMutation {
            source: 1,
            target: 0,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Topical),
            active: true,
            origin: GraphRelationOrigin::Dream,
        },
        GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Causal),
            active: true,
            origin: GraphRelationOrigin::User,
        },
    ];
    let first = if compact {
        encode_compact_mutation(changes[0])
    } else {
        legacy(changes[0], a, b)
    };
    let carrier = owner.container.append(&first).unwrap();
    // A compact carrier must stay unresolved until both dense ordinal mappings arrive.
    append_mapping(&mut owner.container, a, 0);
    append_mapping(&mut owner.container, b, 1);
    append_marker(&mut owner.container, carrier, 1);
    let second = if compact {
        encode_compact_mutation(changes[1])
    } else {
        legacy(changes[1], a, b)
    };
    let carrier = owner.container.append(&second).unwrap();
    append_marker(&mut owner.container, carrier, 2);
    let batch = if compact {
        encode_compact_batch(&changes[2..]).unwrap()
    } else {
        crate::graph_codec::encode_batch(
            &changes[2..]
                .iter()
                .map(|m| GraphMutationPayload {
                    source: SemanticNodeRef::memory(if m.source == 0 { a } else { b }),
                    target: SemanticNodeRef::memory(if m.target == 0 { a } else { b }),
                    kind: m.kind,
                    active: m.active,
                    origin: m.origin,
                })
                .collect::<Vec<_>>(),
        )
    };
    let carrier = owner.container.append(&batch).unwrap();
    append_marker(&mut owner.container, carrier, 3);
    drop(owner);
    let reopened = Cva::open(path).unwrap();
    snapshot_rel(&reopened)
}

fn build_phy(path: &std::path::Path, compact: bool) -> Snapshot {
    let mut owner = Phylactery::create(path).unwrap();
    let (a, b) = create_memories_phy(&mut owner);
    let changes = [
        GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
            active: true,
            origin: GraphRelationOrigin::User,
        },
        GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
            active: true,
            origin: GraphRelationOrigin::User,
        },
        GraphOrdinalMutation {
            source: 1,
            target: 0,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Topical),
            active: true,
            origin: GraphRelationOrigin::Dream,
        },
        GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Causal),
            active: true,
            origin: GraphRelationOrigin::User,
        },
    ];
    let first = if compact {
        encode_compact_mutation(changes[0])
    } else {
        legacy(changes[0], a, b)
    };
    let carrier = owner.container.append(&first).unwrap();
    append_mapping(&mut owner.container, a, 0);
    append_mapping(&mut owner.container, b, 1);
    append_marker(&mut owner.container, carrier, 1);
    let second = if compact {
        encode_compact_mutation(changes[1])
    } else {
        legacy(changes[1], a, b)
    };
    let carrier = owner.container.append(&second).unwrap();
    append_marker(&mut owner.container, carrier, 2);
    let batch = if compact {
        encode_compact_batch(&changes[2..]).unwrap()
    } else {
        crate::graph_codec::encode_batch(
            &changes[2..]
                .iter()
                .map(|m| GraphMutationPayload {
                    source: SemanticNodeRef::memory(if m.source == 0 { a } else { b }),
                    target: SemanticNodeRef::memory(if m.target == 0 { a } else { b }),
                    kind: m.kind,
                    active: m.active,
                    origin: m.origin,
                })
                .collect::<Vec<_>>(),
        )
    };
    let carrier = owner.container.append(&batch).unwrap();
    append_marker(&mut owner.container, carrier, 3);
    drop(owner);
    let reopened = Phylactery::open(path).unwrap();
    snapshot_phy(&reopened)
}

#[test]
fn compact_and_legacy_graph_replay_match_for_rel_and_phy() {
    let scratch = Scratch::new();
    let rel_legacy = build_rel(&scratch.0.join("rel-legacy.cva"), false);
    let rel_compact = build_rel(&scratch.0.join("rel-compact.cva"), true);
    assert_eq!(rel_compact, rel_legacy);
    assert_eq!(rel_compact.graph_version, 3);
    assert_eq!(rel_compact.memory_graph_version, 3);
    assert_eq!(rel_compact.stats.relation_mutations, 4);
    assert_eq!(rel_compact.relations.len(), 3);
    assert!(
        rel_compact
            .relations
            .iter()
            .any(|r| r.origin == GraphRelationOrigin::User)
    );
    assert!(
        rel_compact
            .relations
            .iter()
            .any(|r| r.origin == GraphRelationOrigin::Dream)
    );

    let phy_legacy = build_phy(&scratch.0.join("phy-legacy.cva"), false);
    let phy_compact = build_phy(&scratch.0.join("phy-compact.cva"), true);
    assert_eq!(phy_compact, phy_legacy);
    assert_eq!(phy_compact.graph_version, 3);
    assert_eq!(phy_compact.memory_graph_version, 3);
    assert_eq!(phy_compact.stats.relation_mutations, 4);
    assert_eq!(phy_compact.relations.len(), 3);
}

fn synthetic_ref(offset: u64, len: u64) -> crate::ObjectRef {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&offset.to_le_bytes());
    bytes[8..].copy_from_slice(&len.to_le_bytes());
    crate::ObjectRef::from_legacy_bytes(bytes)
}

fn graph_open_state() -> crate::graph_rebuild::GraphOpenState {
    let mut state = crate::graph_rebuild::GraphOpenState::new();
    state
        .ingest(synthetic_ref(0, 12), &encode_format(), 1)
        .unwrap();
    state
}

fn compact_payload() -> Vec<u8> {
    encode_compact_mutation(GraphOrdinalMutation {
        source: 0,
        target: 1,
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
        active: true,
        origin: GraphRelationOrigin::User,
    })
}

fn graph_mapping(offset: u64, ordinal: u32, byte: u8) -> (crate::ObjectRef, Vec<u8>) {
    (
        synthetic_ref(offset, 45),
        encode_node(GraphNodePayload {
            semantic_node: SemanticNodeRef::memory(MemoryId([byte; 32])),
            node_id: NodeId(ordinal),
        }),
    )
}

fn compact_marker(carrier: crate::ObjectRef) -> (crate::ObjectRef, [u8; 40]) {
    (
        synthetic_ref(50, 40),
        encode_version(GraphVersionPayload {
            global_version: 1,
            graph_version: 1,
            mutation: carrier,
        }),
    )
}

#[test]
fn compact_publication_requires_prior_dense_mapping_and_marker() {
    let carrier = synthetic_ref(20, 20);
    let mut missing_mapping = graph_open_state();
    missing_mapping
        .ingest(carrier, &compact_payload(), 1)
        .unwrap();
    let (marker_ref, marker) = compact_marker(carrier);
    assert!(missing_mapping.ingest(marker_ref, &marker, 1).is_err());

    let mut mapping_after_marker = graph_open_state();
    mapping_after_marker
        .ingest(carrier, &compact_payload(), 1)
        .unwrap();
    assert!(mapping_after_marker.ingest(marker_ref, &marker, 1).is_err());

    let mut unmarked = graph_open_state();
    let invalid_ordinal = encode_compact_mutation(GraphOrdinalMutation {
        source: 99,
        target: 100,
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
        active: true,
        origin: GraphRelationOrigin::User,
    });
    unmarked.ingest(carrier, &invalid_ordinal, 1).unwrap();
    let rebuilt = unmarked
        .finish(
            &crate::memory_store::MemoryStore::empty(),
            &crate::entity_store::EntityStore::empty(),
        )
        .unwrap();
    assert_eq!(rebuilt.graph_version(), 0);
}

#[test]
fn compact_unmarked_v1_is_inert_but_published_v1_is_rejected() {
    use crate::graph_rebuild::GraphOpenState;

    let v1 = crate::FormatVersion { major: 1, minor: 0 };
    let a = SemanticNodeRef::memory(MemoryId([1; 32]));
    let b = SemanticNodeRef::memory(MemoryId([2; 32]));
    let ordinal = GraphOrdinalMutation {
        source: 0,
        target: 1,
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
        active: true,
        origin: GraphRelationOrigin::User,
    };
    let carrier = crate::ObjectRef::from_legacy_bytes({
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&20_u64.to_le_bytes());
        bytes[8..].copy_from_slice(&20_u64.to_le_bytes());
        bytes
    });

    let mut inert = GraphOpenState::new();
    inert
        .ingest(
            crate::ObjectRef::from_legacy_bytes([0; 16]),
            &encode_format(),
            0,
        )
        .unwrap();
    inert
        .ingest(carrier, &encode_compact_mutation(ordinal), 0)
        .unwrap();
    inert.validate_storage_version(v1).unwrap();

    let mut published = GraphOpenState::new();
    published
        .ingest(
            crate::ObjectRef::from_legacy_bytes([0; 16]),
            &encode_format(),
            1,
        )
        .unwrap();
    published
        .ingest(carrier, &encode_compact_mutation(ordinal), 1)
        .unwrap();
    for (offset, node_id, node) in [(30_u64, 0_u32, a), (40, 1, b)] {
        let mut raw = [0; 16];
        raw[..8].copy_from_slice(&offset.to_le_bytes());
        raw[8..].copy_from_slice(&45_u64.to_le_bytes());
        published
            .ingest(
                crate::ObjectRef::from_legacy_bytes(raw),
                &encode_node(GraphNodePayload {
                    semantic_node: node,
                    node_id: NodeId(node_id),
                }),
                1,
            )
            .unwrap();
    }
    let marker = crate::ObjectRef::from_legacy_bytes({
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&50_u64.to_le_bytes());
        bytes[8..].copy_from_slice(&40_u64.to_le_bytes());
        bytes
    });
    let payload = encode_version(GraphVersionPayload {
        global_version: 1,
        graph_version: 1,
        mutation: carrier,
    });
    published.ingest(marker, &payload, 1).unwrap();
    assert!(published.validate_storage_version(v1).is_err());
    published
        .validate_storage_version(crate::FormatVersion { major: 2, minor: 0 })
        .unwrap();
}

#[test]
fn compact_ordinal_zero_and_dangling_or_repeated_markers_have_exact_admission() {
    let scratch = Scratch::new();
    let path = scratch.0.join("ordinal-zero.cva");
    let mut owner = Cva::create(&path).unwrap();
    let (a, b) = create_memories_rel(&mut owner);
    append_mapping(&mut owner.container, a, 0);
    append_mapping(&mut owner.container, b, 1);
    let carrier = owner
        .container
        .append(&encode_compact_mutation(GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
            active: true,
            origin: GraphRelationOrigin::User,
        }))
        .unwrap();
    append_marker(&mut owner.container, carrier, 1);
    drop(owner);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.graph_version(), 1);
    assert_eq!(
        reopened.semantic_graph_relations()[0].source,
        SemanticNodeRef::memory(a)
    );

    let path = scratch.0.join("dangling-marker.cva");
    let mut owner = Cva::create(&path).unwrap();
    let fake = crate::ObjectRef::from_legacy_bytes([0; 16]);
    append_marker(&mut owner.container, fake, 1);
    drop(owner);
    assert!(Cva::open(&path).is_err());

    let path = scratch.0.join("repeated-marker.cva");
    let mut owner = Cva::create(&path).unwrap();
    let (a, b) = create_memories_rel(&mut owner);
    append_mapping(&mut owner.container, a, 0);
    append_mapping(&mut owner.container, b, 1);
    let carrier = owner
        .container
        .append(&encode_compact_mutation(GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
            active: true,
            origin: GraphRelationOrigin::User,
        }))
        .unwrap();
    append_marker(&mut owner.container, carrier, 1);
    append_marker(&mut owner.container, carrier, 2);
    drop(owner);
    assert!(Cva::open(&path).is_err());
}

#[test]
fn compact_mapping_ids_must_be_unique_and_dense_in_publication_order() {
    let scratch = Scratch::new();
    for (name, second_id, second_ordinal) in [
        ("duplicate-semantic.cva", 0_u8, 1_u32),
        ("out-of-order-ordinal.cva", 1_u8, 0_u32),
    ] {
        let path = scratch.0.join(name);
        let mut owner = Cva::create(&path).unwrap();
        let (a, b) = create_memories_rel(&mut owner);
        append_mapping(&mut owner.container, a, 0);
        append_mapping(
            &mut owner.container,
            if second_id == 0 { a } else { b },
            second_ordinal,
        );
        let carrier = owner
            .container
            .append(&encode_compact_mutation(GraphOrdinalMutation {
                source: 0,
                target: 1,
                kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
                active: true,
                origin: GraphRelationOrigin::User,
            }))
            .unwrap();
        append_marker(&mut owner.container, carrier, 1);
        drop(owner);
        assert!(Cva::open(&path).is_err(), "{name}");
    }
}
