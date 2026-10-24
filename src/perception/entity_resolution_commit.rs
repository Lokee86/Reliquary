use crate::entity_principal_identity::PRINCIPAL_ENTITY_KIND;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::{
    Entity, EntityDraft, EntityId, EntityResolutionDecision, EntityResolutionEvaluation,
    EntityResolutionPrepared, EntityResolverError, MAX_ENTITY_ALIASES,
    MAX_ENTITY_RESOLUTION_CANDIDATES, MemoryEntityMentionKey,
};

pub(crate) struct EntityResolutionCommitContext<'a> {
    pub(crate) owner_id: Option<String>,
    pub(crate) memories: &'a MemoryStore,
    pub(crate) entities: &'a EntityStore,
    pub(crate) graph: &'a GraphStore,
    pub(crate) resolutions: &'a crate::entity_resolution_store::EntityResolutionStore,
}

#[derive(Debug)]
pub(crate) struct EntityResolutionCommitPlan {
    pub(crate) key: MemoryEntityMentionKey,
    pub(crate) decision: EntityResolutionDecision,
    pub(crate) reason: crate::EntityResolutionReason,
    pub(crate) operation: EntityResolutionCommitOperation,
}

#[derive(Debug)]
pub(crate) enum EntityResolutionCommitOperation {
    Existing {
        entity_id: EntityId,
        alias_update: Option<(u64, EntityDraft)>,
        expected_revision: u32,
        now_ns: i64,
    },
    Create {
        draft: EntityDraft,
        expected_revision: u32,
        now_ns: i64,
    },
    Unresolved {
        candidate_entity_ids: Vec<EntityId>,
        candidate_fingerprint: [u8; 32],
        context_fingerprint: [u8; 32],
        expected_revision: u32,
        now_ns: i64,
    },
    Rejected {
        expected_revision: u32,
        now_ns: i64,
    },
}

impl EntityResolutionCommitContext<'_> {
    fn memory_version(&self) -> u64 { self.memories.memory_version() }
    fn entity_version(&self) -> u64 { self.entities.entity_version() }
    fn graph_version(&self) -> u64 { self.graph.graph_version() }
    fn entity_resolution(&self, key: MemoryEntityMentionKey) -> Option<&crate::MemoryEntityResolution> {
        self.resolutions.get(key)
    }
    fn entity(&self, id: EntityId) -> Result<Entity, crate::EntityError> { self.entities.entity(id) }
    fn entity_creation_conflicts(&self, surface: &str, kind: &str, limit: usize) -> Vec<Entity> {
        self.entities.creation_conflicts(surface, kind, limit)
    }
}

pub(crate) fn plan_entity_resolution_commit(
    context: &EntityResolutionCommitContext<'_>,
    prepared: EntityResolutionPrepared,
    evaluation: EntityResolutionEvaluation,
    now_ns: i64,
) -> Result<Option<EntityResolutionCommitPlan>, EntityResolverError> {
    if context.owner_id.as_deref() != Some(prepared.owner_id.as_str())
        || context.memory_version() != prepared.memory_version
        || context.entity_version() != prepared.entity_version
        || context.graph_version() != prepared.graph_version
        || context.entity_resolution(prepared.key).map_or(0, |value| value.revision)
            != prepared.expected_resolution_revision
    {
        return Ok(None);
    }

    let key = prepared.key;
    let mut output = evaluation.output;
    if output.reason.is_rejection_class() { output.decision = EntityResolutionDecision::Reject; }
    let expected_revision = prepared.expected_resolution_revision;
    let (decision, operation) = match output.decision {
        EntityResolutionDecision::ResolveExisting(entity_id) => {
            let entity = context.entity(entity_id)?;
            if entity.kind == PRINCIPAL_ENTITY_KIND {
                return Err(EntityResolverError::InvalidOutput(
                    "principal Entities are reserved for deterministic principal resolution".into(),
                ));
            }
            let surface = prepared.candidates.mention.text.trim();
            let already_known = entity.canonical_name.eq_ignore_ascii_case(surface)
                || entity.aliases.iter().any(|alias| alias.eq_ignore_ascii_case(surface));
            let alias_equivalent = prepared.candidates.candidates.iter()
                .find(|candidate| candidate.entity.id == entity_id)
                .is_some_and(|candidate| candidate.exact_surface || candidate.normalized_surface || candidate.alias_surface);
            let alias_update = if !surface.is_empty() && !already_known && alias_equivalent
                && entity.aliases.len() < MAX_ENTITY_ALIASES
            {
                let mut aliases = entity.aliases.clone();
                aliases.push(surface.to_owned());
                Some((entity.revision, EntityDraft {
                    canonical_name: entity.canonical_name.clone(), aliases, kind: entity.kind.clone(),
                    summary: entity.summary.clone(),
                    mutation_id: format!("entity-resolution-alias:{}:{}", hex32(&entity.id.0), entity.revision + 1),
                    created_at_ns: entity.created_at_ns,
                    updated_at_ns: now_ns.max(entity.updated_at_ns),
                }))
            } else { None };
            (output.decision, EntityResolutionCommitOperation::Existing {
                entity_id, alias_update, expected_revision, now_ns,
            })
        }
        EntityResolutionDecision::CreateNew => {
            let materialization = evaluation.materialization.ok_or_else(|| EntityResolverError::InvalidOutput(
                "create_new evaluation missing Entity metadata".into(),
            ))?;
            if materialization.kind.trim() == PRINCIPAL_ENTITY_KIND {
                return Err(EntityResolverError::InvalidOutput(
                    "principal Entities are reserved for deterministic principal resolution".into(),
                ));
            }
            let conflicts = context.entity_creation_conflicts(
                prepared.candidates.mention.text.trim(), &materialization.kind,
                MAX_ENTITY_RESOLUTION_CANDIDATES,
            );
            if !conflicts.is_empty() {
                let mut ids = conflicts.iter().map(|entity| entity.id).collect::<Vec<_>>();
                for candidate in &prepared.candidates.candidates {
                    if ids.len() >= MAX_ENTITY_RESOLUTION_CANDIDATES { break; }
                    if !ids.contains(&candidate.entity.id) { ids.push(candidate.entity.id); }
                }
                (EntityResolutionDecision::Unresolved, EntityResolutionCommitOperation::Unresolved {
                    candidate_entity_ids: ids,
                    candidate_fingerprint: prepared.candidate_fingerprint,
                    context_fingerprint: prepared.context_fingerprint,
                    expected_revision, now_ns,
                })
            } else {
                let draft = crate::entity_resolution_processor_support::bootstrap_entity_draft(
                    &prepared.memory, &prepared.candidates, materialization,
                );
                (output.decision, EntityResolutionCommitOperation::Create { draft, expected_revision, now_ns })
            }
        }
        EntityResolutionDecision::Unresolved => (output.decision, EntityResolutionCommitOperation::Unresolved {
            candidate_entity_ids: prepared.candidates.candidates.iter().map(|value| value.entity.id).collect(),
            candidate_fingerprint: prepared.candidate_fingerprint,
            context_fingerprint: prepared.context_fingerprint,
            expected_revision, now_ns,
        }),
        EntityResolutionDecision::Reject => (output.decision, EntityResolutionCommitOperation::Rejected { expected_revision, now_ns }),
    };
    Ok(Some(EntityResolutionCommitPlan { key, decision, reason: output.reason, operation }))
}

fn hex32(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes { use std::fmt::Write; let _ = write!(out, "{byte:02x}"); }
    out
}
