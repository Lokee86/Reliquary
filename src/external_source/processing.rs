use super::provenance::ExternalMemoryEvidence;
use crate::external_index::model::*;
use crate::external_index::store::SourceState;
use crate::{MemoryDraft, MemoryRef, MemoryRoutingMetadata};
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

pub(crate) fn evidence_chunk(
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
pub(crate) fn optional_chunk(
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
