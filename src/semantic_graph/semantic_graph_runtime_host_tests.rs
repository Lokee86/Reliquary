use crate::runtime_host_test_support::{one_worker, test_path};
use crate::{
    Cva, EntityDraft, EpisodePolicy, GatewayRuntime, GraphRelationOrigin, InteractionRuntime,
    MemoryDraft, OwnerGrant, ReliquaryRuntimeRoutes, SemanticGraphRelationKind, SemanticNodeRef,
};

#[test]
fn knowledge_read_exposes_entities_and_perception_relations() {
    let mut cva = Cva::create_project(test_path("knowledge-entity.rel")).unwrap();
    let (memory, _) = cva.publish_memory(None, 0, memory_draft()).unwrap();
    let (entity, _) = cva
        .publish_entity(
            None,
            0,
            EntityDraft {
                canonical_name: "Reliquary".into(),
                aliases: vec!["REL".into()],
                kind: "project".into(),
                summary: "Project referent.".into(),
                mutation_id: "knowledge-interface-entity".into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
        )
        .unwrap();
    cva.set_entity_association(memory.id, entity.id, true, cva.graph_version())
        .unwrap();

    let host = GatewayRuntime::new(
        test_path("knowledge-gateway-state"),
        ReliquaryRuntimeRoutes::default(),
        one_worker(),
        EpisodePolicy::default(),
    );
    let owner = host.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let instance = host
        .attach_instance(
            "graph-reader".into(),
            None,
            std::collections::BTreeMap::from([(
                owner.clone(),
                std::collections::BTreeSet::from([OwnerGrant::ViewRel]),
            )]),
        )
        .unwrap();
    let (memories, entities, relations, _, _) = host
        .read_reliquary_knowledge_for(&instance, &owner)
        .unwrap();

    assert!(memories.iter().any(|value| value.id == memory.id));
    assert!(entities.iter().any(|value| value.id == entity.id));
    assert!(relations.iter().any(|relation| {
        relation.source == SemanticNodeRef::memory(memory.id)
            && relation.target == SemanticNodeRef::entity(entity.id)
            && relation.kind == SemanticGraphRelationKind::EntityAssociation
            && relation.origin == GraphRelationOrigin::Perception
    }));
}

fn memory_draft() -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: "Entity memory".into(),
        content: "Entity memory body".into(),
        scope: "private".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(1),
        mutation_id: "entity-memory".into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}
