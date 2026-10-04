use crate::entity_principal::{PRINCIPAL_ENTITY_KIND, principal_entity_id};
use crate::runtime_host_test_support::{one_worker, test_path};
use crate::{
    Cva, EntityDraft, EpisodeId, EpisodePolicy, GraphRelationKind, GraphRelationOrigin,
    InteractionRuntime, Memory, MemoryDraft, MemorySourceRef, Phylactery, ReliquaryRuntimeHost,
    ReliquaryRuntimeRoutes, SemanticGraphRelationKind, SemanticNodeRef,
};

#[test]
fn knowledge_replacement_preserves_lifecycle_edges_and_retires_original() {
    let mut cva = Cva::create(test_path("knowledge-replace.rel")).unwrap();
    let (original, _) = cva
        .publish_memory(None, 0, draft("original", "Original", "knowledge", 1))
        .unwrap();
    let (peer, _) = cva
        .publish_memory(None, 0, draft("peer", "Peer", "knowledge", 2))
        .unwrap();
    cva.set_memory_relation(
        original.id,
        peer.id,
        GraphRelationKind::Topical,
        true,
        cva.graph_version(),
    )
    .unwrap();
    let host = host(cva);

    let replacement = host
        .replace_reliquary_knowledge_memory(original.id, "Edited".into(), "Edited body".into(), 10)
        .unwrap();
    let (memories, _, relations, _, _) = host.read_reliquary_knowledge().unwrap();
    let old = memory(&memories, original.id);
    let new = memory(&memories, replacement.id);

    assert_eq!(new.lifecycle_state, "knowledge");
    assert!(!new.archived);
    assert!(old.archived);
    assert_eq!(old.superseded_by, Some(replacement.id));
    assert!(relations.iter().any(|relation| {
        relation.source == SemanticNodeRef::memory(replacement.id)
            && relation.target == SemanticNodeRef::memory(peer.id)
            && relation.kind == SemanticGraphRelationKind::Memory(GraphRelationKind::Topical)
            && relation.origin == GraphRelationOrigin::Dream
    }));
    assert!(relations.iter().any(|relation| {
        relation.source == SemanticNodeRef::memory(replacement.id)
            && relation.target == SemanticNodeRef::memory(original.id)
            && relation.kind == SemanticGraphRelationKind::Memory(GraphRelationKind::Supersedes)
            && relation.origin == GraphRelationOrigin::User
    }));
}

#[test]
fn knowledge_manual_creation_and_relation_mutation_are_user_owned() {
    let host = host(Cva::create(test_path("knowledge-create.rel")).unwrap());
    let first = host
        .create_reliquary_knowledge_memory(
            "First".into(),
            "First body".into(),
            "fact".into(),
            "project".into(),
            10,
        )
        .unwrap();
    let second = host
        .create_reliquary_knowledge_memory(
            "Second".into(),
            "Second body".into(),
            "fact".into(),
            "project".into(),
            11,
        )
        .unwrap();
    assert_eq!(first.lifecycle_state, "extracted");
    assert!(first.mutation_id.starts_with("user_creation:"));

    host.mutate_reliquary_knowledge_relation(
        first.id,
        second.id,
        None,
        Some(GraphRelationKind::Topical),
    )
    .unwrap();
    let (_, _, relations, _, _) = host.read_reliquary_knowledge().unwrap();
    assert_eq!(relations.len(), 1);
    assert_eq!(relations[0].origin, GraphRelationOrigin::User);

    host.mutate_reliquary_knowledge_relation(
        first.id,
        second.id,
        Some(GraphRelationKind::Topical),
        Some(GraphRelationKind::References),
    )
    .unwrap();
    let (_, _, relations, _, _) = host.read_reliquary_knowledge().unwrap();
    assert_eq!(relations.len(), 1);
    assert_eq!(
        relations[0].kind,
        SemanticGraphRelationKind::Memory(GraphRelationKind::References)
    );
    assert_eq!(relations[0].origin, GraphRelationOrigin::User);

    host.mutate_reliquary_knowledge_relation(
        first.id,
        second.id,
        Some(GraphRelationKind::References),
        None,
    )
    .unwrap();
    assert!(host.read_reliquary_knowledge().unwrap().2.is_empty());
}

