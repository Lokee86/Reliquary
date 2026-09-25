use crate::entity_principal::{
    PRINCIPAL_ENTITY_KIND, principal_entity_global_id, principal_entity_id,
};
use crate::entity_reconciliation_candidates::reconciliation_candidates;
use crate::{
    Cva, EntityDraft, EntityError, EntityId, EntityResolutionDecision, EntityResolutionReason,
    EpisodeBoundary, EpisodeConfig, EpisodeId, EpisodeOrigin, GraphDirection, GraphError,
    MemoryDraft, MemoryEntityMention, MemoryEntityMentionKey, MemoryRoutingMetadata,
    MemorySourceRef, MemoryTextField, Phylactery, SemanticNodeRef,
};
use std::path::PathBuf;

const PRINCIPAL_A: &str = "phy-00000000-0000-0000-0000-0000000000a1";
const PRINCIPAL_B: &str = "phy-00000000-0000-0000-0000-0000000000b2";

#[test]
fn rel_owner_relative_mention_materializes_isolated_principal_entity() {
    let path = temp_path("principal-rel");
    let mut rel = Cva::create_project(&path).unwrap();
    let memory = rel_memory(&mut rel, "c1", "u1", PRINCIPAL_A, "The user chose SQLite.");
    let key = install_rel_mention(&mut rel, memory.id, "The user");

    let outcome = rel
        .resolve_principal_entity_mention(key, 100)
        .unwrap()
        .unwrap();
    let entity_id = principal_entity_id(PRINCIPAL_A);
    assert_eq!(outcome.decision, EntityResolutionDecision::CreateNew);
    assert_eq!(outcome.reason, EntityResolutionReason::PrincipalIdentity);
    assert_eq!(outcome.entity_id, Some(entity_id));

    let entity = rel.entity(entity_id).unwrap();
    assert_eq!(
        entity.global_id,
        Some(principal_entity_global_id(PRINCIPAL_A))
    );
    assert_eq!(entity.kind, PRINCIPAL_ENTITY_KIND);
    assert_eq!(entity.canonical_name, PRINCIPAL_A);
    assert_eq!(
        rel.principal_associations_for_memory(memory.id),
        vec![entity_id]
    );
    assert!(rel.entity_associations_for_memory(memory.id).is_empty());
    assert_eq!(rel.memories_for_principal(entity_id), vec![memory.id]);
    assert_eq!(rel.principal_entity(PRINCIPAL_A).unwrap().id, entity_id);
    assert_eq!(rel.memories_for_phy_principal(PRINCIPAL_A), vec![memory.id]);
    assert!(rel.principal_entity("user").is_none());
    assert!(
        rel.entity_candidates_for_surface(PRINCIPAL_A, 10)
            .is_empty()
    );
    assert!(
        rel.entity_candidates_for_normalized_surface(PRINCIPAL_A, 10)
            .is_empty()
    );
    assert!(reconciliation_candidates(&rel.entities(), 16).is_empty());
    let audit = rel.audit_entities();
    assert_eq!(audit.entity_count, 0);
    assert!(audit.zero_degree_entities.is_empty());
    assert!(audit.missing_resolved_targets.is_empty());
    assert!(rel.semantic_graph_relations().is_empty());
    assert!(
        rel.semantic_graph_neighbors(SemanticNodeRef::memory(memory.id), GraphDirection::Outgoing)
            .unwrap()
            .is_empty()
    );
    assert!(
        rel.refresh_communities_leiden()
            .unwrap()
            .communities
            .is_empty()
    );

    rel.sync().unwrap();
    drop(rel);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened.principal_associations_for_memory(memory.id),
        vec![entity_id]
    );
    assert!(
        reopened
            .entity_associations_for_memory(memory.id)
            .is_empty()
    );
}

#[test]
fn principal_profile_updates_aliases_without_changing_identity() {
    let mut rel = Cva::create_project(temp_path("principal-profile")).unwrap();
    let memory = rel_memory(&mut rel, "c1", "u1", PRINCIPAL_A, "The user chose SQLite.");
    let key = install_rel_mention(&mut rel, memory.id, "The user");
    let entity_id = rel
        .resolve_principal_entity_mention(key, 100)
        .unwrap()
        .unwrap()
        .entity_id
        .unwrap();

    assert!(
        rel.sync_principal_profile(
            PRINCIPAL_A,
            &crate::PhylacteryProfile {
                display_name: Some("Example User".into()),
            },
            200,
        )
        .unwrap()
    );
    let first = rel.entity(entity_id).unwrap();
    assert_eq!(first.id, entity_id);
    assert_eq!(first.canonical_name, PRINCIPAL_A);
    assert_eq!(first.aliases, vec!["Example User"]);
    assert_eq!(
        first.summary,
        "Phylactery-backed user principal: Example User."
    );
    assert!(
        rel.entity_candidates_for_surface("Example User", 10)
            .is_empty()
    );

    assert!(
        rel.sync_principal_profile(
            PRINCIPAL_A,
            &crate::PhylacteryProfile {
                display_name: Some("Example Renamed".into()),
            },
            300,
        )
        .unwrap()
    );
    let renamed = rel.entity(entity_id).unwrap();
    assert_eq!(renamed.id, entity_id);
    assert_eq!(renamed.global_id, first.global_id);
    assert_eq!(renamed.canonical_name, PRINCIPAL_A);
    assert_eq!(renamed.aliases, vec!["Example Renamed"]);
    assert!(!renamed.aliases.contains(&"Example User".to_string()));
    assert_eq!(rel.memories_for_phy_principal(PRINCIPAL_A), vec![memory.id]);
}

