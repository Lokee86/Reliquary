use super::InsomniaProcessError;
use super::prepared::{PreparedApplication, PreparedMemory, PreparedUserMemory, UserPublication};
use super::source_validation::{candidate_source_time_ns, validate_candidate_sources};
use crate::insomnia::candidate::hex;
use crate::memory_model::{memory_body_id, memory_id};
use crate::{
    Episode, InsomniaExtraction, InsomniaOwnership, InsomniaRejection, Memory, MemoryDraft,
    MemoryRef, MemoryRoutingMetadata, MemorySourceRef,
};

pub(crate) fn prepare_application_read(
    archive: &crate::archive::ArchiveReadView<'_>,
    owner_id: Option<String>,
    episode: &Episode,
    turns: &[crate::ResolvedTurn],
    extraction: InsomniaExtraction,
    scope: &str,
    completed_at_ns: i64,
) -> Result<PreparedApplication, InsomniaProcessError> {
    let mut rejected = extraction.rejected;
    let mut project_drafts = Vec::with_capacity(extraction.candidates.len());
    let mut user_drafts = Vec::new();
    for candidate in extraction.candidates {
        if let Err(reason) = validate_candidate_sources(
            archive,
            &candidate,
            &episode.conversation_id,
            turns,
            &extraction.evidence_turns,
        ) {
            rejected.push(InsomniaRejection {
                candidate_key: Some(candidate.key.clone()),
                reason,
            });
            continue;
        }
        let source_time_ns = candidate_source_time_ns(archive, &candidate, turns)?;
        let mut draft = MemoryDraft {
            category: candidate.category,
            memory_type: candidate.memory_type,
            authority_kind: candidate.authority_kind,
            temporal_status: candidate.temporal_status,
            title: candidate.title,
            content: candidate.content,
            scope: scope.trim().to_owned(),
            lifecycle_state: "extracted".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: Some(candidate.source_node_id),
            content_source_conversation_id: candidate.authority_source_conversation_id,
            content_source_node_id: candidate.authority_source_node_id,
            grounding_source_conversation_id: candidate.grounding_source_conversation_id,
            grounding_source_node_id: candidate.grounding_source_node_id,
            source_episode_id: Some(episode.id),
            source_time_ns: Some(source_time_ns),
            mutation_id: format!("insomnia:{}:{}", hex(&episode.id.0), candidate.key),
            created_at_ns: episode.source_through_ns,
            updated_at_ns: completed_at_ns.max(episode.source_through_ns),
        };
        let temporal = crate::insomnia::temporal::assess_draft(&draft);
        let routing_metadata = candidate
            .routing_metadata
            .map(|routing| MemoryRoutingMetadata {
                memory_id: memory_id(&draft.mutation_id),
                body_id: memory_body_id(&draft.title, &draft.content),
                entity_mentions: routing.entity_mentions,
            });
        match candidate.ownership {
            InsomniaOwnership::Project => project_drafts.push(PreparedMemory {
                draft,
                temporal,
                temporal_inference: None,
                routing_metadata,
            }),
            InsomniaOwnership::User => {
                let owner_id = owner_id.clone().ok_or_else(|| {
                    InsomniaProcessError::InvalidCandidate(
                        "source Reliquary has no durable owner ID; migrate it before routing user Memory provenance"
                            .into(),
                    )
                })?;
                let source_ref = MemorySourceRef {
                    owner_id,
                    principal_id: turns
                        .iter()
                        .find(|turn| {
                            draft
                                .source_node_id
                                .as_deref()
                                .is_some_and(|source| source == turn.node_id)
                        })
                        .and_then(|turn| turn.principal_id.clone()),
                    source_episode_id: episode.id,
                    source_node_id: draft.source_node_id.clone().ok_or_else(|| {
                        InsomniaProcessError::InvalidCandidate(
                            "user Memory source node is unavailable".into(),
                        )
                    })?,
                    content_source_conversation_id: draft.content_source_conversation_id.clone(),
                    content_source_node_id: draft.content_source_node_id.clone(),
                    grounding_source_conversation_id: draft
                        .grounding_source_conversation_id
                        .clone(),
                    grounding_source_node_id: draft.grounding_source_node_id.clone(),
                };
                draft.source_node_id = None;
                draft.content_source_conversation_id = None;
                draft.content_source_node_id = None;
                draft.grounding_source_conversation_id = None;
                draft.grounding_source_node_id = None;
                draft.source_episode_id = None;
                user_drafts.push(PreparedUserMemory {
                    memory: PreparedMemory {
                        draft,
                        temporal,
                        temporal_inference: None,
                        routing_metadata,
                    },
                    source_ref,
                });
            }
        }
    }
    Ok(PreparedApplication {
        project_drafts,
        user_drafts,
        rejected,
        model: extraction.model,
    })
}
