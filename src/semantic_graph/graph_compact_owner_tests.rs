fn encode_mutation(value: GraphOrdinalMutation) -> Vec<u8> {
    crate::graph_compact_codec::publication::encode(
        &crate::graph_compact_codec::publication::GraphPublication {
            graph_version: 1,
            transaction: crate::graph_compact_codec::publication::GraphTransaction::Single(value),
        },
    )
    .unwrap()
}
fn encode_batch(values: &[GraphOrdinalMutation]) -> Result<Vec<u8>, crate::GraphError> {
    crate::graph_compact_codec::publication::encode(
        &crate::graph_compact_codec::publication::GraphPublication {
            graph_version: 1,
            transaction: crate::graph_compact_codec::publication::GraphTransaction::Batch(
                values.to_vec(),
            ),
        },
    )
}
use crate::graph_codec::{GraphVersionPayload, encode_version};

use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::graph_store::GraphStore;
use crate::historical_graph_fingerprint::GraphHistory;
use crate::{
    Cva, EntityDraft, EntityId, GraphRelationKind, MemoryDraft, MemoryId, Phylactery,
    SemanticGraphRelationKind, SemanticNodeRef,
};
use arcana::NodeId;
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!("graph-compact-owner-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn memory_draft(name: &str) -> MemoryDraft {
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

fn entity_draft(name: &str) -> EntityDraft {
    EntityDraft {
        canonical_name: name.into(),
        aliases: Vec::new(),
        kind: "person".into(),
        summary: format!("Graph owner fixture {name}"),
        mutation_id: format!("entity-{name}"),
        created_at_ns: 11,
        updated_at_ns: 11,
    }
}

fn append_compact_deactivation(
    container: &mut crate::Container,
    source: u32,
    target: u32,
    graph_version: u64,
) {
    let body = crate::graph_compact_codec::publication::encode(
        &crate::graph_compact_codec::publication::GraphPublication {
            graph_version,
            transaction: crate::graph_compact_codec::publication::GraphTransaction::Single(
                GraphOrdinalMutation {
                    source,
                    target,
                    kind: SemanticGraphRelationKind::EntityAssociation,
                    active: false,
                    origin: crate::GraphRelationOrigin::Perception,
                },
            ),
        },
    )
    .unwrap();
    container.set_next_transaction_time_override(Some(50_000 + graph_version as i64));
    container.append_publication_envelope(&body).unwrap();
}

fn retired_association_history(path: &Path, phy: bool) -> ([u8; 32], [u8; 32]) {
    let (memory_id, entity_id, memory_revision, entity_revision, source_ordinal, target_ordinal) =
        if phy {
            let mut owner = Phylactery::create(path).unwrap();
            let memory = owner
                .publish_memory(None, 0, memory_draft("phy-typed"))
                .unwrap()
                .0;
            let entity = owner
                .publish_entity(None, 0, entity_draft("phy-target"))
                .unwrap()
                .0;
            owner
                .set_entity_association(memory.id, entity.id, true, 0)
                .unwrap();
            let source_ordinal = owner
                .graph
                .semantic_node_id(SemanticNodeRef::memory(memory.id))
                .unwrap()
                .0;
            let target_ordinal = owner
                .graph
                .semantic_node_id(SemanticNodeRef::entity(entity.id))
                .unwrap()
                .0;
            append_compact_deactivation(&mut owner.container, source_ordinal, target_ordinal, 2);
            owner.tombstone_memory(memory.id, memory.revision).unwrap();
            owner.tombstone_entity(entity.id, entity.revision).unwrap();
            (
                memory.id.0,
                entity.id.0,
                memory.revision,
                entity.revision,
                source_ordinal,
                target_ordinal,
            )
        } else {
            let mut owner = Cva::create(path).unwrap();
            let memory = owner
                .publish_memory(None, 0, memory_draft("rel-typed"))
                .unwrap()
                .0;
            let entity = owner
                .publish_entity(None, 0, entity_draft("rel-target"))
                .unwrap()
                .0;
            owner
                .set_entity_association(memory.id, entity.id, true, 0)
                .unwrap();
            let source_ordinal = owner
                .graph
                .semantic_node_id(SemanticNodeRef::memory(memory.id))
                .unwrap()
                .0;
            let target_ordinal = owner
                .graph
                .semantic_node_id(SemanticNodeRef::entity(entity.id))
                .unwrap()
                .0;
            append_compact_deactivation(&mut owner.container, source_ordinal, target_ordinal, 2);
            owner.tombstone_memory(memory.id, memory.revision).unwrap();
            owner.tombstone_entity(entity.id, entity.revision).unwrap();
            (
                memory.id.0,
                entity.id.0,
                memory.revision,
                entity.revision,
                source_ordinal,
                target_ordinal,
            )
        };

    // The compact endpoint decoder resolves retained typed mappings even once
    // lifecycle projections have hidden both nodes from current queries.
    if phy {
        let owner = Phylactery::open(path).unwrap();
        let compact = encode_mutation(GraphOrdinalMutation {
            source: source_ordinal,
            target: target_ordinal,
            kind: SemanticGraphRelationKind::EntityAssociation,
            active: false,
            origin: crate::GraphRelationOrigin::Perception,
        });
        let decoded = owner
            .graph
            .decode_stored_mutations(&compact)
            .unwrap()
            .unwrap();
        assert_eq!(
            decoded[0].source,
            SemanticNodeRef::memory(MemoryId(memory_id))
        );
        assert_eq!(
            decoded[0].target,
            SemanticNodeRef::entity(EntityId(entity_id))
        );
        let history = GraphHistory::read(&owner.container, &owner.graph).unwrap();
        let edge = (decoded[0].source, decoded[0].target, decoded[0].kind);
        assert!(history.at(1, false).unwrap().states[&edge].active);
        assert!(!history.at(2, false).unwrap().states[&edge].active);
        assert_eq!(
            owner
                .memory_retention_descriptor(MemoryId(memory_id))
                .unwrap()
                .tombstone_revision,
            memory_revision + 1
        );
        assert_eq!(
            owner
                .entity_retention_descriptor(EntityId(entity_id))
                .unwrap()
                .tombstone_revision,
            entity_revision + 1
        );
    } else {
        let owner = Cva::open(path).unwrap();
        let compact = encode_mutation(GraphOrdinalMutation {
            source: source_ordinal,
            target: target_ordinal,
            kind: SemanticGraphRelationKind::EntityAssociation,
            active: false,
            origin: crate::GraphRelationOrigin::Perception,
        });
        let decoded = owner
            .graph
            .decode_stored_mutations(&compact)
            .unwrap()
            .unwrap();
        assert_eq!(
            decoded[0].source,
            SemanticNodeRef::memory(MemoryId(memory_id))
        );
        assert_eq!(
            decoded[0].target,
            SemanticNodeRef::entity(EntityId(entity_id))
        );
        let history = GraphHistory::read(&owner.container, &owner.graph).unwrap();
        let edge = (decoded[0].source, decoded[0].target, decoded[0].kind);
        assert!(history.at(1, false).unwrap().states[&edge].active);
        assert!(!history.at(2, false).unwrap().states[&edge].active);
        assert_eq!(
            owner
                .memory_retention_descriptor(MemoryId(memory_id))
                .unwrap()
                .tombstone_revision,
            memory_revision + 1
        );
        assert_eq!(
            owner
                .entity_retention_descriptor(EntityId(entity_id))
                .unwrap()
                .tombstone_revision,
            entity_revision + 1
        );
    }
    (memory_id, entity_id)
}

#[test]
fn compact_mutations_materialize_typed_stable_ids_and_keep_principal_kind_distinct() {
    let mut graph = GraphStore::empty();
    let shared = [0x71; 32];
    let memory = SemanticNodeRef::memory(MemoryId(shared));
    let same_bytes_entity = SemanticNodeRef::entity(EntityId(shared));
    let principal = SemanticNodeRef::entity(EntityId([0x72; 32]));
    let other_memory = SemanticNodeRef::memory(MemoryId([0x73; 32]));
    for (ordinal, node) in [memory, same_bytes_entity, principal, other_memory]
        .into_iter()
        .enumerate()
    {
        graph
            .insert_node_rebuilt(crate::GraphNodeRecord {
                semantic_node: node,
                node_id: NodeId(ordinal as u32),
            })
            .unwrap();
    }
    let expected = vec![
        crate::graph_codec::GraphMutationPayload {
            source: memory,
            target: same_bytes_entity,
            kind: SemanticGraphRelationKind::EntityAssociation,
            active: true,
            origin: crate::GraphRelationOrigin::Perception,
        },
        crate::graph_codec::GraphMutationPayload {
            source: memory,
            target: principal,
            kind: SemanticGraphRelationKind::PrincipalAssociation,
            active: false,
            origin: crate::GraphRelationOrigin::Perception,
        },
        crate::graph_codec::GraphMutationPayload {
            source: memory,
            target: other_memory,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
            active: true,
            origin: crate::GraphRelationOrigin::User,
        },
    ];
    let ordinal = |source: u32, target: u32, kind, active, origin| GraphOrdinalMutation {
        source,
        target,
        kind,
        active,
        origin,
    };
    let bytes = encode_batch(&[
        ordinal(
            0,
            1,
            expected[0].kind,
            expected[0].active,
            expected[0].origin,
        ),
        ordinal(
            0,
            2,
            expected[1].kind,
            expected[1].active,
            expected[1].origin,
        ),
        ordinal(
            0,
            3,
            expected[2].kind,
            expected[2].active,
            expected[2].origin,
        ),
    ])
    .unwrap();
    assert_eq!(
        graph.decode_stored_mutations(&bytes).unwrap().unwrap(),
        expected
    );
    assert_ne!(
        memory, same_bytes_entity,
        "raw 32-byte IDs remain kind-tagged"
    );
    assert_ne!(
        expected[0].kind, expected[1].kind,
        "principal association stays distinct"
    );
}

#[test]
fn compact_association_history_survives_memory_and_entity_retirement_in_rel_and_phy() {
    let scratch = Scratch::new();
    let rel = retired_association_history(&scratch.0.join("retired.rel"), false);
    let phy = retired_association_history(&scratch.0.join("retired.phy"), true);
    assert_ne!(rel.0, [0; 32]);
    assert_ne!(rel.1, [0; 32]);
    assert_ne!(phy.0, [0; 32]);
    assert_ne!(phy.1, [0; 32]);
}