#[test]
fn same_rel_keeps_phy_principals_distinct() {
    let mut rel = Cva::create_project(temp_path("multi-principal")).unwrap();
    let a = rel_memory(&mut rel, "ca", "ua", PRINCIPAL_A, "The user chose A.");
    let b = rel_memory(&mut rel, "cb", "ub", PRINCIPAL_B, "The user chose B.");
    let key_a = install_rel_mention(&mut rel, a.id, "The user");
    let key_b = install_rel_mention(&mut rel, b.id, "The user");

    let out_a = rel
        .resolve_principal_entity_mention(key_a, 100)
        .unwrap()
        .unwrap();
    let out_b = rel
        .resolve_principal_entity_mention(key_b, 101)
        .unwrap()
        .unwrap();
    assert_ne!(out_a.entity_id, out_b.entity_id);
    assert_eq!(rel.entity_stats().entities, 2);
}

#[test]
fn phy_owner_relative_mention_is_rejected_without_materializing_entity() {
    let mut phy = Phylactery::create(temp_path("principal-phy")).unwrap();
    let principal = phy.owner_id().unwrap();
    let draft = memory_draft("The user prefers concise answers.", "phy-user");
    let (memory, _) = phy
        .publish_memory_with_source_ref(
            None,
            0,
            draft,
            MemorySourceRef {
                owner_id: "rel-00000000-0000-0000-0000-000000000001".into(),
                principal_id: Some(principal),
                source_episode_id: EpisodeId([7; 32]),
                source_node_id: "u1".into(),
                content_source_conversation_id: None,
                content_source_node_id: None,
                grounding_source_conversation_id: None,
                grounding_source_node_id: None,
            },
        )
        .unwrap();
    let key = install_phy_mention(&mut phy, memory.id, "The user");

    let outcome = phy
        .resolve_principal_entity_mention(key, 100)
        .unwrap()
        .unwrap();
    assert_eq!(outcome.decision, EntityResolutionDecision::Reject);
    assert_eq!(outcome.reason, EntityResolutionReason::PrincipalIdentity);
    assert!(outcome.entity_id.is_none());
    assert_eq!(phy.entity_stats().entities, 0);

    let (direct, _) = phy
        .publish_memory(
            None,
            0,
            memory_draft("The user prefers local models.", "phy-direct"),
        )
        .unwrap();
    let direct_key = install_phy_mention(&mut phy, direct.id, "The user");
    let direct_outcome = phy
        .resolve_principal_entity_mention(direct_key, 101)
        .unwrap()
        .unwrap();
    assert_eq!(direct_outcome.decision, EntityResolutionDecision::Reject);
    assert_eq!(
        direct_outcome.reason,
        EntityResolutionReason::PrincipalIdentity
    );
    assert_eq!(phy.entity_stats().entities, 0);

    assert!(matches!(
        phy.publish_entity(
            None,
            0,
            EntityDraft {
                canonical_name: "phy-test".into(),
                aliases: Vec::new(),
                kind: PRINCIPAL_ENTITY_KIND.into(),
                summary: String::new(),
                mutation_id: "forbidden-principal".into(),
                created_at_ns: 0,
                updated_at_ns: 0,
            }
        ),
        Err(EntityError::InvalidField("Phylactery principal Entity"))
    ));
}

