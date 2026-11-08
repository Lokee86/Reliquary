//! Owner-scoped Entity-resolution proposal preparation.

use reliquary_entity_resolution::{
    EntityCandidate, EntityModelEvaluator, MentionSpan, ResolutionOutcome, TextField,
    rank_candidates,
};
use reliquary_owner_session::model::{CompatibilityProfile, CompatibilityProfileId};
use reliquary_owner_session::model::{
    Entity, EntityId, MAX_MENTION_CANDIDATES, MemoryEntityMentionKey, MemoryEntityResolution,
    MemoryTextField,
};
use reliquary_owner_session::model::{
    MemoryBody, MemoryBodyId, MemoryId, NodeRef, PayloadState, Relation, RelationKind,
};
use reliquary_owner_session::{OwnerSessionError, PhySession, RelSession};
use reliquary_switchboard::GeneralEndpoint;
use std::fmt;

/// An immutable owner-qualified Memory identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityMemoryMention {
    pub owner_id: [u8; 16],
    pub memory_id: MemoryId,
    pub field: TextField,
    pub start_byte: usize,
    pub end_byte: usize,
    pub text: String,
}

/// Select exactly one opened owner; candidate and body reads stay inside it.
pub enum EntityOwnerSession<'a> {
    Rel(&'a RelSession),
    Phy(&'a PhySession),
    MountedPhy(&'a reliquary_owner_session::PhyWriter),
}

impl EntityOwnerSession<'_> {
    pub(crate) fn owner_id(&self) -> [u8; 16] {
        match self {
            Self::Rel(session) => session.owner_id(),
            Self::Phy(session) => session.owner_id(),
            Self::MountedPhy(session) => session.owner_id(),
        }
    }

    pub(crate) fn current_memory(
        &self,
        id: MemoryId,
    ) -> Result<reliquary_owner_session::model::MemoryRevision, OwnerSessionError> {
        match self {
            Self::Rel(s) => s.current_memory(id),
            Self::Phy(s) => s.current_memory(id),
            Self::MountedPhy(s) => s.current_memory(id),
        }
    }
    pub(crate) fn dream_candidate_pool(
        &self,
        id: MemoryId,
        profile: CompatibilityProfileId,
        row: &[f32],
    ) -> Result<reliquary_owner_session::DreamCandidatePool, OwnerSessionError> {
        match self {
            Self::Rel(s) => s.dream_candidate_pool(id, profile, row),
            Self::Phy(s) => s.dream_candidate_pool(id, profile, row),
            Self::MountedPhy(s) => s.dream_candidate_pool(id, profile, row),
        }
    }
    pub(crate) fn memory_history(
        &self,
        memory_id: MemoryId,
    ) -> Result<Option<reliquary_owner_session::model::MemoryHistory>, OwnerSessionError> {
        match self {
            Self::Rel(session) => session.memory_history(memory_id),
            Self::Phy(session) => session.memory_history(memory_id),
            Self::MountedPhy(session) => session.memory_history(memory_id),
        }
    }

    pub(crate) fn memory_history_at_cut(
        &self,
        memory_id: MemoryId,
        memory_version: u64,
    ) -> Result<Option<reliquary_owner_session::model::MemoryHistory>, OwnerSessionError> {
        match self {
            Self::Rel(session) => session.memory_history_at_cut(memory_id, memory_version),
            Self::Phy(session) => session.memory_history_at_cut(memory_id, memory_version),
            Self::MountedPhy(session) => session.memory_history_at_cut(memory_id, memory_version),
        }
    }

    pub(crate) fn memory_version(&self) -> u64 {
        match self {
            Self::Rel(session) => session.memory_version(),
            Self::Phy(session) => session.memory_version(),
            Self::MountedPhy(session) => session.memory_version(),
        }
    }

    pub(crate) fn dream_duplicate_context(
        &self,
        seeds: &[MemoryId],
    ) -> Result<
        (
            Vec<reliquary_owner_session::model::MemoryRecord>,
            Vec<Relation>,
        ),
        OwnerSessionError,
    > {
        match self {
            Self::Rel(s) => s.dream_duplicate_context(seeds),
            Self::Phy(s) => s.dream_duplicate_context(seeds),
            Self::MountedPhy(s) => s.dream_duplicate_context(seeds),
        }
    }
    pub(crate) fn memory_ids_page(
        &self,
        after: Option<MemoryId>,
        limit: usize,
    ) -> Result<(u64, Vec<MemoryId>), OwnerSessionError> {
        match self {
            Self::Rel(session) => session.memory_ids_page(after, limit),
            Self::Phy(session) => session.memory_ids_page(after, limit),
            Self::MountedPhy(session) => session.memory_ids_page(after, limit),
        }
    }

    pub(crate) fn memory_vector_row(
        &self,
        profile: CompatibilityProfileId,
        body: MemoryBodyId,
    ) -> Result<Option<Vec<f32>>, OwnerSessionError> {
        match self {
            Self::Rel(session) => session.memory_vector_row(profile, body),
            Self::Phy(session) => session.memory_vector_row(profile, body),
            Self::MountedPhy(session) => session.memory_vector_row(profile, body),
        }
    }

    pub(crate) fn graph_relation(
        &self,
        source: NodeRef,
        target: NodeRef,
        kind: RelationKind,
    ) -> Option<Relation> {
        match self {
            Self::Rel(session) => session.graph_relation(source, target, kind).cloned(),
            Self::Phy(session) => session.graph_relation(source, target, kind).cloned(),
            Self::MountedPhy(session) => session.graph_relation(source, target, kind).cloned(),
        }
    }

    pub(crate) fn graph_version(&self) -> u64 {
        match self {
            Self::Rel(session) => session.graph_version(),
            Self::Phy(session) => session.graph_version(),
            Self::MountedPhy(session) => session.graph_version(),
        }
    }

    pub(crate) fn dream_pair_evaluated(&self, left: MemoryId, right: MemoryId) -> bool {
        match self {
            Self::Rel(session) => session.dream_pair_evaluated(left, right),
            Self::Phy(session) => session.dream_pair_evaluated(left, right),
            Self::MountedPhy(session) => session.dream_pair_evaluated(left, right),
        }
    }

    pub(crate) fn active_compatibility_profile(
        &self,
        id: CompatibilityProfileId,
    ) -> Option<CompatibilityProfile> {
        match self {
            Self::Rel(session) => session.active_compatibility_profile(id).cloned(),
            Self::Phy(session) => session.active_compatibility_profile(id).cloned(),
            Self::MountedPhy(session) => session.active_compatibility_profile(id).cloned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntityWorkflowError {
    OwnerMismatch,
    MemoryUnavailable,
    MemoryTombstoned,
    MemoryBodyUnavailable,
    InvalidMention(&'static str),
    CandidateLimit,
    OwnerRead(String),
    Evaluator(String),
}

impl fmt::Display for EntityWorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnerMismatch => f.write_str("Memory reference belongs to another owner"),
            Self::MemoryUnavailable => f.write_str("Memory is unavailable in this owner"),
            Self::MemoryTombstoned => f.write_str("Memory is terminally tombstoned"),
            Self::MemoryBodyUnavailable => f.write_str("Memory body is not available"),
            Self::InvalidMention(reason) => write!(f, "invalid Memory mention: {reason}"),
            Self::CandidateLimit => f.write_str("too many same-owner Entity candidates"),
            Self::OwnerRead(reason) => write!(f, "owner read failed: {reason}"),
            Self::Evaluator(reason) => write!(f, "Entity evaluator failed: {reason}"),
        }
    }
}

impl std::error::Error for EntityWorkflowError {}

impl From<OwnerSessionError> for EntityWorkflowError {
    fn from(error: OwnerSessionError) -> Self {
        Self::OwnerRead(error.to_string())
    }
}

/// Source-bound evaluator result. This is a proposal only; the owning
/// Perception store remains unchanged.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityResolutionProposal {
    owner_id: [u8; 16],
    memory_id: MemoryId,
    memory_revision: u64,
    body_id: MemoryBodyId,
    mention: MentionSpan,
    candidates: Vec<EntityCandidate>,
    existing_resolution: Option<MemoryEntityResolution>,
    outcome: ResolutionOutcome,
    admission: Option<reliquary_entity_resolution::EntityAdmissionMaterialization>,
}

impl EntityResolutionProposal {
    pub fn outcome(&self) -> ResolutionOutcome {
        self.outcome
    }

    /// Prepare the exact owner command from an evaluated proposal.
    pub fn into_mention_commit(
        self,
        updated_at_ns: i64,
    ) -> Result<reliquary_owner_session::EntityMentionCommit, EntityWorkflowError> {
        use reliquary_entity_resolution::ResolutionDecision;
        use reliquary_owner_session::model::{
            EntityResolutionReason, MemoryEntityResolutionStatus,
        };

        let status = match self.outcome.decision {
            ResolutionDecision::ResolveExisting(entity_id) => {
                MemoryEntityResolutionStatus::Resolved {
                    entity_id,
                    reason: self.outcome.reason,
                }
            }
            ResolutionDecision::Unresolved
                if self.outcome.reason == EntityResolutionReason::Ambiguous =>
            {
                let mut candidates: Vec<_> = self
                    .candidates
                    .iter()
                    .map(|candidate| candidate.entity.id)
                    .collect();
                candidates.sort_unstable();
                candidates.dedup();
                if candidates.len() >= 2 {
                    MemoryEntityResolutionStatus::Ambiguous { candidates }
                } else {
                    MemoryEntityResolutionStatus::Unresolved {
                        reason: self.outcome.reason,
                    }
                }
            }
            ResolutionDecision::Unresolved | ResolutionDecision::Reject => {
                MemoryEntityResolutionStatus::Unresolved {
                    reason: self.outcome.reason,
                }
            }
            ResolutionDecision::CreateNew => MemoryEntityResolutionStatus::Unresolved {
                reason: self.outcome.reason,
            },
        };
        let targets = match &status {
            MemoryEntityResolutionStatus::Resolved { entity_id, .. } => {
                std::slice::from_ref(entity_id)
            }
            MemoryEntityResolutionStatus::Ambiguous { candidates } => candidates,
            MemoryEntityResolutionStatus::Unresolved { .. } => &[],
        };
        let target_revisions = targets
            .iter()
            .map(|id| {
                self.candidates
                    .iter()
                    .find(|candidate| candidate.entity.id == *id)
                    .map(|candidate| (*id, candidate.entity.revision))
                    .ok_or(EntityWorkflowError::InvalidMention(
                        "selected Entity was not evaluated",
                    ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(reliquary_owner_session::EntityMentionCommit {
            owner_id: self.owner_id,
            memory_revision: self.memory_revision,
            body_id: self.body_id,
            key: mention_key(self.memory_id, &self.mention)?,
            surface: self.mention.text,
            expected_revision: self
                .existing_resolution
                .as_ref()
                .map_or(0, |value| value.revision),
            updated_at_ns,
            status,
            target_revisions,
            new_entity: self
                .admission
                .map(|admission| (admission.kind, admission.summary)),
        })
    }
}

/// An owned, same-owner input captured before calling a model.
pub struct PreparedEntityMention {
    owner_id: [u8; 16],
    memory_id: MemoryId,
    memory_revision: u64,
    body_id: MemoryBodyId,
    body: MemoryBody,
    mention: MentionSpan,
    candidates: Vec<EntityCandidate>,
    existing_resolution: Option<MemoryEntityResolution>,
}

impl PreparedEntityMention {
    pub fn existing_resolution(&self) -> Option<&MemoryEntityResolution> {
        self.existing_resolution.as_ref()
    }

    pub fn evaluate_with_endpoint<E: GeneralEndpoint>(
        self,
        endpoint: E,
    ) -> Result<EntityResolutionProposal, EntityWorkflowError> {
        self.evaluate(&EntityModelEvaluator::new(endpoint))
    }

    pub fn evaluate<E: GeneralEndpoint>(
        self,
        evaluator: &EntityModelEvaluator<E>,
    ) -> Result<EntityResolutionProposal, EntityWorkflowError> {
        let mut outcome = evaluator
            .resolve(&self.body, &self.mention, &self.candidates)
            .map_err(|error| EntityWorkflowError::Evaluator(error.to_string()))?;
        let admission =
            if outcome.decision == reliquary_entity_resolution::ResolutionDecision::CreateNew {
                let admission = evaluator
                    .admit(&self.body, &self.mention, &[])
                    .map_err(|error| EntityWorkflowError::Evaluator(error.to_string()))?;
                outcome.reason = admission.reason;
                if admission.materialization.is_none() {
                    outcome.decision = match admission.decision {
                        reliquary_entity_resolution::AdmissionDecision::Reject => {
                            reliquary_entity_resolution::ResolutionDecision::Reject
                        }
                        _ => reliquary_entity_resolution::ResolutionDecision::Unresolved,
                    };
                }
                admission.materialization
            } else {
                None
            };
        Ok(EntityResolutionProposal {
            owner_id: self.owner_id,
            memory_id: self.memory_id,
            memory_revision: self.memory_revision,
            body_id: self.body_id,
            mention: self.mention,
            candidates: self.candidates,
            existing_resolution: self.existing_resolution,
            outcome,
            admission,
        })
    }
}

/// Resolve one exact Memory span using only that owner's latest body and Entity
/// records. The endpoint receives no cross-owner context; authorization to
/// obtain the session remains the caller's responsibility.
pub fn resolve_memory_mention<E: GeneralEndpoint>(
    evaluator: &EntityModelEvaluator<E>,
    owner: EntityOwnerSession<'_>,
    input: EntityMemoryMention,
) -> Result<EntityResolutionProposal, EntityWorkflowError> {
    prepare_memory_mention(owner, input)?.evaluate(evaluator)
}

/// Read and validate owner-local facts without calling the evaluator.
pub fn prepare_memory_mention(
    owner: EntityOwnerSession<'_>,
    input: EntityMemoryMention,
) -> Result<PreparedEntityMention, EntityWorkflowError> {
    let owner_id = owner.owner_id();
    if owner_id != input.owner_id {
        return Err(EntityWorkflowError::OwnerMismatch);
    }
    prepare_mention_with(input, |key| match owner {
        EntityOwnerSession::Rel(session) => session.entity_mention_inputs(key),
        EntityOwnerSession::Phy(session) => session.entity_mention_inputs(key),
        EntityOwnerSession::MountedPhy(writer) => writer.entity_mention_inputs(key),
    })
}

pub fn prepare_phy_writer_mention(
    owner: &reliquary_owner_session::PhyWriter,
    input: EntityMemoryMention,
) -> Result<PreparedEntityMention, EntityWorkflowError> {
    if owner.owner_id() != input.owner_id {
        return Err(EntityWorkflowError::OwnerMismatch);
    }
    prepare_mention_with(input, |key| owner.entity_mention_inputs(key))
}

fn prepare_mention_with(
    input: EntityMemoryMention,
    read: impl FnOnce(
        MemoryEntityMentionKey,
    ) -> Result<reliquary_owner_session::EntityMentionInputs, OwnerSessionError>,
) -> Result<PreparedEntityMention, EntityWorkflowError> {
    use reliquary_owner_session::model::MemoryError;
    let mention = MentionSpan {
        field: input.field,
        start_byte: input.start_byte,
        end_byte: input.end_byte,
        text: input.text,
    };
    let key = mention_key(input.memory_id, &mention)?;
    let reliquary_owner_session::EntityMentionInputs {
        revision,
        entities,
        existing: existing_resolution,
        source_entities,
        supporting_memories,
    } = read(key).map_err(|error| match error {
        OwnerSessionError::Memory(MemoryError::MissingMemory) => {
            EntityWorkflowError::MemoryUnavailable
        }
        OwnerSessionError::Memory(MemoryError::TerminalTombstone) => {
            EntityWorkflowError::MemoryTombstoned
        }
        error => error.into(),
    })?;
    let PayloadState::Available(body) = revision.payload else {
        return Err(EntityWorkflowError::MemoryBodyUnavailable);
    };
    validate_span(&body, &mention)?;
    let mut candidates = build_candidates(&entities, &mention)?;
    merge_owner_evidence(
        &mut candidates,
        &entities,
        &body,
        &mention,
        &source_entities,
        &supporting_memories,
    );
    Ok(PreparedEntityMention {
        owner_id: input.owner_id,
        memory_id: input.memory_id,
        memory_revision: revision.record.revision,
        body_id: revision.record.body_id,
        mention,
        candidates,
        existing_resolution,
        body,
    })
}

pub(crate) fn mention_key(
    memory_id: MemoryId,
    mention: &MentionSpan,
) -> Result<MemoryEntityMentionKey, EntityWorkflowError> {
    Ok(MemoryEntityMentionKey {
        memory_id: memory_id.0,
        field: match mention.field {
            TextField::Title => MemoryTextField::Title,
            TextField::Content => MemoryTextField::Content,
        },
        start_byte: u32::try_from(mention.start_byte)
            .map_err(|_| EntityWorkflowError::InvalidMention("offset exceeds stored range"))?,
        end_byte: u32::try_from(mention.end_byte)
            .map_err(|_| EntityWorkflowError::InvalidMention("offset exceeds stored range"))?,
    })
}

fn validate_span(body: &MemoryBody, mention: &MentionSpan) -> Result<(), EntityWorkflowError> {
    let source = match mention.field {
        TextField::Title => &body.title,
        TextField::Content => &body.content,
    };
    if mention.start_byte >= mention.end_byte || mention.end_byte > source.len() {
        return Err(EntityWorkflowError::InvalidMention("span is out of bounds"));
    }
    let exact = source
        .get(mention.start_byte..mention.end_byte)
        .ok_or(EntityWorkflowError::InvalidMention("span splits UTF-8"))?;
    if exact != mention.text || exact.trim().is_empty() {
        return Err(EntityWorkflowError::InvalidMention(
            "surface does not match body",
        ));
    }
    Ok(())
}

fn build_candidates(
    entities: &[Entity],
    mention: &MentionSpan,
) -> Result<Vec<EntityCandidate>, EntityWorkflowError> {
    let surface = &mention.text;
    let normalized = normalize(surface);
    let mut candidates = entities
        .iter()
        .filter(|entity| entity.kind != "principal")
        .filter_map(|entity| {
            let canonical_exact = entity.canonical_name == *surface;
            let canonical_normalized = normalize(&entity.canonical_name) == normalized;
            let alias_exact = entity.aliases.iter().any(|alias| alias == surface);
            let alias_normalized = entity
                .aliases
                .iter()
                .any(|alias| normalize(alias) == normalized);
            let exact_surface = canonical_exact || alias_exact;
            let normalized_surface = canonical_normalized || alias_normalized;
            (exact_surface || normalized_surface).then(|| EntityCandidate {
                entity: entity.clone(),
                exact_surface,
                normalized_surface,
                alias_surface: alias_exact || alias_normalized,
                source_association: false,
                lexical_memory_hits: 0,
                graph_neighbor_hits: 0,
                best_lexical_score: 0.0,
                supporting_memory_ids: vec![],
            })
        })
        .collect::<Vec<_>>();
    rank_candidates(&mut candidates);
    if candidates.len() > MAX_MENTION_CANDIDATES {
        return Err(EntityWorkflowError::CandidateLimit);
    }
    Ok(candidates)
}

fn merge_owner_evidence(
    candidates: &mut Vec<EntityCandidate>,
    entities: &[Entity],
    body: &MemoryBody,
    mention: &MentionSpan,
    source_entities: &[EntityId],
    support: &[reliquary_owner_session::EntityCandidateMemory],
) {
    const LEXICAL_HIT_LIMIT: usize = 16;
    const SUPPORT_LIMIT: usize = 8;

    let mut terms = reliquary_retrieval::lexical_terms(&format!(
        "{} {} {}",
        mention.text, body.title, body.content
    ));
    terms.truncate(32);
    let mut lexical_hits: Vec<_> = support
        .iter()
        .filter_map(|memory| {
            let score = reliquary_retrieval::lexical_score(memory.lexical_text.as_deref()?, &terms);
            (score > 0.0).then_some((memory.id, score))
        })
        .collect();
    lexical_hits.sort_by(|(a_id, a), (b_id, b)| b.total_cmp(a).then_with(|| a_id.cmp(b_id)));
    lexical_hits.truncate(LEXICAL_HIT_LIMIT);

    let mut by_id: std::collections::BTreeMap<_, _> = candidates
        .drain(..)
        .map(|candidate| (candidate.entity.id, candidate))
        .collect();
    let entities: std::collections::BTreeMap<_, _> = entities
        .iter()
        .filter(|entity| entity.kind != "principal")
        .map(|entity| (entity.id, entity))
        .collect();
    let mut apply = |id, source, graph, lexical: Option<f64>, memory: Option<MemoryId>| {
        let Some(entity) = entities.get(&id) else {
            return;
        };
        let candidate = by_id.entry(id).or_insert_with(|| EntityCandidate {
            entity: (*entity).clone(),
            exact_surface: false,
            normalized_surface: false,
            alias_surface: false,
            source_association: false,
            lexical_memory_hits: 0,
            graph_neighbor_hits: 0,
            best_lexical_score: 0.0,
            supporting_memory_ids: vec![],
        });
        candidate.source_association |= source;
        candidate.graph_neighbor_hits += usize::from(graph);
        if let Some(score) = lexical {
            candidate.lexical_memory_hits += 1;
            candidate.best_lexical_score = candidate.best_lexical_score.max(score);
        }
        if let Some(memory) = memory
            && candidate.supporting_memory_ids.len() < SUPPORT_LIMIT
            && !candidate.supporting_memory_ids.contains(&memory)
        {
            candidate.supporting_memory_ids.push(memory);
        }
    };
    for id in source_entities {
        apply(*id, true, false, None, None);
    }
    for memory in support {
        let lexical = lexical_hits
            .iter()
            .find_map(|(id, score)| (*id == memory.id).then_some(*score));
        if lexical.is_some() || memory.graph_neighbor {
            for id in &memory.entities {
                apply(*id, false, memory.graph_neighbor, lexical, Some(memory.id));
            }
        }
    }
    candidates.extend(by_id.into_values());
    rank_candidates(candidates);
    candidates.truncate(MAX_MENTION_CANDIDATES);
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::{Container, ContainerIdentity};
    use reliquary_memory_core::{MemoryDraft, MemoryStore};
    use reliquary_owner_session::GraphAdmission;
    use reliquary_perception_store::{EntityDraft, PerceptionStore};
    use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
    use reliquary_vector_store::{CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore};
    use serde_json::{Value, json};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        sync::{Arc, Mutex},
    };

    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
            fs::create_dir_all(&root).unwrap();
            Self(root.join(format!(
                "entity-workflow-{}-{}.phy",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[derive(Clone, Default)]
    struct RecordingEndpoint(Arc<Mutex<Vec<String>>>);
    impl GeneralEndpoint for RecordingEndpoint {
        fn model(&self) -> &str {
            "test-entity-model"
        }
        fn complete_json(
            &self,
            _system_prompt: &str,
            user_payload: &str,
            _schema_name: &str,
            _schema: &Value,
        ) -> Result<Value, GeneralEndpointError> {
            self.0.lock().unwrap().push(user_payload.to_owned());
            Ok(json!({
                "decision":"resolve_existing",
                "reason":"context_match",
                "target_candidate_index":0,
                "identity_relation":"same_identity",
                "mention_kind":"tool"
            }))
        }
    }

    struct AdmissionEndpoint;
    impl GeneralEndpoint for AdmissionEndpoint {
        fn model(&self) -> &str {
            "entity-admission-fixture"
        }
        fn complete_json(
            &self,
            _prompt: &str,
            _payload: &str,
            schema: &str,
            _shape: &Value,
        ) -> Result<Value, GeneralEndpointError> {
            Ok(match schema {
                "entity_resolution" => json!({
                    "decision":"create_new", "reason":"named_referent", "target_candidate_index":-1,
                    "identity_relation":"related_distinct", "mention_kind":"tool"
                }),
                "entity_admission" => json!({
                    "decision":"create_new", "reason":"first_seen_identity", "promotion_policy":"immediate",
                    "entity_kind":"tool", "entity_summary":"Atlas is the migration tool."
                }),
                other => panic!("unexpected schema: {other}"),
            })
        }
    }

    #[test]
    fn admitted_entity_and_mention_survive_phy_reopen_and_retry() {
        let (path, session, memory_id, _, _) = phy_session(
            [91; 16],
            "Atlas is the migration tool.",
            "Other tool",
            "An unrelated tool.",
        );
        let owner_id = session.owner_id();
        let proposal = resolve_memory_mention(
            &EntityModelEvaluator::new(AdmissionEndpoint),
            EntityOwnerSession::Phy(&session),
            EntityMemoryMention {
                owner_id,
                memory_id,
                field: TextField::Content,
                start_byte: 0,
                end_byte: 5,
                text: "Atlas".into(),
            },
        )
        .unwrap();
        let command = proposal.into_mention_commit(10).unwrap();
        assert!(command.new_entity.is_some());
        drop(session);
        // Materialize only the first synced record to model interruption
        // between Entity creation and mention publication.
        let mut container =
            Container::open_writer_as(&path.0, reliquary_container::FileKind::Phylactery).unwrap();
        let mut store = PerceptionStore::open(&container).unwrap();
        let creation = store
            .publish_entity(
                &mut container,
                EntityDraft {
                    canonical_name: "Atlas".into(),
                    aliases: vec![],
                    kind: "tool".into(),
                    summary: "Atlas is the migration tool.".into(),
                    mutation_id: format!(
                        "entity-mention:{}:content:0:5",
                        memory_id
                            .0
                            .iter()
                            .map(|byte| format!("{byte:02x}"))
                            .collect::<String>(),
                    ),
                    created_at_ns: 1,
                    updated_at_ns: 1,
                },
            )
            .unwrap();
        drop(store);
        drop(container);
        let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let mut writer =
            reliquary_owner_session::PhyWriter::open_path(&path.0, &admission, []).unwrap();
        let resolution = writer.commit_entity_mention(command.clone()).unwrap();
        drop(writer);
        let mut writer =
            reliquary_owner_session::PhyWriter::open_path(&path.0, &admission, []).unwrap();
        assert_eq!(writer.commit_entity_mention(command).unwrap(), resolution);
        drop(writer);
        let session = PhySession::open_path(&path.0, &admission, []).unwrap();
        let reliquary_owner_session::model::MemoryEntityResolutionStatus::Resolved {
            entity_id,
            ..
        } = resolution.status
        else {
            panic!("admitted Entity did not resolve the mention");
        };
        assert_eq!(entity_id, creation.id);
        assert_eq!(session.entity(entity_id).unwrap().revision, 1);
        assert_eq!(session.entity(entity_id).unwrap().canonical_name, "Atlas");
        assert_eq!(session.mention(resolution.key), Some(&resolution));
    }

    fn owner_memory_draft(content: &str, mutation_id: &str) -> MemoryDraft {
        MemoryDraft {
            title: "Migration tool choice".into(),
            content: content.into(),
            category: "fact".into(),
            memory_type: "project".into(),
            authority_kind: "direct".into(),
            temporal_status: "current".into(),
            scope: "project".into(),
            lifecycle_state: "knowledge".into(),
            archived: false,
            parent_id: None,
            superseded_by: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_ref: None,
            source_time_ns: Some(1),
            temporal_inference: None,
            mutation_id: mutation_id.into(),
            created_at_ns: 1,
            updated_at_ns: 1,
        }
    }

    fn phy_session(
        owner_id: [u8; 16],
        content: &str,
        name: &str,
        summary: &str,
    ) -> (Temp, PhySession, MemoryId, MemoryId, EntityId) {
        let path = Temp::new();
        let mut container =
            Container::create(&path.0, ContainerIdentity::phylactery(), owner_id).unwrap();
        PackedVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        CompatibilityProfileStore::default()
            .initialize(&mut container)
            .unwrap();
        MemoryVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        let mut memories = MemoryStore::create(&mut container).unwrap();
        let memory = memories
            .publish(
                &mut container,
                None,
                0,
                owner_memory_draft(content, &format!("entity-workflow-memory-{}", owner_id[0])),
            )
            .unwrap()
            .revision
            .record
            .id;
        let second_memory = memories
            .publish(
                &mut container,
                None,
                0,
                owner_memory_draft(
                    "Second owner-local migration memory.",
                    &format!("entity-workflow-second-memory-{}", owner_id[0]),
                ),
            )
            .unwrap()
            .revision
            .record
            .id;
        let mut perception = PerceptionStore::open(&container).unwrap();
        let entity = perception
            .publish_entity(
                &mut container,
                EntityDraft {
                    canonical_name: name.into(),
                    aliases: vec!["N-Star".into()],
                    kind: "tool".into(),
                    summary: summary.into(),
                    mutation_id: format!("entity-workflow-entity-{}", owner_id[0]),
                    created_at_ns: 1,
                    updated_at_ns: 1,
                },
            )
            .unwrap()
            .id;
        perception
            .publish_entity(
                &mut container,
                EntityDraft {
                    canonical_name: "Unrelated Widget".into(),
                    aliases: vec![],
                    kind: "tool".into(),
                    summary: "foreign-candidate-secret".into(),
                    mutation_id: format!("entity-workflow-unrelated-{}", owner_id[0]),
                    created_at_ns: 1,
                    updated_at_ns: 1,
                },
            )
            .unwrap();
        container.sync().unwrap();
        drop(perception);
        drop(memories);
        drop(container);
        let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let session = PhySession::open_path(&path.0, &admission, []).unwrap();
        (path, session, memory, second_memory, entity)
    }

    struct AmbiguousEndpoint;
    impl GeneralEndpoint for AmbiguousEndpoint {
        fn model(&self) -> &str {
            "entity-ambiguity-fixture"
        }
        fn complete_json(
            &self,
            _prompt: &str,
            _payload: &str,
            schema: &str,
            _shape: &Value,
        ) -> Result<Value, GeneralEndpointError> {
            assert_eq!(schema, "entity_resolution");
            Ok(json!({
                "decision":"unresolved", "reason":"ambiguous", "target_candidate_index":-1,
                "identity_relation":"uncertain", "mention_kind":"tool"
            }))
        }
    }

    #[test]
    fn source_lexical_and_graph_lanes_reuse_only_owner_associations() {
        use reliquary_graph::{GraphStore, RelationChange, RelationOrigin};
        let owner_id = [92; 16];
        let (path, session, source, neighbor, entity) = phy_session(
            owner_id,
            "Use Atlas for migration.",
            "Northstar",
            "Owner-local tool",
        );
        let alternate = session
            .entities()
            .into_iter()
            .find(|item| item.id != entity)
            .unwrap()
            .id;
        drop(session);
        let mut container =
            Container::open_writer_as(&path.0, reliquary_container::FileKind::Phylactery).unwrap();
        let changes = GraphAdmission::for_changes(
            owner_id,
            [source.0, neighbor.0],
            [(entity.0, false), (alternate.0, false)],
        );
        let mut graph = GraphStore::open(&container, &changes).unwrap();
        graph
            .put(
                &mut container,
                &changes,
                &[
                    RelationChange {
                        source: NodeRef::Memory(source.0),
                        target: NodeRef::Entity(alternate.0),
                        kind: RelationKind::EntityAssociation,
                        active: true,
                        origin: RelationOrigin::Perception,
                    },
                    RelationChange {
                        source: NodeRef::Memory(source.0),
                        target: NodeRef::Entity(entity.0),
                        kind: RelationKind::EntityAssociation,
                        active: true,
                        origin: RelationOrigin::Perception,
                    },
                    RelationChange {
                        source: NodeRef::Memory(neighbor.0),
                        target: NodeRef::Entity(entity.0),
                        kind: RelationKind::EntityAssociation,
                        active: true,
                        origin: RelationOrigin::Perception,
                    },
                    RelationChange {
                        source: NodeRef::Memory(neighbor.0),
                        target: NodeRef::Memory(source.0),
                        kind: RelationKind::Topical,
                        active: true,
                        origin: RelationOrigin::Dream,
                    },
                ],
            )
            .unwrap();
        container.sync().unwrap();
        drop(graph);
        drop(container);
        let replay = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let session = PhySession::open_path(&path.0, &replay, []).unwrap();
        let input = EntityMemoryMention {
            owner_id,
            memory_id: source,
            field: TextField::Content,
            start_byte: 4,
            end_byte: 9,
            text: "Atlas".into(),
        };
        let prepared =
            prepare_memory_mention(EntityOwnerSession::Phy(&session), input.clone()).unwrap();
        assert_eq!(prepared.candidates.len(), 2);
        let candidate = &prepared.candidates[0];
        assert_eq!(candidate.entity.id, entity);
        assert!(!candidate.exact_surface);
        assert!(candidate.source_association);
        assert_eq!(candidate.lexical_memory_hits, 1);
        assert_eq!(candidate.graph_neighbor_hits, 1);
        assert_eq!(candidate.supporting_memory_ids, vec![neighbor]);
        drop(session);
        let mut writer =
            reliquary_owner_session::PhyWriter::open_path(&path.0, &replay, []).unwrap();
        assert_eq!(
            prepare_phy_writer_mention(&writer, input)
                .unwrap()
                .candidates,
            prepared.candidates
        );
        let command = prepared
            .evaluate_with_endpoint(AmbiguousEndpoint)
            .unwrap()
            .into_mention_commit(10)
            .unwrap();
        let receipt = writer.commit_entity_mention(command.clone()).unwrap();
        assert_eq!(writer.commit_entity_mention(command).unwrap(), receipt);
        drop(writer);
        let session = PhySession::open_path(&path.0, &replay, []).unwrap();
        let mut expected = vec![entity, alternate];
        expected.sort_unstable();
        assert_eq!(
            receipt.status,
            reliquary_owner_session::model::MemoryEntityResolutionStatus::Ambiguous {
                candidates: expected
            }
        );
        assert_eq!(session.mention(receipt.key), Some(&receipt));
    }

    #[test]
    fn generic_candidates_exclude_principal_entities() {
        let entity = Entity {
            id: reliquary_perception_store::EntityId([1; 32]),
            revision: 1,
            canonical_name: "phy-00000000-0000-0000-0000-000000000123".into(),
            aliases: vec!["Alice".into()],
            kind: "principal".into(),
            summary: String::new(),
            mutation_id: "principal".into(),
            created_at_ns: 1,
            updated_at_ns: 1,
        };
        let mention = MentionSpan {
            field: TextField::Title,
            start_byte: 0,
            end_byte: 5,
            text: "Alice".into(),
        };
        assert!(build_candidates(&[entity], &mention).unwrap().is_empty());
    }

    #[test]
    fn span_validation_is_utf8_exact_and_normalization_is_deterministic() {
        let body = MemoryBody {
            title: "Café Engine".into(),
            content: "Private body must not leak.".into(),
        };
        let exact = MentionSpan {
            field: TextField::Title,
            start_byte: 0,
            end_byte: 5,
            text: "Café".into(),
        };
        assert!(validate_span(&body, &exact).is_ok());
        let split = MentionSpan {
            end_byte: 4,
            ..exact.clone()
        };
        assert_eq!(
            validate_span(&body, &split),
            Err(EntityWorkflowError::InvalidMention("span splits UTF-8"))
        );
        assert_eq!(normalize("NORTH-star"), normalize("North Star"));
    }

    #[test]
    fn phy_resolution_uses_owner_body_and_candidates_and_returns_a_proposal() {
        let (path, session, memory, _second_memory, entity) = phy_session(
            [88; 16],
            "Use Northstar in the internal migration. Owner-only note: blue lantern.",
            "Northstar",
            "The owner's migration tool.",
        );
        let (_foreign_path, _foreign_session, _foreign_memory, _foreign_second_memory, _) =
            phy_session(
                [89; 16],
                "Foreign-owner body marker: silver comet.",
                "ForeignSystem",
                "foreign-owner entity secret",
            );
        let endpoint = RecordingEndpoint::default();
        let evaluator = EntityModelEvaluator::new(endpoint.clone());
        let proposal = resolve_memory_mention(
            &evaluator,
            EntityOwnerSession::Phy(&session),
            EntityMemoryMention {
                owner_id: session.owner_id(),
                memory_id: memory,
                field: TextField::Content,
                start_byte: 4,
                end_byte: 13,
                text: "Northstar".into(),
            },
        )
        .unwrap();
        assert_eq!(
            proposal.outcome.decision,
            reliquary_entity_resolution::ResolutionDecision::ResolveExisting(entity)
        );
        assert_eq!(proposal.candidates.len(), 1);
        assert_eq!(proposal.memory_revision, 1);
        assert!(proposal.existing_resolution.is_none());

        let payload = endpoint.0.lock().unwrap().join("\n");
        assert!(payload.contains("Owner-only note: blue lantern."));
        assert!(payload.contains("The owner's migration tool."));
        assert!(!payload.contains("foreign-candidate-secret"));
        assert!(!payload.contains("Foreign-owner body marker: silver comet."));
        assert!(!payload.contains("foreign-owner entity secret"));

        let wrong_owner = EntityMemoryMention {
            owner_id: [89; 16],
            memory_id: memory,
            field: TextField::Content,
            start_byte: 4,
            end_byte: 13,
            text: "Northstar".into(),
        };
        assert_eq!(
            resolve_memory_mention(&evaluator, EntityOwnerSession::Phy(&session), wrong_owner,),
            Err(EntityWorkflowError::OwnerMismatch)
        );

        let command = proposal.into_mention_commit(20).unwrap();
        let key = command.key;
        let admission = GraphAdmission::for_replay(session.owner_id(), [], [], [], [], []);
        drop(session);
        let mut writer =
            reliquary_owner_session::PhyWriter::open_path(&path.0, &admission, []).unwrap();
        let mut foreign = command.clone();
        foreign.owner_id = [89; 16];
        assert!(writer.commit_entity_mention(foreign).is_err());
        let mut stale = command.clone();
        stale.target_revisions[0].1 += 1;
        assert!(writer.commit_entity_mention(stale).is_err());
        let stored = writer.commit_entity_mention(command.clone()).unwrap();
        assert_eq!(writer.commit_entity_mention(command).unwrap(), stored);
        drop(writer);
        let reopened = PhySession::open_path(&path.0, &admission, []).unwrap();
        assert_eq!(reopened.mention(key), Some(&stored));
        assert_eq!(reopened.mention_revision(key, 1), Some(&stored));
    }

    #[test]
    fn retrieval_selection_hydrates_owner_bodies_and_ranks_supplied_scores() {
        let (_path, session, memory, second_memory, _) = phy_session(
            [90; 16],
            "Use Northstar in the internal migration. Owner-only note: blue lantern.",
            "Northstar",
            "The owner's migration tool.",
        );
        let selection = crate::select_owner_memories(
            &EntityOwnerSession::Phy(&session),
            session.owner_id(),
            "Northstar migration",
            &[
                crate::OwnerMemoryEvidence {
                    owner_id: session.owner_id(),
                    memory_id: memory,
                    lexical_score: Some(0.8),
                    semantic_score: Some(0.6),
                },
                crate::OwnerMemoryEvidence {
                    owner_id: session.owner_id(),
                    memory_id: second_memory,
                    lexical_score: Some(0.9),
                    semantic_score: Some(0.8),
                },
            ],
            reliquary_retrieval::RetrievalConfig::default(),
        )
        .unwrap();
        assert_eq!(selection.selected.len(), 2);
        assert_eq!(selection.selected[0].memory_id, second_memory);
        assert_eq!(selection.selected[1].memory_id, memory);
        assert_eq!(selection.selected[0].revision, 1);
        assert!(
            selection.selected[1]
                .body
                .content
                .contains("Owner-only note")
        );

        assert_eq!(
            crate::select_owner_memories(
                &EntityOwnerSession::Phy(&session),
                session.owner_id(),
                "Northstar migration",
                &[crate::OwnerMemoryEvidence {
                    owner_id: [91; 16],
                    memory_id: memory,
                    lexical_score: Some(0.8),
                    semantic_score: None,
                }],
                reliquary_retrieval::RetrievalConfig::default(),
            ),
            Err(crate::RetrievalWorkflowError::OwnerMismatch)
        );
    }
}
