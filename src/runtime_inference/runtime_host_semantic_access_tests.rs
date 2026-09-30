use crate::runtime_host_test_support::{one_worker, test_path};
use crate::{
    Cva, EchoEvent, EchoEventKind, EmbeddingEndpoint, EpisodePolicy, InteractionRuntime,
    MemoryDraft, Phylactery, ReliquaryRuntimeHost, ReliquaryRuntimeRoutes,
    RuntimeKnowledgeProvenanceResolution, RuntimeSemanticOwnerKind, SimulatedEmbeddingEndpoint,
    VectorNormalization,
};
use std::sync::Arc;

fn project(name: &str) -> (Cva, String) {
    let cva = Cva::create_project(test_path(name)).unwrap();
    let owner_id = cva.owner_id().unwrap();
    (cva, owner_id)
}

fn memory_draft(title: &str, memory_type: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: memory_type.into(),
        authority_kind: "direct".into(),
        title: title.into(),
        content: format!("{title} memory"),
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
        temporal_status: "unknown".into(),
        mutation_id: format!("semantic-{title}"),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn seed_rel_memory(mut cva: Cva, title: &str, endpoint: &SimulatedEmbeddingEndpoint) -> Cva {
    cva.publish_memory(None, 0, memory_draft(title, "project"))
        .unwrap();
    let profile = cva.establish_compatibility_profile(endpoint).unwrap();
    cva.build_missing_memory_vectors(profile.id, endpoint)
        .unwrap();
    cva
}

fn seed_phy_memory(
    mut phylactery: Phylactery,
    title: &str,
    endpoint: &SimulatedEmbeddingEndpoint,
) -> Phylactery {
    phylactery
        .publish_memory(None, 0, memory_draft(title, "user"))
        .unwrap();
    let profile = phylactery
        .establish_compatibility_profile(endpoint)
        .unwrap();
    phylactery
        .build_missing_memory_vectors(profile.id, endpoint)
        .unwrap();
    phylactery
}

fn seed_archive(mut cva: Cva, token: &str) -> Cva {
    cva.append_node(
        "u1".into(),
        "history".into(),
        None,
        "user".into(),
        1,
        &format!("historical archive {token}"),
    )
    .unwrap();
    cva.append_node(
        "a1".into(),
        "history".into(),
        Some("u1".into()),
        "assistant".into(),
        2,
        "acknowledged",
    )
    .unwrap();
    cva
}

#[test]
fn visible_semantic_owners_follow_dependency_closure_plus_phylactery() {
    let (root, root_id) = project("semantic-root.prj.rel");
    let (mut middle, middle_id) = project("semantic-middle.prj.rel");
    middle
        .set_rel_metadata(None, vec![root_id.clone()])
        .unwrap();
    let (mut leaf, leaf_id) = project("semantic-leaf.prj.rel");
    leaf.set_rel_metadata(None, vec![middle_id.clone()])
        .unwrap();
    let (sibling, sibling_id) = project("semantic-sibling.prj.rel");
    let phylactery = Phylactery::create(test_path("semantic-user.phy")).unwrap();
    let phy_id = phylactery.owner_id().unwrap();

    let mut host = ReliquaryRuntimeHost::new(
        ReliquaryRuntimeRoutes::default(),
        one_worker(),
        EpisodePolicy::default(),
    );
    host.mount_rel(InteractionRuntime::new(leaf)).unwrap();
    host.mount_rel(InteractionRuntime::new(sibling)).unwrap();
    host.mount_rel(InteractionRuntime::new(root)).unwrap();
    host.mount_rel(InteractionRuntime::new(middle)).unwrap();
    host.attach_phylactery(phylactery).unwrap();
    host.set_active_rel(&leaf_id).unwrap();

    let visible = host.visible_semantic_owners().unwrap();
    assert_eq!(
        visible
            .iter()
            .map(|owner| owner.owner_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            root_id.as_str(),
            middle_id.as_str(),
            leaf_id.as_str(),
            phy_id.as_str(),
        ]
    );
    assert_eq!(visible[0].kind, RuntimeSemanticOwnerKind::Reliquary);
    assert_eq!(visible[3].kind, RuntimeSemanticOwnerKind::Phylactery);

    assert_eq!(
        host.semantic_owner(&sibling_id).unwrap().kind,
        RuntimeSemanticOwnerKind::Reliquary
    );
    let error = host
        .visible_semantic_owner(&sibling_id)
        .unwrap_err()
        .to_string();
    assert!(error.contains("not visible from the active REL"));
}

#[test]
fn visible_memory_search_uses_dependency_closure_plus_phylactery() {
    let endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 77);
    let (root, root_id) = project("memory-root.prj.rel");
    let root = seed_rel_memory(root, "root-memory", &endpoint);

    let (mut leaf, leaf_id) = project("memory-leaf.prj.rel");
    leaf.set_rel_metadata(None, vec![root_id.clone()]).unwrap();
    let leaf = seed_rel_memory(leaf, "leaf-memory", &endpoint);

    let (sibling, sibling_id) = project("memory-sibling.prj.rel");
    let sibling = seed_rel_memory(sibling, "sibling-memory", &endpoint);

    let phylactery = Phylactery::create(test_path("memory-user.phy")).unwrap();
    let phy_id = phylactery.owner_id().unwrap();
    let phylactery = seed_phy_memory(phylactery, "user-memory", &endpoint);

    let embedding: Arc<dyn EmbeddingEndpoint + Send + Sync> = Arc::new(endpoint);
    let mut host = ReliquaryRuntimeHost::new(
        ReliquaryRuntimeRoutes::new(None, None, None, None, Some(embedding)),
        one_worker(),
        EpisodePolicy::default(),
    );
    host.mount_rel(InteractionRuntime::new(sibling)).unwrap();
    host.mount_rel(InteractionRuntime::new(root)).unwrap();
    host.mount_rel(InteractionRuntime::new(leaf)).unwrap();
    host.attach_phylactery(phylactery).unwrap();
    host.set_active_rel(&leaf_id).unwrap();

    let result = host.search_visible_memories("memory", 5, 3).unwrap();
    assert_eq!(
        result
            .owners
            .iter()
            .map(|owner| owner.owner.owner_id.as_str())
            .collect::<Vec<_>>(),
        vec![root_id.as_str(), leaf_id.as_str(), phy_id.as_str()]
    );
    assert!(
        result
            .owners
            .iter()
            .all(|owner| owner.error.is_none() && owner.lane.is_some())
    );
    assert!(
        !result
            .owners
            .iter()
            .any(|owner| owner.owner.owner_id == sibling_id)
    );
}