#[test]
fn generic_knowledge_read_hides_principal_entities_and_associations() {
    let mut cva = Cva::create(test_path("knowledge-principal.rel")).unwrap();
    let (memory, _) = cva
        .publish_memory(
            None,
            0,
            draft("principal-memory", "Principal memory", "knowledge", 1),
        )
        .unwrap();
    let principal = "phy-00000000-0000-0000-0000-000000000123";
    let entity_id = principal_entity_id(principal);
    cva.publish_entity(
        Some(entity_id),
        0,
        EntityDraft {
            canonical_name: principal.into(),
            aliases: vec!["Brian".into()],
            kind: PRINCIPAL_ENTITY_KIND.into(),
            summary: "User principal".into(),
            mutation_id: "knowledge-principal".into(),
            created_at_ns: 1,
            updated_at_ns: 1,
        },
    )
    .unwrap();
    cva.set_principal_association(memory.id, entity_id, true, cva.graph_version())
        .unwrap();

    let host = host(cva);
    let (memories, entities, relations, _, _) = host.read_reliquary_knowledge().unwrap();
    assert_eq!(memories.len(), 1);
    assert!(entities.is_empty());
    assert!(relations.is_empty());
}

#[test]
fn phylactery_replacement_preserves_external_source_reference() {
    let cva = Cva::create(test_path("knowledge-source.rel")).unwrap();
    let mut phy = Phylactery::create(test_path("knowledge-source.phy")).unwrap();
    let source_ref = MemorySourceRef {
        owner_id: "rel-source".into(),
        principal_id: None,
        source_episode_id: EpisodeId([7; 32]),
        source_node_id: "u1".into(),
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
    };
    let (original, _) = phy
        .publish_memory_with_source_ref(
            None,
            0,
            draft("phy-original", "Original PHY", "knowledge", 1),
            source_ref.clone(),
        )
        .unwrap();
    let host = ReliquaryRuntimeHost::start_with_phylactery(
        InteractionRuntime::new(cva),
        phy,
        ReliquaryRuntimeRoutes::default(),
        one_worker(),
        EpisodePolicy::default(),
    )
    .unwrap();

    let replacement = host
        .replace_phylactery_knowledge_memory(
            original.id,
            "Edited PHY".into(),
            "Edited PHY body".into(),
            10,
        )
        .unwrap();
    let (memories, _, _, _, _) = host.read_phylactery_knowledge().unwrap().unwrap();
    assert_eq!(
        memory(&memories, replacement.id).source_ref,
        Some(source_ref)
    );
    let old = memory(&memories, original.id);
    assert!(old.archived);
    assert_eq!(old.superseded_by, Some(replacement.id));
}

fn host(cva: Cva) -> ReliquaryRuntimeHost {
    ReliquaryRuntimeHost::start_inactive(
        InteractionRuntime::new(cva),
        ReliquaryRuntimeRoutes::default(),
        one_worker(),
        EpisodePolicy::default(),
    )
    .unwrap()
}

fn memory(memories: &[Memory], id: crate::MemoryId) -> &Memory {
    memories.iter().find(|memory| memory.id == id).unwrap()
}

fn draft(mutation_id: &str, title: &str, lifecycle_state: &str, time: i64) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        title: title.into(),
        content: format!("{title} body"),
        scope: "private".into(),
        lifecycle_state: lifecycle_state.into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(time),
        temporal_status: "unknown".into(),
        mutation_id: mutation_id.into(),
        created_at_ns: time,
        updated_at_ns: time,
    }
}
