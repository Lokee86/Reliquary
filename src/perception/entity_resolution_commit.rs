use crate::entity_resolution_owner::ResolutionWriteContext;
use crate::entity_resolution_processor_support::bootstrap_entity_draft;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::{
    Container, Entity, EntityDraft, EntityError, EntityId, EntityResolutionDecision,
    EntityResolutionEvaluation, EntityResolutionOutcome, EntityResolutionPrepared,
    EntityResolverError, MAX_ENTITY_ALIASES, MAX_ENTITY_RESOLUTION_CANDIDATES,
    MemoryEntityMentionKey, MemoryError,
};

pub(crate) struct EntityResolutionCommitContext<'a> {
    pub(crate) container: &'a mut Container,
    pub(crate) memories: &'a MemoryStore,
    pub(crate) entities: &'a mut EntityStore,
    pub(crate) graph: &'a mut GraphStore,
    pub(crate) resolutions: &'a mut EntityResolutionStore,
}

impl EntityResolutionCommitContext<'_> {
    fn owner_id(&self) -> Option<String> {
        self.container.owner_id()
    }
    fn memory_version(&self) -> u64 {
        self.memories.memory_version()
    }
    fn entity_version(&self) -> u64 {
        self.entities.entity_version()
    }
    fn graph_version(&self) -> u64 {
        self.graph.graph_version()
    }
    fn entity_resolution(
        &self,
        key: MemoryEntityMentionKey,
    ) -> Option<&crate::MemoryEntityResolution> {
        self.resolutions.get(key)
    }
    fn entity(&self, id: EntityId) -> Result<Entity, EntityError> {
        self.entities.entity(id)
    }
    fn publish_entity(
        &mut self,
        id: Option<EntityId>,
        rev: u64,
        draft: EntityDraft,
    ) -> Result<(Entity, bool), EntityError> {
        self.entities.publish(self.container, id, rev, draft)
    }
    fn set_entity_association(
        &mut self,
        memory_id: crate::MemoryId,
        entity_id: EntityId,
        active: bool,
        version: u64,
    ) -> Result<Option<crate::SemanticGraphRelation>, crate::GraphError> {
        let change =
            crate::SemanticGraphRelationChange::entity_association(memory_id, entity_id, active);
        crate::graph_endpoint_admission::validate_semantic_changes(
            self.memories,
            self.entities,
            &[change],
            crate::GraphRelationOrigin::Perception,
        )?;
        self.graph
            .set_entity_association(self.container, memory_id, entity_id, active, version)
    }
    fn entity_creation_conflicts(&self, surface: &str, kind: &str, limit: usize) -> Vec<Entity> {
        self.entities.creation_conflicts(surface, kind, limit)
    }
    fn put_entity_resolved(
        &mut self,
        key: MemoryEntityMentionKey,
        rev: u32,
        id: EntityId,
        reason: crate::EntityResolutionReason,
        now: i64,
    ) -> Result<bool, MemoryError> {
        let mut owner = ResolutionWriteContext {
            container: self.container,
            memories: self.memories,
            entities: self.entities,
            entity_resolutions: self.resolutions,
        };
        owner.put_entity_resolved(key, rev, id, reason, now)
    }
    fn put_entity_unresolved(
        &mut self,
        key: MemoryEntityMentionKey,
        rev: u32,
        ids: Vec<EntityId>,
        reason: crate::EntityResolutionReason,
        cf: [u8; 32],
        xf: [u8; 32],
        now: i64,
    ) -> Result<bool, MemoryError> {
        let mut owner = ResolutionWriteContext {
            container: self.container,
            memories: self.memories,
            entities: self.entities,
            entity_resolutions: self.resolutions,
        };
        owner.put_entity_unresolved(key, rev, ids, reason, cf, xf, now)
    }
    fn put_entity_rejected(
        &mut self,
        key: MemoryEntityMentionKey,
        rev: u32,
        reason: crate::EntityResolutionReason,
        now: i64,
    ) -> Result<bool, MemoryError> {
        let mut owner = ResolutionWriteContext {
            container: self.container,
            memories: self.memories,
            entities: self.entities,
            entity_resolutions: self.resolutions,
        };
        owner.put_entity_rejected(key, rev, reason, now)
    }
}

