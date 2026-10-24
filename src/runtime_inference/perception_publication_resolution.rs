use crate::entity_principal::PrincipalResolutionPlan;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::lexical_index::LexicalIndex;
use crate::memory_store::MemoryReadView;
use crate::{
    EntityCandidateConfig, EntityResolutionPreparation, EntityResolverError, PhylacteryProfile,
};

pub(crate) fn prepare_entity_resolution(
    memories: crate::memory_store::MemoryReadView<'_>,
    lexical_index: &mut LexicalIndex,
    entities: &EntityStore,
    graph: &GraphStore,
    resolutions: &EntityResolutionStore,
    key: crate::MemoryEntityMentionKey,
    config: EntityCandidateConfig,
    owner_id: Option<String>,
) -> Result<EntityResolutionPreparation, EntityResolverError> {
    let mut candidates = crate::entity_candidates::EntityCandidateContext {
        memories: crate::lexical_search::MemoryLexicalReadView::new(lexical_index, memories),
        entities,
        graph,
    };
    crate::entity_resolution_prepare::prepare_entity_resolution(
        &mut crate::entity_resolution_prepare::EntityResolutionContext {
            candidates,
            resolutions,
        },
        key,
        config,
        owner_id,
    )
}

pub(crate) fn plan_rel_principal_resolution(
    memories: crate::memory_store::MemoryReadView<'_>,
    entities: &EntityStore,
    resolutions: &EntityResolutionStore,
    graph: &GraphStore,
    archive: &crate::Archive,
    key: crate::MemoryEntityMentionKey,
    now_ns: i64,
) -> Result<Option<PrincipalResolutionPlan>, EntityResolverError> {
    crate::entity_principal::PrincipalContext {
        memories,
        entities,
        entity_resolutions: resolutions,
        graph,
        archive: Some(archive),
    }
    .resolve_principal_entity_mention(key, now_ns)
}

pub(crate) fn plan_phy_principal_resolution(
    memories: crate::memory_store::MemoryReadView<'_>,
    entities: &EntityStore,
    resolutions: &EntityResolutionStore,
    graph: &GraphStore,
    key: crate::MemoryEntityMentionKey,
    now_ns: i64,
) -> Result<Option<PrincipalResolutionPlan>, EntityResolverError> {
    crate::entity_principal::PrincipalContext {
        memories,
        entities,
        entity_resolutions: resolutions,
        graph,
        archive: None,
    }
    .reject_phy_principal_entity_mention(key, now_ns)
}

pub(crate) fn plan_principal_profile_sync(
    memories: MemoryReadView<'_>,
    entities: &EntityStore,
    resolutions: &EntityResolutionStore,
    graph: &GraphStore,
    archive: &crate::Archive,
    principal_id: &str,
    profile: &PhylacteryProfile,
    now_ns: i64,
) -> Result<Option<(crate::EntityId, u64, crate::EntityDraft)>, crate::EntityError> {
    crate::entity_principal::PrincipalContext {
        memories,
        entities,
        entity_resolutions: resolutions,
        graph,
        archive: Some(archive),
    }
    .sync_principal_profile(principal_id, profile, now_ns)
}
