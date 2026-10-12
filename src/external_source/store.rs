use super::model::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const MAGIC: &[u8; 8] = b"CVAXIDX1";
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IndexedChunk {
    pub reference: ExternalChunkRef,
    pub matrix: [u8; 32],
    pub row: u32,
    /// Lexical routing retains term hashes, never source strings.
    pub terms: Vec<[u8; 32]>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IndexedMessage {
    pub descriptor: ExternalMessageDescriptor,
    pub chunks: Vec<IndexedChunk>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceState {
    pub namespace: String,
    pub contract: u32,
    pub profile: [u8; 32],
    pub dimensions: u32,
    pub generation: u64,
    pub revision: u64,
    pub cursor: u64,
    pub messages: Vec<IndexedMessage>,
    pub last_base: u64,
    pub last_digest: [u8; 32],
    pub rebuild: bool,
}
impl SourceState {
    pub(crate) fn status(&self) -> ExternalIndexStatus {
        ExternalIndexStatus {
            source_namespace: self.namespace.clone(),
            generation: self.generation,
            revision: self.revision,
            cursor: self.cursor,
            messages: self.messages.len(),
            chunks: self.messages.iter().map(|m| m.chunks.len()).sum(),
            compatibility_profile: crate::CompatibilityProfileId(self.profile),
        }
    }
    pub(crate) fn validate(&self) -> Result<(), ExternalSourceError> {
        identifier(&self.namespace)?;
        if self.contract != EXTERNAL_SOURCE_CONTRACT
            || self.generation == 0
            || self.revision == 0
            || self.dimensions == 0
            || self.dimensions > 4096
            || self.messages.len() > MAX_EXTERNAL_MANIFEST
            || self.last_base > self.cursor
        {
            return Err(ExternalSourceError::InvalidContract);
        }
        let mut prior = None;
        for m in &self.messages {
            m.descriptor.validate(&self.namespace)?;
            let key = (
                &m.descriptor.reference.conversation_id,
                &m.descriptor.reference.message_id,
            );
            if prior.is_some_and(|p| p >= key) {
                return Err(ExternalSourceError::InvalidContract);
            }
            prior = Some(key);
            if !m.descriptor.active && !m.chunks.is_empty() {
                return Err(ExternalSourceError::InvalidContract);
            }
            let mut end = 0;
            for c in &m.chunks {
                if c.reference.message != m.descriptor.reference
                    || c.reference.start != end
                    || c.reference.end <= end
                    || c.reference.end > MAX_EXTERNAL_MESSAGE_BYTES as u64
                    || c.terms.len() > 1024
                    || c.terms.windows(2).any(|w| w[0] >= w[1])
                {
                    return Err(ExternalSourceError::InvalidContract);
                }
                end = c.reference.end;
            }
        }
        Ok(())
    }
}
#[derive(Default)]
pub(crate) struct ExternalIndexStore {
    pub sources: BTreeMap<String, SourceState>,
    /// A damaged derived record is discarded; canonical REL semantics stay intact.
    pub damaged: bool,
}
impl ExternalIndexStore {
    pub(crate) fn ingest(&mut self, bytes: &[u8]) {
        if !bytes.starts_with(MAGIC) {
            return;
        }
        let decoded = decode(bytes);
        match decoded {
            Ok(state) => {
                let valid = if state.rebuild {
                    self.sources.get(&state.namespace).is_none_or(|old| {
                        Some(state.revision) == old.revision.checked_add(1)
                            && Some(state.generation) == old.generation.checked_add(1)
                            && state.cursor >= old.cursor
                    })
                } else {
                    self.sources.get(&state.namespace).is_some_and(|old| {
                        Some(state.revision) == old.revision.checked_add(1)
                            && state.generation == old.generation
                            && state.last_base == old.cursor
                            && state.cursor >= old.cursor
                    })
                };
                if valid {
                    self.sources.insert(state.namespace.clone(), state);
                } else {
                    self.sources.clear();
                    self.damaged = true;
                }
            }
            Err(_) => {
                self.sources.clear();
                self.damaged = true;
            }
        }
    }
    pub(crate) fn install(
        &mut self,
        container: &mut crate::Container,
        state: SourceState,
        expected_revision: Option<u64>,
    ) -> Result<(), ExternalSourceError> {
        if self.sources.get(&state.namespace).map(|s| s.revision) != expected_revision {
            return Err(ExternalSourceError::Conflict);
        }
        let bytes = encode(&state)?;
        if container.append(&bytes).is_err() {
            self.sources.clear();
            self.damaged = true;
            return Err(ExternalSourceError::Storage);
        }
        // Memory/cursor state is exposed only after this complete record is synchronized.
        if container.sync().is_err() {
            self.sources.clear();
            self.damaged = true;
            return Err(ExternalSourceError::Storage);
        }
        self.sources.insert(state.namespace.clone(), state);
        Ok(())
    }
    pub(crate) fn validate_backing(
        &mut self,
        packed: &crate::packed_vector_store::PackedVectorStore,
        profiles: &crate::compatibility_profile_store::CompatibilityProfileStore,
    ) {
        self.sources.retain(|_, s| {
            let valid = profiles
                .get(crate::CompatibilityProfileId(s.profile))
                .is_some_and(|p| p.dimensions == s.dimensions)
                && s.messages.iter().flat_map(|m| &m.chunks).all(|c| {
                    packed
                        .info(crate::PackedVectorId(c.matrix))
                        .is_some_and(|i| {
                            i.schema.dimensions == s.dimensions
                                && i.schema.scalar == crate::ScalarType::F32
                                && u64::from(c.row) < i.count
                        })
                });
            if !valid {
                self.damaged = true;
            }
            valid
        });
    }
}
pub(crate) fn encode(state: &SourceState) -> Result<Vec<u8>, ExternalSourceError> {
    state.validate()?;
    let json = serde_json::to_vec(state).map_err(|_| ExternalSourceError::InvalidContract)?;
    if json.len() > MAX_EXTERNAL_INDEX_BYTES {
        return Err(ExternalSourceError::Bounds);
    }
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&(json.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&json);
    bytes.extend_from_slice(&Sha256::digest(&bytes));
    Ok(bytes)
}
fn decode(bytes: &[u8]) -> Result<SourceState, ExternalSourceError> {
    if bytes.len() < 48 || bytes.len() > MAX_EXTERNAL_INDEX_BYTES + 48 {
        return Err(ExternalSourceError::Bounds);
    }
    let len = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    if usize::try_from(len).ok().and_then(|l| l.checked_add(48)) != Some(bytes.len())
        || Sha256::digest(&bytes[..bytes.len() - 32]).as_slice() != &bytes[bytes.len() - 32..]
    {
        return Err(ExternalSourceError::Storage);
    }
    let state: SourceState = serde_json::from_slice(&bytes[16..bytes.len() - 32])
        .map_err(|_| ExternalSourceError::InvalidContract)?;
    state.validate()?;
    Ok(state)
}
