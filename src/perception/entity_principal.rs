use crate::entity_principal_identity::{
    PRINCIPAL_ENTITY_KIND, is_owner_relative_principal_surface, is_phy_principal_id,
    principal_entity_id,
};
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryReadView;
use crate::{
    EntityDraft, EntityId, EntityResolutionDecision, EntityResolutionOutcome,
    EntityResolutionReason, EntityResolverError, MemoryEntityMentionKey,
    MemoryEntityResolutionStatus,
};

#[derive(Debug)]
pub(crate) struct PrincipalResolutionPlan {
    pub(crate) outcome: EntityResolutionOutcome,
    pub(crate) entity_draft: Option<(EntityId, EntityDraft)>,
    pub(crate) associate: bool,
    pub(crate) expected_revision: u32,
    pub(crate) should_apply: bool,
}

pub(crate) struct PrincipalContext<'a> {
    pub(crate) memories: MemoryReadView<'a>,
    pub(crate) entities: &'a EntityStore,
    pub(crate) entity_resolutions: &'a crate::entity_resolution_store::EntityResolutionStore,
    pub(crate) graph: &'a GraphStore,
    pub(crate) archive: Option<&'a crate::Archive>,
}

impl PrincipalContext<'_> {
    pub(crate) fn resolve_principal_entity_mention(
        &self,
        key: MemoryEntityMentionKey,
        now_ns: i64,
    ) -> Result<Option<PrincipalResolutionPlan>, EntityResolverError> {
        if !self.principal_mention_matches(key) {
            return Ok(None);
        }
        if let Some(outcome) = terminal_outcome(key, self.entity_resolutions.get(key)) {
            return Ok(Some(PrincipalResolutionPlan {
                outcome,
                entity_draft: None,
                associate: false,
                expected_revision: 0,
                should_apply: false,
            }));
        }

        let memory = self.memories.memory(key.memory_id)?;
        let Some(principal_id) = self
            .archive
            .and_then(|archive| rel_source_principal(archive, &memory))
        else {
            return Ok(None);
        };
        if !is_phy_principal_id(&principal_id) {
            return Ok(None);
        }

        let entity_id = principal_entity_id(&principal_id);
        let existing = self.entities.contains(entity_id);
        let entity_draft = if existing {
            let entity = self.entities.entity(entity_id)?;
            if entity.kind != PRINCIPAL_ENTITY_KIND || entity.canonical_name != principal_id {
                return Err(crate::EntityError::InvalidField("principal Entity identity").into());
            }
            None
        } else {
            Some((
                entity_id,
                EntityDraft {
                    canonical_name: principal_id.clone(),
                    aliases: Vec::new(),
                    kind: PRINCIPAL_ENTITY_KIND.into(),
                    summary: "Durable Phylactery-backed user principal.".into(),
                    mutation_id: format!("principal-entity:{principal_id}"),
                    created_at_ns: 0,
                    updated_at_ns: 0,
                },
            ))
        };
        let expected_revision = self
            .entity_resolutions
            .get(key)
            .map_or(0, |value| value.revision);
        Ok(Some(PrincipalResolutionPlan {
            outcome: EntityResolutionOutcome {
                key,
                decision: if existing {
                    EntityResolutionDecision::ResolveExisting(entity_id)
                } else {
                    EntityResolutionDecision::CreateNew
                },
                reason: EntityResolutionReason::PrincipalIdentity,
                entity_id: Some(entity_id),
                entity_created: !existing,
                association_changed: false,
                resolution_changed: false,
            },
            entity_draft,
            associate: true,
            expected_revision,
            should_apply: true,
        }))
    }

    pub(crate) fn sync_principal_profile(
        &self,
        principal_id: &str,
        profile: &crate::PhylacteryProfile,
        now_ns: i64,
    ) -> Result<Option<(EntityId, u64, EntityDraft)>, crate::EntityError> {
        if !is_phy_principal_id(principal_id) {
            return Err(crate::EntityError::InvalidField(
                "principal Entity identity",
            ));
        }
        let entity_id = principal_entity_id(principal_id);
        let Ok(entity) = self.entities.entity(entity_id) else {
            return Ok(None);
        };
        if entity.kind != PRINCIPAL_ENTITY_KIND || entity.canonical_name != principal_id {
            return Err(crate::EntityError::InvalidField(
                "principal Entity identity",
            ));
        }

        let aliases = principal_profile_aliases(profile);
        let summary = principal_profile_summary(profile);
        if entity.aliases == aliases && entity.summary == summary {
            return Ok(None);
        }
        let draft = EntityDraft {
            canonical_name: entity.canonical_name,
            aliases,
            kind: entity.kind,
            summary,
            mutation_id: format!("principal-profile:{principal_id}:{}", entity.revision + 1),
            created_at_ns: entity.created_at_ns,
            updated_at_ns: now_ns.max(entity.updated_at_ns),
        };
        Ok(Some((entity_id, entity.revision, draft)))
    }

    pub(crate) fn reject_phy_principal_entity_mention(
        &self,
        key: MemoryEntityMentionKey,
        _now_ns: i64,
    ) -> Result<Option<PrincipalResolutionPlan>, EntityResolverError> {
        if !mention_text(self.memories.routing_metadata(key.memory_id), key)
            .is_some_and(is_owner_relative_principal_surface)
        {
            return Ok(None);
        }
        if let Some(outcome) = terminal_outcome(key, self.entity_resolutions.get(key)) {
            return Ok(Some(PrincipalResolutionPlan {
                outcome,
                entity_draft: None,
                associate: false,
                expected_revision: 0,
                should_apply: false,
            }));
        }
        let expected_revision = self
            .entity_resolutions
            .get(key)
            .map_or(0, |value| value.revision);
        Ok(Some(PrincipalResolutionPlan {
            outcome: EntityResolutionOutcome {
                key,
                decision: EntityResolutionDecision::Reject,
                reason: EntityResolutionReason::PrincipalIdentity,
                entity_id: None,
                entity_created: false,
                association_changed: false,
                resolution_changed: false,
            },
            entity_draft: None,
            associate: false,
            expected_revision,
            should_apply: true,
        }))
    }

    fn principal_mention_matches(&self, key: MemoryEntityMentionKey) -> bool {
        mention_text(self.memories.routing_metadata(key.memory_id), key)
            .is_some_and(is_owner_relative_principal_surface)
    }
}