#[test]
fn visible_archive_search_uses_dependency_closure_and_excludes_phylactery() {
    let (root, root_id) = project("archive-root.prj.rel");
    let root = seed_archive(root, "root");

    let (mut leaf, leaf_id) = project("archive-leaf.prj.rel");
    leaf.set_rel_metadata(None, vec![root_id.clone()]).unwrap();
    let leaf = seed_archive(leaf, "leaf");

    let (sibling, sibling_id) = project("archive-sibling.prj.rel");
    let sibling = seed_archive(sibling, "sibling");

    let phylactery = Phylactery::create(test_path("archive-user.phy")).unwrap();
    let phy_id = phylactery.owner_id().unwrap();

    let endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 91);
    let embedding: Arc<dyn EmbeddingEndpoint + Send + Sync> = Arc::new(endpoint);
    let mut host = ReliquaryRuntimeHost::new(
        ReliquaryRuntimeRoutes::new(None, None, None, None, Some(embedding)),
        one_worker(),
        EpisodePolicy::default(),
    );
    host.mount_rel(InteractionRuntime::new(sibling)).unwrap();
    host.mount_rel(InteractionRuntime::new(root)).unwrap();
    host.mount_rel(InteractionRuntime::new(leaf)).unwrap();
    host.attach_phylactery(phylactery).unwrap();
    host.set_active_rel(&leaf_id).unwrap();

    let result = host.search_visible_archive("archive", 5).unwrap();
    assert_eq!(
        result
            .owners
            .iter()
            .map(|owner| owner.owner.owner_id.as_str())
            .collect::<Vec<_>>(),
        vec![root_id.as_str(), leaf_id.as_str()]
    );
    assert!(
        !result
            .owners
            .iter()
            .any(|owner| owner.owner.owner_id == sibling_id || owner.owner.owner_id == phy_id)
    );
    assert!(
        result.owners.iter().all(|owner| owner.error.is_none()),
        "archive owner errors: {:?}",
        result
            .owners
            .iter()
            .map(|owner| (&owner.owner.owner_id, &owner.error))
            .collect::<Vec<_>>()
    );
}

#[test]
fn owner_archive_search_falls_back_to_lexical_without_embedding_route() {
    let (rel, rel_id) = project("archive-lexical-fallback.prj.rel");
    let rel = seed_archive(rel, "lexical-fallback");

    let mut host = ReliquaryRuntimeHost::new(
        ReliquaryRuntimeRoutes::default(),
        one_worker(),
        EpisodePolicy::default(),
    );
    host.mount_rel(InteractionRuntime::new(rel)).unwrap();
    host.set_active_rel(&rel_id).unwrap();

    let result = host
        .search_archive_owner(&rel_id, "lexical-fallback", 5)
        .unwrap();

    assert!(
        result.error.is_none(),
        "archive search error: {:?}",
        result.error
    );
    assert!(!result.hits.is_empty());
}