#[test]
fn malformed_principal_entities_are_rejected() {
    let mut rel = Cva::create_project(temp_path("principal-identity-contract")).unwrap();
    assert!(matches!(
        rel.publish_entity(
            Some(EntityId([9; 32])),
            0,
            EntityDraft {
                canonical_name: PRINCIPAL_B.into(),
                aliases: Vec::new(),
                kind: PRINCIPAL_ENTITY_KIND.into(),
                summary: "Malformed principal".into(),
                mutation_id: "bad-principal-id".into(),
                created_at_ns: 0,
                updated_at_ns: 0,
            },
        ),
        Err(EntityError::InvalidField("principal Entity ID"))
    ));
    assert!(matches!(
        rel.publish_entity(
            Some(principal_entity_id("not-a-phy")),
            0,
            EntityDraft {
                canonical_name: "not-a-phy".into(),
                aliases: Vec::new(),
                kind: PRINCIPAL_ENTITY_KIND.into(),
                summary: "Malformed principal".into(),
                mutation_id: "bad-principal-name".into(),
                created_at_ns: 0,
                updated_at_ns: 0,
            },
        ),
        Err(EntityError::InvalidField("principal Entity identity"))
    ));
}

#[test]
fn principal_association_kind_cannot_be_mixed_with_regular_entities() {
    let mut rel = Cva::create_project(temp_path("principal-graph-contract")).unwrap();
    let memory = rel_memory(&mut rel, "c1", "u1", PRINCIPAL_A, "The user chose SQLite.");
    let key = install_rel_mention(&mut rel, memory.id, "The user");
    let principal = rel
        .resolve_principal_entity_mention(key, 100)
        .unwrap()
        .unwrap()
        .entity_id
        .unwrap();
    let (ordinary, _) = rel
        .publish_entity(
            None,
            0,
            EntityDraft {
                canonical_name: "SQLite".into(),
                aliases: Vec::new(),
                kind: "technology".into(),
                summary: "Database".into(),
                mutation_id: "ordinary-sqlite".into(),
                created_at_ns: 0,
                updated_at_ns: 0,
            },
        )
        .unwrap();

    assert!(matches!(
        rel.set_entity_association(memory.id, principal, true, rel.graph_version()),
        Err(GraphError::InvalidRelationShape)
    ));
    assert!(matches!(
        rel.set_principal_association(memory.id, ordinary.id, true, rel.graph_version()),
        Err(GraphError::InvalidRelationShape)
    ));
}

fn rel_memory(
    rel: &mut Cva,
    conversation: &str,
    node: &str,
    principal: &str,
    content: &str,
) -> crate::Memory {
    rel.append_node_with_principal(
        node.into(),
        conversation.into(),
        None,
        "user".into(),
        Some(principal.into()),
        10,
        content,
    )
    .unwrap();
    let episode = rel
        .materialize_path_episodes(
            conversation,
            node,
            EpisodeConfig::default(),
            EpisodeOrigin::Live,
            Some((EpisodeBoundary::Inactivity, 20)),
        )
        .unwrap()
        .created[0]
        .clone();
    let mut draft = memory_draft(content, node);
    draft.source_node_id = Some(node.into());
    draft.source_episode_id = Some(episode.id);
    rel.publish_memory(None, 0, draft).unwrap().0
}

fn memory_draft(content: &str, mutation: &str) -> MemoryDraft {
    MemoryDraft {
        category: "decision".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "Principal test".into(),
        content: content.into(),
        scope: "project".into(),
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
        source_time_ns: Some(10),
        mutation_id: format!("principal-test-{mutation}-{}", uuid::Uuid::new_v4()),
        created_at_ns: 20,
        updated_at_ns: 20,
    }
}

fn install_rel_mention(rel: &mut Cva, id: crate::MemoryId, text: &str) -> MemoryEntityMentionKey {
    let body = rel.memory_body_id(id).unwrap();
    let content = rel.memory(id).unwrap().content;
    install_mention(
        &mut rel.memories,
        &mut rel.container,
        id,
        body,
        &content,
        text,
    )
}

fn install_phy_mention(
    phy: &mut Phylactery,
    id: crate::MemoryId,
    text: &str,
) -> MemoryEntityMentionKey {
    let body = phy.memory_body_id(id).unwrap();
    let content = phy.memory(id).unwrap().content;
    install_mention(
        &mut phy.memories,
        &mut phy.container,
        id,
        body,
        &content,
        text,
    )
}

fn install_mention(
    memories: &mut crate::memory_store::MemoryStore,
    container: &mut crate::Container,
    id: crate::MemoryId,
    body_id: crate::MemoryBodyId,
    content: &str,
    text: &str,
) -> MemoryEntityMentionKey {
    let start = content.find(text).unwrap() as u32;
    let mention = MemoryEntityMention {
        field: MemoryTextField::Content,
        start_byte: start,
        end_byte: start + text.len() as u32,
        text: text.into(),
    };
    memories
        .put_routing_metadata(
            container,
            MemoryRoutingMetadata {
                memory_id: id,
                body_id,
                entity_mentions: vec![mention.clone()],
            },
        )
        .unwrap();
    MemoryEntityMentionKey::new(id, &mention)
}

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{name}-{}.rel", uuid::Uuid::new_v4()))
}
