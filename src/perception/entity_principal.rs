use crate::{
    Cva, EntityDraft, EntityGlobalId, EntityId, EntityResolutionDecision, EntityResolutionOutcome,
    EntityResolutionReason, EntityResolverError, Memory, MemoryEntityMentionKey,
    MemoryEntityResolutionStatus, Phylactery,
};
use sha2::{Digest, Sha256};

pub(crate) const PRINCIPAL_ENTITY_KIND: &str = "principal";

pub(crate) fn principal_entity_id(principal_id: &str) -> EntityId {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-principal-entity-v1\0");
    hash.update((principal_id.len() as u64).to_le_bytes());
    hash.update(principal_id.as_bytes());
    EntityId(hash.finalize().into())
}

pub(crate) fn principal_entity_global_id(principal_id: &str) -> EntityGlobalId {
    let uuid = uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        format!("reliquary-principal-entity:{principal_id}").as_bytes(),
    );
    EntityGlobalId(*uuid.as_bytes())
}

pub(crate) fn is_owner_relative_principal_surface(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    let base = normalized
        .strip_suffix("'s")
        .or_else(|| normalized.strip_suffix("’s"))
        .unwrap_or(&normalized);
    matches!(base, "user" | "the user")
}

impl Cva {
    pub(crate) fn resolve_principal_entity_mention(
        &mut self,
        key: MemoryEntityMentionKey,
        now_ns: i64,
    ) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
        if !self.principal_mention_matches(key) {
            return Ok(None);
        }
        if let Some(outcome) = terminal_outcome(key, self.entity_resolution(key)) {
            return Ok(Some(outcome));
        }

        let memory = self.memory(key.memory_id)?;
        let Some(principal_id) = rel_source_principal(self, &memory) else {
            return Ok(None);
        };
        if !is_phy_principal_id(&principal_id) {
            return Ok(None);
        }

        let entity_id = principal_entity_id(&principal_id);
        let entity_created = if self.entities.contains(entity_id) {
            let entity = self.entity(entity_id)?;
            if entity.kind != PRINCIPAL_ENTITY_KIND || entity.canonical_name != principal_id {
                return Err(crate::EntityError::InvalidField("principal Entity identity").into());
            }
            false
        } else {
            self.publish_entity(
                Some(entity_id),
                0,
                EntityDraft {
                    canonical_name: principal_id.clone(),
                    aliases: Vec::new(),
                    kind: PRINCIPAL_ENTITY_KIND.into(),
                    summary: "Durable Phylactery-backed user principal.".into(),
                    mutation_id: format!("principal-entity:{principal_id}"),
                    created_at_ns: 0,
                    updated_at_ns: 0,
                },
            )?;
            true
        };

        let association_changed = self
            .set_principal_association(key.memory_id, entity_id, true, self.graph_version())?
            .is_some();
        let expected_revision = self
            .entity_resolution(key)
            .map_or(0, |value| value.revision);
        let resolution_changed = self.put_entity_resolved(
            key,
            expected_revision,
            entity_id,
            EntityResolutionReason::PrincipalIdentity,
            now_ns,
        )?;

        Ok(Some(EntityResolutionOutcome {
            key,
            decision: if entity_created {
                EntityResolutionDecision::CreateNew
            } else {
                EntityResolutionDecision::ResolveExisting(entity_id)
            },
            reason: EntityResolutionReason::PrincipalIdentity,
            entity_id: Some(entity_id),
            entity_created,
            association_changed,
            resolution_changed,
        }))
    }

    pub(crate) fn sync_principal_profile(
        &mut self,
        principal_id: &str,
        profile: &crate::PhylacteryProfile,
        now_ns: i64,
    ) -> Result<bool, crate::EntityError> {
        if !is_phy_principal_id(principal_id) {
            return Err(crate::EntityError::InvalidField(
                "principal Entity identity",
            ));
        }
        let entity_id = principal_entity_id(principal_id);
        let Ok(entity) = self.entity(entity_id) else {
            return Ok(false);
        };
        if entity.kind != PRINCIPAL_ENTITY_KIND || entity.canonical_name != principal_id {
            return Err(crate::EntityError::InvalidField(
                "principal Entity identity",
            ));
        }

        let aliases = principal_profile_aliases(profile);
        let summary = principal_profile_summary(profile);
        if entity.aliases == aliases && entity.summary == summary {
            return Ok(false);
        }
        self.publish_entity(
            Some(entity_id),
            entity.revision,
            EntityDraft {
                canonical_name: entity.canonical_name,
                aliases,
                kind: entity.kind,
                summary,
                mutation_id: format!("principal-profile:{principal_id}:{}", entity.revision + 1),
                created_at_ns: entity.created_at_ns,
                updated_at_ns: now_ns.max(entity.updated_at_ns),
            },
        )
        .map(|(_, changed)| changed)
    }

    fn principal_mention_matches(&self, key: MemoryEntityMentionKey) -> bool {
        mention_text(self.memory_routing_metadata(key.memory_id), key)
            .is_some_and(is_owner_relative_principal_surface)
    }
}

impl Phylactery {
    pub(crate) fn resolve_principal_entity_mention(
        &mut self,
        key: MemoryEntityMentionKey,
        now_ns: i64,
    ) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
        if !mention_text(self.memory_routing_metadata(key.memory_id), key)
            .is_some_and(is_owner_relative_principal_surface)
        {
            return Ok(None);
        }
        if let Some(outcome) = terminal_outcome(key, self.entity_resolution(key)) {
            return Ok(Some(outcome));
        }

        // A PHY is already the durable identity boundary for its owner. Owner-relative
        // surfaces must never materialize a self-Entity inside that same PHY, regardless of
        // whether the Memory was routed from a REL or authored directly in the PHY.
        let expected_revision = self
            .entity_resolution(key)
            .map_or(0, |value| value.revision);
        let resolution_changed = self.put_entity_rejected(
            key,
            expected_revision,
            EntityResolutionReason::PrincipalIdentity,
            now_ns,
        )?;
        Ok(Some(EntityResolutionOutcome {
            key,
            decision: EntityResolutionDecision::Reject,
            reason: EntityResolutionReason::PrincipalIdentity,
            entity_id: None,
            entity_created: false,
            association_changed: false,
            resolution_changed,
        }))
    }
}

fn rel_source_principal(cva: &Cva, memory: &Memory) -> Option<String> {
    let episode = cva.episode(memory.source_episode_id?)?;
    let node_id = memory.source_node_id.as_deref()?;
    cva.archive
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

pub(crate) fn is_phy_principal_id(value: &str) -> bool {
    value
        .strip_prefix("phy-")
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .is_some()
}