pub(crate) fn commit_entity_resolution(
    context: &mut EntityResolutionCommitContext<'_>,
    prepared: EntityResolutionPrepared,
    evaluation: EntityResolutionEvaluation,
    now_ns: i64,
) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
    if context.owner_id().as_deref() != Some(prepared.owner_id.as_str())
        || context.memory_version() != prepared.memory_version
        || context.entity_version() != prepared.entity_version
        || context.graph_version() != prepared.graph_version
        || context
            .entity_resolution(prepared.key)
            .map_or(0, |value| value.revision)
            != prepared.expected_resolution_revision
    {
        return Ok(None);
    }

    let key = prepared.key;
    let mut output = evaluation.output;
    if output.reason.is_rejection_class() {
        output.decision = EntityResolutionDecision::Reject;
    }
    let expected_revision = prepared.expected_resolution_revision;
    let outcome = match output.decision {
        EntityResolutionDecision::ResolveExisting(entity_id) => {
            let entity = context.entity(entity_id)?;
            if entity.kind == crate::entity_principal_identity::PRINCIPAL_ENTITY_KIND {
                return Err(EntityResolverError::InvalidOutput(
                    "principal Entities are reserved for deterministic principal resolution".into(),
                ));
            }
            let surface = prepared.candidates.mention.text.trim();
            let already_known = entity.canonical_name.eq_ignore_ascii_case(surface)
                || entity
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(surface));
            let alias_equivalent = prepared
                .candidates
                .candidates
                .iter()
                .find(|candidate| candidate.entity.id == entity_id)
                .is_some_and(|candidate| {
                    candidate.exact_surface
                        || candidate.normalized_surface
                        || candidate.alias_surface
                });
            if !surface.is_empty()
                && !already_known
                && alias_equivalent
                && entity.aliases.len() < MAX_ENTITY_ALIASES
            {
                let mut aliases = entity.aliases.clone();
                aliases.push(surface.to_owned());
                let draft = EntityDraft {
                    canonical_name: entity.canonical_name.clone(),
                    aliases,
                    kind: entity.kind.clone(),
                    summary: entity.summary.clone(),
                    mutation_id: format!(
                        "entity-resolution-alias:{}:{}",
                        hex32(&entity.id.0),
                        entity.revision + 1
                    ),
                    created_at_ns: entity.created_at_ns,
                    updated_at_ns: now_ns.max(entity.updated_at_ns),
                };
                context.publish_entity(Some(entity_id), entity.revision, draft)?;
            }
            let association_changed = context
                .set_entity_association(key.memory_id, entity_id, true, context.graph_version())?
                .is_some();
            let resolution_changed = context.put_entity_resolved(
                key,
                expected_revision,
                entity_id,
                output.reason,
                now_ns,
            )?;
            EntityResolutionOutcome {
                key,
                decision: output.decision,
                reason: output.reason,
                entity_id: Some(entity_id),
                entity_created: false,
                association_changed,
                resolution_changed,
            }
        }
        EntityResolutionDecision::CreateNew => {
            let materialization = evaluation.materialization.ok_or_else(|| {
                EntityResolverError::InvalidOutput(
                    "create_new evaluation missing Entity metadata".into(),
                )
            })?;
            if materialization.kind.trim()
                == crate::entity_principal_identity::PRINCIPAL_ENTITY_KIND
            {
                return Err(EntityResolverError::InvalidOutput(
                    "principal Entities are reserved for deterministic principal resolution".into(),
                ));
            }
            let conflicts = context.entity_creation_conflicts(
                prepared.candidates.mention.text.trim(),
                &materialization.kind,
                MAX_ENTITY_RESOLUTION_CANDIDATES,
            );
            if !conflicts.is_empty() {
                let mut candidate_entity_ids =
                    conflicts.iter().map(|entity| entity.id).collect::<Vec<_>>();
                for candidate in &prepared.candidates.candidates {
                    if candidate_entity_ids.len() >= MAX_ENTITY_RESOLUTION_CANDIDATES {
                        break;
                    }
                    if !candidate_entity_ids.contains(&candidate.entity.id) {
                        candidate_entity_ids.push(candidate.entity.id);
                    }
                }
                let resolution_changed = context.put_entity_unresolved(
                    key,
                    expected_revision,
                    candidate_entity_ids,
                    crate::EntityResolutionReason::Ambiguous,
                    prepared.candidate_fingerprint,
                    prepared.context_fingerprint,
                    now_ns,
                )?;
                return Ok(Some(EntityResolutionOutcome {
                    key,
                    decision: EntityResolutionDecision::Unresolved,
                    reason: crate::EntityResolutionReason::Ambiguous,
                    entity_id: None,
                    entity_created: false,
                    association_changed: false,
                    resolution_changed,
                }));
            }
            let draft =
                bootstrap_entity_draft(&prepared.memory, &prepared.candidates, materialization);
            let (entity, entity_created) = context.publish_entity(None, 0, draft)?;
            let association_changed = context
                .set_entity_association(key.memory_id, entity.id, true, context.graph_version())?
                .is_some();
            let resolution_changed = context.put_entity_resolved(
                key,
                expected_revision,
                entity.id,
                output.reason,
                now_ns,
            )?;
            EntityResolutionOutcome {
                key,
                decision: output.decision,
                reason: output.reason,
                entity_id: Some(entity.id),
                entity_created,
                association_changed,
                resolution_changed,
            }
        }
        EntityResolutionDecision::Unresolved => {
            let resolution_changed = context.put_entity_unresolved(
                key,
                expected_revision,
                prepared
                    .candidates
                    .candidates
                    .iter()
                    .map(|value| value.entity.id)
                    .collect(),
                output.reason,
                prepared.candidate_fingerprint,
                prepared.context_fingerprint,
                now_ns,
            )?;
            EntityResolutionOutcome {
                key,
                decision: output.decision,
                reason: output.reason,
                entity_id: None,
                entity_created: false,
                association_changed: false,
                resolution_changed,
            }
        }
        EntityResolutionDecision::Reject => {
            let resolution_changed =
                context.put_entity_rejected(key, expected_revision, output.reason, now_ns)?;
            EntityResolutionOutcome {
                key,
                decision: output.decision,
                reason: output.reason,
                entity_id: None,
                entity_created: false,
                association_changed: false,
                resolution_changed,
            }
        }
    };
    Ok(Some(outcome))
}

fn hex32(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}
