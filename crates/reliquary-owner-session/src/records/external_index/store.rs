use super::model::*;
use reliquary_archive::Archive;
use reliquary_container::Container;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedChunk {
    pub reference: ExternalChunkRef,
    pub matrix: [u8; 32],
    pub row: u32,
    /// Lexical routing retains term hashes, never source strings.
    pub terms: Vec<[u8; 32]>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedMessage {
    pub descriptor: ExternalMessageDescriptor,
    pub chunks: Vec<IndexedChunk>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceState {
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
    pub fn validate(&self) -> Result<(), ExternalSourceError> {
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
pub struct ExternalIndexStore {
    pub sources: BTreeMap<String, SourceState>,
    /// A damaged derived record is discarded; canonical REL semantics stay intact.
    pub damaged: bool,
    owner: Option<[u8; 16]>,
}
impl ExternalIndexStore {
    /// Open derived indexes from a single 2.1 owner container. These records
    /// are disposable hints and never establish source authority by themselves.
    pub fn open(container: &Container) -> Result<Self, String> {
        if container.identity().file_kind != reliquary_container::FileKind::Reliquary {
            return Err("external index requires an owner Reliquary".into());
        }
        let mut store = Self {
            owner: Some(container.owner_id()),
            ..Self::default()
        };
        for frame in container.read_frames().map_err(|e| e.to_string())? {
            store.ingest(&frame.payload);
        }
        Ok(store)
    }

    pub fn ingest(&mut self, bytes: &[u8]) {
        if bytes.starts_with(super::compact_index::MAGIC) {
            match super::compact_index::decode(bytes) {
                Ok(super::compact_index::Record::Snapshot(state)) => {
                    // Each derived snapshot stands alone. Revisions remain CAS
                    // state for writers, never authoritative retained history.
                    self.sources.insert(state.namespace.clone(), state);
                }
                Ok(super::compact_index::Record::Discard) | Err(_) => {
                    self.sources.clear();
                    self.damaged = true;
                }
            }
            return;
        }
    }

    /// Persist a snapshot only after each indexed descriptor is matched to the
    /// canonical owner Archive node. The index remains derived: callers must
    /// rehydrate and authorize hits before returning source content.
    pub fn install_admitted(
        &mut self,
        container: &mut Container,
        archive: &Archive,
        state: SourceState,
        expected_revision: Option<u64>,
    ) -> Result<(), ExternalSourceError> {
        if self.owner != Some(container.owner_id()) {
            return Err(ExternalSourceError::Unauthorized);
        }
        archive
            .validate_owner(container)
            .map_err(|_| ExternalSourceError::Unauthorized)?;
        state.validate()?;
        for message in &state.messages {
            let descriptor = &message.descriptor;
            let node = archive
                .node(
                    &descriptor.reference.conversation_id,
                    &descriptor.reference.message_id,
                )
                .ok_or(ExternalSourceError::Unauthorized)?;
            if node.content_id.0 != descriptor.reference.content_hash
                || node.role != descriptor.role
                || node.principal_id != descriptor.principal_id
                || node.timestamp_ns != descriptor.timestamp_ns
            {
                return Err(ExternalSourceError::Unauthorized);
            }
            let content = archive
                .content_for_node(
                    &descriptor.reference.conversation_id,
                    &descriptor.reference.message_id,
                )
                .ok_or(ExternalSourceError::Unauthorized)?;
            if message.chunks.iter().any(|chunk| {
                let start = chunk.reference.start as usize;
                let end = chunk.reference.end as usize;
                end > content.len()
                    || !content.is_char_boundary(start)
                    || !content.is_char_boundary(end)
            }) {
                return Err(ExternalSourceError::InvalidContract);
            }
        }
        self.install(container, state, expected_revision)
    }

    fn install(
        &mut self,
        container: &mut reliquary_container::Container,
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
    /// Durably discard all derived source snapshots for this owner.
    pub fn discard_all(&mut self, container: &mut Container) -> Result<(), ExternalSourceError> {
        if self.owner != Some(container.owner_id()) {
            return Err(ExternalSourceError::Unauthorized);
        }
        let bytes = super::compact_index::discard();
        if container.append(&bytes).is_err() || container.sync().is_err() {
            self.sources.clear();
            self.damaged = true;
            return Err(ExternalSourceError::Storage);
        }
        self.sources.clear();
        self.damaged = true;
        Ok(())
    }
}
pub fn encode(state: &SourceState) -> Result<Vec<u8>, ExternalSourceError> {
    super::compact_index::encode(state)
}

pub fn discard_payload() -> Vec<u8> {
    super::compact_index::discard()
}