#[test]
fn visible_semantic_searches_validate_their_owned_bounds() {
    let host = ReliquaryRuntimeHost::new(
        ReliquaryRuntimeRoutes::default(),
        one_worker(),
        EpisodePolicy::default(),
    );

    assert!(host.search_visible_memories("", 5, 3).is_err());
    assert!(
        host.search_visible_memories(&"x".repeat(513), 5, 3)
            .is_err()
    );
    assert!(host.search_visible_memories("valid", 11, 3).is_err());
    assert!(host.search_visible_memories("valid", 5, 0).is_err());

    assert!(host.search_visible_archive("", 5).is_err());
    assert!(host.search_visible_archive(&"x".repeat(513), 5).is_err());
    assert!(host.search_visible_archive("valid", 11).is_err());
}

#[test]
fn generic_knowledge_access_dispatches_by_owner_kind() {
    let (rel, rel_id) = project("semantic-knowledge.prj.rel");
    let phylactery = Phylactery::create(test_path("semantic-knowledge.phy")).unwrap();
    let phy_id = phylactery.owner_id().unwrap();

    let mut host = ReliquaryRuntimeHost::new(
        ReliquaryRuntimeRoutes::default(),
        one_worker(),
        EpisodePolicy::default(),
    );
    host.mount_rel(InteractionRuntime::new(rel)).unwrap();
    host.attach_phylactery(phylactery).unwrap();
    host.set_active_rel(&rel_id).unwrap();

    let rel_memory = host
        .create_knowledge_memory_for_owner(
            &rel_id,
            "REL fact".into(),
            "Project-owned knowledge".into(),
            "knowledge".into(),
            "fact".into(),
        )
        .unwrap();
    let phy_memory = host
        .create_knowledge_memory_for_owner(
            &phy_id,
            "PHY fact".into(),
            "User-owned knowledge".into(),
            "knowledge".into(),
            "fact".into(),
        )
        .unwrap();

    let rel_state = host.read_knowledge_for_owner(&rel_id).unwrap();
    let phy_state = host.read_knowledge_for_owner(&phy_id).unwrap();
    assert!(rel_state.0.iter().any(|memory| memory.id == rel_memory.id));
    assert!(phy_state.0.iter().any(|memory| memory.id == phy_memory.id));

    let resolution = host
        .resolve_knowledge_provenance_for_owner(&phy_id, phy_memory.id)
        .unwrap();
    assert!(matches!(
        resolution,
        RuntimeKnowledgeProvenanceResolution::None {
            memory_owner_id,
            provenance: None,
        } if memory_owner_id == phy_id
    ));

    let error = host
        .read_memory_provenance_for_owner(&phy_id, phy_memory.id)
        .unwrap_err()
        .to_string();
    assert!(error.contains("do not have Reliquary Archive provenance"));
}

#[test]
fn echo_resolution_is_owner_qualified_and_path_bounded() {
    let (mut rel, rel_id) = project("semantic-echo.prj.rel");
    rel.append_node(
        "u1".into(),
        "conversation".into(),
        None,
        "user".into(),
        1,
        "Question",
    )
    .unwrap();
    rel.append_node(
        "a1".into(),
        "conversation".into(),
        Some("u1".into()),
        "assistant".into(),
        2,
        "Answer",
    )
    .unwrap();
    rel.put_echo_event(EchoEvent {
        conversation_id: "conversation".into(),
        message_id: "a1".into(),
        sequence: 0,
        timestamp_ns: 3,
        model_round: Some(1),
        kind: EchoEventKind::ReasoningSummary,
        correlation_id: None,
        name: None,
        content: "Reasoned answer".into(),
    })
    .unwrap();
    rel.sync().unwrap();

    let mut host = ReliquaryRuntimeHost::new(
        ReliquaryRuntimeRoutes::default(),
        one_worker(),
        EpisodePolicy::default(),
    );
    host.mount_rel(InteractionRuntime::new(rel)).unwrap();
    host.set_active_rel(&rel_id).unwrap();

    let source = host
        .read_echo_source_for(&rel_id, "conversation", "u1", "a1")
        .unwrap();
    assert_eq!(source.owner_id, rel_id);
    assert_eq!(source.turns.len(), 1);
    assert_eq!(source.turns[0].message_id, "a1");
    assert_eq!(source.turns[0].events[0].content, "Reasoned answer");

    let error = host
        .read_echo_source_for(&source.owner_id, "conversation", "missing", "a1")
        .unwrap_err()
        .to_string();
    assert!(error.contains("start is not on the returned transcript path"));
}