fn rel_source_principal(archive: &crate::Archive, memory: &crate::Memory) -> Option<String> {
    let episode = archive.episode(memory.source_episode_id?)?;
    let node_id = memory.source_node_id.as_deref()?;
    archive
        .nodes
        .get(&episode.conversation_id, node_id)
        .and_then(|node| node.principal_id.clone())
}

fn principal_profile_aliases(profile: &crate::PhylacteryProfile) -> Vec<String> {
    profile.display_name.iter().cloned().collect()
}

fn principal_profile_summary(profile: &crate::PhylacteryProfile) -> String {
    match profile.display_name.as_deref() {
        Some(display_name) => format!("Phylactery-backed user principal: {display_name}."),
        None => "Durable Phylactery-backed user principal.".into(),
    }
}

fn mention_text(
    metadata: Option<&crate::MemoryRoutingMetadata>,
    key: MemoryEntityMentionKey,
) -> Option<&str> {
    metadata?
        .entity_mentions
        .iter()
        .find(|mention| {
            mention.field == key.field
                && mention.start_byte == key.start_byte
                && mention.end_byte == key.end_byte
        })
        .map(|mention| mention.text.as_str())
}

fn terminal_outcome(
    key: MemoryEntityMentionKey,
    value: Option<&crate::MemoryEntityResolution>,
) -> Option<EntityResolutionOutcome> {
    let value = value?;
    match value.status {
        MemoryEntityResolutionStatus::Resolved { entity_id, reason } => {
            Some(EntityResolutionOutcome {
                key,
                decision: EntityResolutionDecision::ResolveExisting(entity_id),
                reason,
                entity_id: Some(entity_id),
                entity_created: false,
                association_changed: false,
                resolution_changed: false,
            })
        }
        MemoryEntityResolutionStatus::Rejected { reason } => Some(EntityResolutionOutcome {
            key,
            decision: EntityResolutionDecision::Reject,
            reason,
            entity_id: None,
            entity_created: false,
            association_changed: false,
            resolution_changed: false,
        }),
        MemoryEntityResolutionStatus::Pending(_) | MemoryEntityResolutionStatus::Dormant(_) => None,
    }
}
