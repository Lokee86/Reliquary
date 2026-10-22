use crate::{
    EntityId, GraphRelationKind, GraphRelationOrigin, MemoryId, SemanticGraphRelationKind,
    SemanticNodeKind, SemanticNodeRef,
};

#[test]
fn semantic_node_identity_distinguishes_node_kinds() {
    let raw = [7_u8; 32];
    let memory = SemanticNodeRef::memory(MemoryId(raw));
    let entity = SemanticNodeRef::entity(EntityId(raw));

    assert_ne!(memory, entity);
    assert_eq!(memory.kind, SemanticNodeKind::Memory);
    assert_eq!(entity.kind, SemanticNodeKind::Entity);
    assert_eq!(memory.as_memory(), Some(MemoryId(raw)));
    assert_eq!(entity.as_entity(), Some(EntityId(raw)));
}

#[test]
fn graph_node_codec_reads_legacy_memory_and_typed_entity_nodes() {
    use crate::graph_codec::{GraphNodePayload, decode_node, encode_node};
    use arcana::NodeId;

    let memory = GraphNodePayload {
        semantic_node: SemanticNodeRef::memory(MemoryId([1; 32])),
        node_id: NodeId(3),
    };
    let memory_bytes = encode_node(memory);
    assert_eq!(&memory_bytes[..8], b"CVAGNODE");
    assert_eq!(decode_node(&memory_bytes).unwrap(), Some(memory));

    let entity = GraphNodePayload {
        semantic_node: SemanticNodeRef::entity(EntityId([2; 32])),
        node_id: NodeId(4),
    };
    let entity_bytes = encode_node(entity);
    assert_eq!(&entity_bytes[..8], b"CVAGNOD2");
    assert_eq!(decode_node(&entity_bytes).unwrap(), Some(entity));
}

#[test]
fn typed_entity_association_codec_roundtrips() {
    use crate::graph_codec::{GraphMutationPayload, decode_mutation, encode_mutation};

    let payload = GraphMutationPayload {
        source: SemanticNodeRef::memory(MemoryId([1; 32])),
        target: SemanticNodeRef::entity(EntityId([2; 32])),
        kind: SemanticGraphRelationKind::EntityAssociation,
        active: true,
        origin: GraphRelationOrigin::Perception,
    };
    let bytes = encode_mutation(payload);
    assert_eq!(&bytes[..8], b"CVAGMUT2");
    assert_eq!(decode_mutation(&bytes).unwrap(), Some(payload));
}
