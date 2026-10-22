use super::model::*;
use super::provenance::ExternalMemoryEvidence;
use super::store::SourceState;
use crate::{GeneralEndpoint, InsomniaExtractor, MemoryDraft, MemoryRef, MemoryRoutingMetadata};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(crate) struct PreparedExternalMemory {
    pub draft: MemoryDraft,
    pub temporal: Option<crate::MemoryTemporalInference>,
    pub routing: Option<MemoryRoutingMetadata>,
    pub evidence: ExternalMemoryEvidence,
}
/// Opaque transient application. It stores derived Memory drafts, not a source cache.
pub struct PreparedExternalProcessing {
    pub(crate) window: [u8; 32],
    pub(crate) expected: SourceState,
    pub(crate) project: Vec<PreparedExternalMemory>,
    pub(crate) user: Vec<PreparedExternalMemory>,
    pub(crate) rejected: usize,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalProcessingResult {
    pub project: Vec<MemoryRef>,
    pub user: Vec<MemoryRef>,
    pub rejected: Option<usize>,
    pub replayed: bool,
}
pub(crate) fn descriptors(
    state: &SourceState,
    refs: &[ExternalMessageRef],
) -> Result<Vec<ExternalMessageDescriptor>, ExternalSourceError> {
    if refs.is_empty() || refs.len() > MAX_EXTERNAL_BATCH {
        return Err(ExternalSourceError::Bounds);
    }
    let conversation = &refs[0].conversation_id;
    let mut seen = BTreeSet::new();
    let mut descriptors = Vec::new();
    for reference in refs {
        reference.validate()?;
        if &reference.conversation_id != conversation
            || reference.source_namespace != state.namespace
            || !seen.insert(&reference.message_id)
        {
            return Err(ExternalSourceError::InvalidContract);
        }
        let descriptor = state
            .messages
            .iter()
            .find(|m| m.descriptor.reference == *reference && m.descriptor.active)
            .ok_or(ExternalSourceError::StaleEvidence)?
            .descriptor
            .clone();
        if descriptors
            .last()
            .is_some_and(|old: &ExternalMessageDescriptor| {
                (old.timestamp_ns, &old.reference.message_id)
                    > (descriptor.timestamp_ns, &descriptor.reference.message_id)
            })
        {
            return Err(ExternalSourceError::InvalidContract);
        }
        descriptors.push(descriptor);
    }
    Ok(descriptors)
}
pub(crate) fn window_key(
    owner: &str,
    scope: &str,
    descriptors: &[ExternalMessageDescriptor],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"RELIQUARY-EXTERNAL-INSOMNIA-1");
    for s in [owner, scope, crate::INSOMNIA_EXTRACTOR_CONTRACT_VERSION] {
        hash.update((s.len() as u64).to_le_bytes());
        hash.update(s.as_bytes());
    }
    hash.update(serde_json::to_vec(descriptors).expect("descriptor serialization"));
    hash.finalize().into()
}
pub(crate) fn prepare<E: GeneralEndpoint>(
    owner: &str,
    principal: &str,
    scope: &str,
    state: SourceState,
    refs: &[ExternalMessageRef],
    source: &impl ExternalConversationSource,
    extractor: &InsomniaExtractor<E>,
    now_ns: i64,
) -> Result<PreparedExternalProcessing, ExternalSourceError> {
    identifier(scope)?;
    let expected = descriptors(&state, refs)?;
    let window = window_key(owner, scope, &expected);
    let hydrated = source.hydrate(principal, &expected)?;
    if hydrated.len() != expected.len() {
        return Err(ExternalSourceError::InvalidContract);
    }
    let mut bytes = 0;
    for (message, descriptor) in hydrated.iter().zip(&expected) {
        message.validate(descriptor)?;
        bytes += message.content.len();
    }
    if bytes > 256 * 1024 {
        return Err(ExternalSourceError::Bounds);
    }
    // Legacy internal field node_id is a transient source anchor here. It is
    // never persisted as an Archive ID or carried into a MemoryDraft.
    let turns = hydrated
        .iter()
        .map(|m| crate::ResolvedTurn {
            node_id: m.descriptor.reference.message_id.clone(),
            role: m.descriptor.role.clone(),
            principal_id: m.descriptor.principal_id.clone(),
            timestamp_ns: m.descriptor.timestamp_ns,
            content: m.content.clone(),
        })
        .collect::<Vec<_>>();
    let extraction = extractor
        .extract_external_input(window, &state.namespace, &refs[0].conversation_id, &turns)
        .map_err(|_| ExternalSourceError::Unavailable)?;
    let mut project = Vec::new();
    let mut user = Vec::new();
    let inferencer = crate::TemporalInferencer::new(extractor.temporal_endpoint());
    let through = expected.iter().map(|d| d.timestamp_ns).max().unwrap();
    for candidate in extraction.candidates {
        let authority = evidence_chunk(
            &hydrated,
            &candidate.source_node_id,
            &candidate.source_quote,
        )?;
        let anchor = hydrated
            .iter()
            .find(|m| m.descriptor.reference == authority.message)
            .unwrap();
        if anchor.descriptor.role != "user" {
            return Err(ExternalSourceError::InvalidContract);
        }
        if candidate.ownership == crate::InsomniaOwnership::User
            && anchor.descriptor.principal_id.as_deref() != Some(principal)
        {
            return Err(ExternalSourceError::Unauthorized);
        }
        let content = optional_chunk(
            &hydrated,
            candidate.authority_source_node_id.as_deref(),
            candidate.authority_source_quote.as_deref(),
        )?;
        let grounding = optional_chunk(
            &hydrated,
            candidate.grounding_source_node_id.as_deref(),
            candidate.grounding_source_quote.as_deref(),
        )?;
        for conversation in [
            candidate.authority_source_conversation_id.as_deref(),
            candidate.grounding_source_conversation_id.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if conversation != refs[0].conversation_id {
                return Err(ExternalSourceError::InvalidContract);
            }
        }
        let source_time = content
            .as_ref()
            .map_or(anchor.descriptor.timestamp_ns, |c| {
                hydrated
                    .iter()
                    .find(|m| m.descriptor.reference == c.message)
                    .unwrap()
                    .descriptor
                    .timestamp_ns
            });
        let draft = MemoryDraft {
            category: candidate.category,
            memory_type: candidate.memory_type,
            authority_kind: candidate.authority_kind,
            temporal_status: candidate.temporal_status,
            title: candidate.title,
            content: candidate.content,
            scope: if candidate.ownership == crate::InsomniaOwnership::User {
                "private".to_owned()
            } else {
                scope.to_owned()
            },
            lifecycle_state: "extracted".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_time_ns: Some(source_time),
            mutation_id: format!("external-insomnia:{}:{}", hex(&window), candidate.key),
            created_at_ns: through,
            updated_at_ns: now_ns.max(through),
        };
        let routing = candidate.routing_metadata.map(|r| MemoryRoutingMetadata {
            memory_id: crate::memory_model::memory_id(&draft.mutation_id),
            body_id: crate::memory_model::memory_body_id(&draft.title, &draft.content),
            entity_mentions: r.entity_mentions,
        });
        let mut prepared = crate::insomnia::PreparedMemory {
            temporal: crate::insomnia::temporal::assess_draft(&draft),
            draft,
            temporal_inference: None,
            routing_metadata: routing,
        };
        crate::insomnia::temporal::infer_memory(&inferencer, &mut prepared)
            .map_err(|_| ExternalSourceError::Unavailable)?;
        let temporal = prepared.bound_temporal_inference();
        let evidence = ExternalMemoryEvidence {
            source_owner_id: owner.to_owned(),
            principal_id: anchor
                .descriptor
                .principal_id
                .clone()
                .unwrap_or_else(|| principal.to_owned()),
            processing_window: window,
            authority,
            content,
            grounding,
        };
        evidence.validate()?;
        let value = PreparedExternalMemory {
            draft: prepared.draft,
            temporal,
            routing: prepared.routing_metadata,
            evidence,
        };
        match candidate.ownership {
            crate::InsomniaOwnership::Project => project.push(value),
            crate::InsomniaOwnership::User => user.push(value),
        }
    }
    Ok(PreparedExternalProcessing {
        window,
        expected: state,
        project,
        user,
        rejected: extraction.rejected.len(),
    })
}
fn evidence_chunk(
    messages: &[HydratedExternalMessage],
    id: &str,
    quote: &str,
) -> Result<ExternalChunkRef, ExternalSourceError> {
    let message = messages
        .iter()
        .find(|m| m.descriptor.reference.message_id == id)
        .ok_or(ExternalSourceError::StaleEvidence)?;
    if quote.is_empty() {
        return Err(ExternalSourceError::InvalidContract);
    }
    let start = message
        .content
        .find(quote)
        .ok_or(ExternalSourceError::StaleEvidence)?;
    Ok(ExternalChunkRef {
        message: message.descriptor.reference.clone(),
        start: start as u64,
        end: (start + quote.len()) as u64,
    })
}
fn optional_chunk(
    messages: &[HydratedExternalMessage],
    id: Option<&str>,
    quote: Option<&str>,
) -> Result<Option<ExternalChunkRef>, ExternalSourceError> {
    match (id, quote) {
        (None, None) => Ok(None),
        (Some(id), Some(quote)) => evidence_chunk(messages, id, quote).map(Some),
        _ => Err(ExternalSourceError::InvalidContract),
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl crate::Cva {
    pub(crate) fn publish_external_window(
        &mut self,
        window: [u8; 32],
        prepared: &[PreparedExternalMemory],
    ) -> Result<Vec<crate::Memory>, crate::MemoryError> {
        if let Some(existing) = self.memories.external_window(&mut self.container, window)? {
            return Ok(existing);
        }
        let turn = self.rel_turn_count();
        let mut after = self.freshness.clone();
        if let Some(owner) = self.owner_uuid() {
            for m in prepared {
                after
                    .derive_birth(
                        owner,
                        crate::memory_model::memory_id(&m.draft.mutation_id),
                        turn,
                        false,
                    )
                    .map_err(crate::MemoryError::Freshness)?;
            }
        }
        let birth_turn = self.owner_uuid().map(|_| turn);
        let results = super::publication::publish(
            &mut self.container,
            &mut self.memories,
            window,
            prepared,
            birth_turn,
        )?;
        self.freshness = after;
        for memory in &results {
            self.dream_freshness.derive_publication_birth(memory.id);
        }
        Ok(results)
    }
}
