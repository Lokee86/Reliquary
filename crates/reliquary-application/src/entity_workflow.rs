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
}

impl EntityOwnerSession<'_> {
    pub(crate) fn owner_id(&self) -> [u8; 16] {
        match self {
            Self::Rel(session) => session.owner_id(),
            Self::Phy(session) => session.owner_id(),
        }
    }

    pub(crate) fn memory_history(
        &self,
        memory_id: MemoryId,
    ) -> Result<Option<reliquary_owner_session::model::MemoryHistory>, OwnerSessionError> {
        match self {
            Self::Rel(session) => session.memory_history(memory_id),
            Self::Phy(session) => session.memory_history(memory_id),
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
        }
    }

    pub(crate) fn graph_version(&self) -> u64 {
        match self {
            Self::Rel(session) => session.graph_version(),
            Self::Phy(session) => session.graph_version(),
        }
    }

    pub(crate) fn active_compatibility_profile(
        &self,
        id: CompatibilityProfileId,
    ) -> Option<CompatibilityProfile> {
        match self {
            Self::Rel(session) => session.active_compatibility_profile(id).cloned(),
            Self::Phy(session) => session.active_compatibility_profile(id).cloned(),
        }
    }

    fn entities(&self) -> Vec<Entity> {
        match self {
            Self::Rel(session) => session.entities(),
            Self::Phy(session) => session.entities(),
        }
    }

    fn mention(&self, key: MemoryEntityMentionKey) -> Option<MemoryEntityResolution> {
        match self {
            Self::Rel(session) => session.mention(key).cloned(),
            Self::Phy(session) => session.mention(key).cloned(),
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
    pub owner_id: [u8; 16],
    pub memory_id: MemoryId,
    pub memory_revision: u64,
    pub body_id: MemoryBodyId,
    pub mention: MentionSpan,
    pub candidates: Vec<EntityCandidate>,
    pub existing_resolution: Option<MemoryEntityResolution>,
    pub outcome: ResolutionOutcome,
}

/// Resolve one exact Memory span using only that owner's latest body and Entity
/// records. The endpoint receives no cross-owner context; authorization to
/// obtain the session remains the caller's responsibility.
pub fn resolve_memory_mention<E: GeneralEndpoint>(
    evaluator: &EntityModelEvaluator<E>,
    owner: EntityOwnerSession<'_>,
    input: EntityMemoryMention,
) -> Result<EntityResolutionProposal, EntityWorkflowError> {
    let owner_id = owner.owner_id();
    if owner_id != input.owner_id {
        return Err(EntityWorkflowError::OwnerMismatch);
    }
    let history = owner
        .memory_history(input.memory_id)?
        .ok_or(EntityWorkflowError::MemoryUnavailable)?;
    if history.tombstone.is_some() {
        return Err(EntityWorkflowError::MemoryTombstoned);
    }
    let revision = history
        .revisions
        .last()
        .ok_or(EntityWorkflowError::MemoryUnavailable)?;
    let body = match &revision.payload {
        PayloadState::Available(body) => body,
        PayloadState::Absent => return Err(EntityWorkflowError::MemoryBodyUnavailable),
    };
    let mention = MentionSpan {
        field: input.field,
        start_byte: input.start_byte,
        end_byte: input.end_byte,
        text: input.text,
    };
    validate_span(body, &mention)?;

    let mut candidates = build_candidates(&owner.entities(), &mention)?;
    rank_candidates(&mut candidates);
    let outcome = evaluator
        .resolve(body, &mention, &candidates)
        .map_err(|error| EntityWorkflowError::Evaluator(error.to_string()))?;
    let key = MemoryEntityMentionKey {
        memory_id: input.memory_id.0,
        field: match mention.field {
            TextField::Title => MemoryTextField::Title,
            TextField::Content => MemoryTextField::Content,
        },
        start_byte: u32::try_from(mention.start_byte)
            .map_err(|_| EntityWorkflowError::InvalidMention("offset exceeds stored range"))?,
        end_byte: u32::try_from(mention.end_byte)
            .map_err(|_| EntityWorkflowError::InvalidMention("offset exceeds stored range"))?,
    };
    let existing_resolution = owner.mention(key);
    Ok(EntityResolutionProposal {
        owner_id,
        memory_id: input.memory_id,
        memory_revision: revision.record.revision,
        body_id: revision.record.body_id,
        mention,
        candidates,
        existing_resolution,
        outcome,
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
        let (_path, session, memory, _second_memory, entity) = phy_session(
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
