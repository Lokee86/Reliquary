//! Recoverable external-source Memory publication for REL owners.
use super::external_index::model::{MAX_EXTERNAL_INDEX_BYTES, MAX_EXTERNAL_MESSAGE_BYTES};
use super::external_index::{ExternalChunkRef, ExternalSourceError};
use crate::model::MemoryDraft;
use reliquary_container::{Container, FileKind};
use reliquary_memory_core::{MemoryId, MemoryStore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const INTENT: &[u8; 8] = b"RELXMEM3";
const RECEIPT: &[u8; 8] = b"RELXMRP3";
const MAX_ITEMS: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalMemoryDraft {
    pub draft: MemoryDraft,
    pub authority_source: ExternalChunkRef,
    pub content_sources: Vec<ExternalChunkRef>,
    pub grounding_sources: Vec<ExternalChunkRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalMemoryReceipt {
    owner_id: [u8; 16],
    principal: String,
    window: [u8; 32],
    memory_ids: Vec<MemoryId>,
    sources: Vec<ExternalMemoryProvenance>,
    digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExternalMemoryProvenance {
    pub authority_source: ExternalChunkRef,
    pub content_sources: Vec<ExternalChunkRef>,
    pub grounding_sources: Vec<ExternalChunkRef>,
}

impl ExternalMemoryReceipt {
    pub fn owner_id(&self) -> [u8; 16] {
        self.owner_id
    }
    pub fn principal(&self) -> &str {
        &self.principal
    }
    pub fn window(&self) -> [u8; 32] {
        self.window
    }
    pub fn memory_ids(&self) -> &[MemoryId] {
        &self.memory_ids
    }
    pub fn sources(&self) -> &[ExternalMemoryProvenance] {
        &self.sources
    }
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DraftWire {
    title: String,
    content: String,
    category: String,
    memory_type: String,
    authority_kind: String,
    temporal_status: String,
    scope: String,
    lifecycle_state: String,
    archived: bool,
    mutation_id: String,
    created_at_ns: i64,
    updated_at_ns: i64,
    source_time_ns: Option<i64>,
}

impl TryFrom<&MemoryDraft> for DraftWire {
    type Error = ExternalSourceError;
    fn try_from(d: &MemoryDraft) -> Result<Self, Self::Error> {
        if d.parent_id.is_some()
            || d.superseded_by.is_some()
            || d.source_node_id.is_some()
            || d.content_source_conversation_id.is_some()
            || d.content_source_node_id.is_some()
            || d.grounding_source_conversation_id.is_some()
            || d.grounding_source_node_id.is_some()
            || d.source_episode_id.is_some()
            || d.source_ref.is_some()
            || d.temporal_inference.is_some()
        {
            return Err(ExternalSourceError::InvalidContract);
        }
        Ok(Self {
            title: d.title.clone(),
            content: d.content.clone(),
            category: d.category.clone(),
            memory_type: d.memory_type.clone(),
            authority_kind: d.authority_kind.clone(),
            temporal_status: d.temporal_status.clone(),
            scope: d.scope.clone(),
            lifecycle_state: d.lifecycle_state.clone(),
            archived: d.archived,
            mutation_id: d.mutation_id.clone(),
            created_at_ns: d.created_at_ns,
            updated_at_ns: d.updated_at_ns,
            source_time_ns: d.source_time_ns,
        })
    }
}
impl From<DraftWire> for MemoryDraft {
    fn from(d: DraftWire) -> Self {
        Self {
            title: d.title,
            content: d.content,
            category: d.category,
            memory_type: d.memory_type,
            authority_kind: d.authority_kind,
            temporal_status: d.temporal_status,
            scope: d.scope,
            lifecycle_state: d.lifecycle_state,
            archived: d.archived,
            parent_id: None,
            superseded_by: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_ref: None,
            source_time_ns: d.source_time_ns,
            temporal_inference: None,
            mutation_id: d.mutation_id,
            created_at_ns: d.created_at_ns,
            updated_at_ns: d.updated_at_ns,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ChunkWire {
    reference: ExternalChunkRef,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct EntryWire {
    draft: DraftWire,
    authority: ChunkWire,
    content: Vec<ChunkWire>,
    grounding: Vec<ChunkWire>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct IntentWire {
    version: u32,
    owner: [u8; 16],
    principal: String,
    window: [u8; 32],
    entries: Vec<EntryWire>,
    digest: [u8; 32],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ReceiptWire {
    version: u32,
    owner: [u8; 16],
    principal: String,
    window: [u8; 32],
    memory_ids: Vec<[u8; 32]>,
    sources: Vec<ExternalMemoryProvenance>,
    digest: [u8; 32],
}

impl IntentWire {
    pub(crate) fn digest(&self) -> [u8; 32] {
        self.digest
    }
    fn new(
        owner: [u8; 16],
        principal: &str,
        window: [u8; 32],
        entries: &[ExternalMemoryDraft],
    ) -> Result<Self, ExternalSourceError> {
        if entries.len() > MAX_ITEMS {
            return Err(ExternalSourceError::Bounds);
        }
        super::external_index::model::identifier(principal)?;
        let mut mutations = std::collections::BTreeSet::new();
        for entry in entries {
            reliquary_memory_core::validate_memory_draft(&entry.draft)
                .map_err(|_| ExternalSourceError::InvalidContract)?;
            if entry.draft.scope != "project"
                || entry.draft.archived
                || entry.draft.lifecycle_state != "extracted"
                || !mutations.insert(&entry.draft.mutation_id)
                || entry.content_sources.len() > 1
                || entry.grounding_sources.len() > 1
            {
                return Err(ExternalSourceError::InvalidContract);
            }
            for chunk in std::iter::once(&entry.authority_source)
                .chain(&entry.content_sources)
                .chain(&entry.grounding_sources)
            {
                chunk.message.validate()?;
                if chunk.start >= chunk.end || chunk.end > MAX_EXTERNAL_MESSAGE_BYTES as u64 {
                    return Err(ExternalSourceError::InvalidContract);
                }
            }
        }
        let mut wire = Self {
            version: 1,
            owner,
            principal: principal.to_owned(),
            window,
            entries: entries
                .iter()
                .map(|e| {
                    Ok(EntryWire {
                        draft: DraftWire::try_from(&e.draft)?,
                        authority: ChunkWire {
                            reference: e.authority_source.clone(),
                        },
                        content: e
                            .content_sources
                            .iter()
                            .cloned()
                            .map(|reference| ChunkWire { reference })
                            .collect(),
                        grounding: e
                            .grounding_sources
                            .iter()
                            .cloned()
                            .map(|reference| ChunkWire { reference })
                            .collect(),
                    })
                })
                .collect::<Result<_, ExternalSourceError>>()?,
            digest: [0; 32],
        };
        wire.digest = wire.hash()?;
        Ok(wire)
    }
    fn hash(&self) -> Result<[u8; 32], ExternalSourceError> {
        let mut v = self.clone();
        v.digest = [0; 32];
        let b = serde_json::to_vec(&v).map_err(|_| ExternalSourceError::InvalidContract)?;
        Ok(Sha256::digest(b).into())
    }
    fn encode(&self) -> Result<Vec<u8>, ExternalSourceError> {
        let b = serde_json::to_vec(self).map_err(|_| ExternalSourceError::InvalidContract)?;
        if b.len() + 8 > MAX_EXTERNAL_INDEX_BYTES {
            return Err(ExternalSourceError::Bounds);
        }
        let mut out = INTENT.to_vec();
        out.extend(b);
        Ok(out)
    }
    fn decode(b: &[u8]) -> Result<Self, ExternalSourceError> {
        if b.len() > MAX_EXTERNAL_INDEX_BYTES || !b.starts_with(INTENT) {
            return Err(ExternalSourceError::Bounds);
        }
        let v: Self =
            serde_json::from_slice(&b[8..]).map_err(|_| ExternalSourceError::InvalidContract)?;
        if v.version != 1 || v.digest != v.hash()? || v.entries.len() > MAX_ITEMS {
            return Err(ExternalSourceError::InvalidContract);
        }
        let validated = Self::new(v.owner, &v.principal, v.window, &v.entries())?;
        if validated.digest != v.digest {
            return Err(ExternalSourceError::InvalidContract);
        }
        Ok(v)
    }
    fn entries(&self) -> Vec<ExternalMemoryDraft> {
        self.entries
            .iter()
            .map(|e| ExternalMemoryDraft {
                draft: e.draft.clone().into(),
                authority_source: e.authority.reference.clone(),
                content_sources: e.content.iter().map(|c| c.reference.clone()).collect(),
                grounding_sources: e.grounding.iter().map(|c| c.reference.clone()).collect(),
            })
            .collect()
    }
}

mod store;
pub(crate) use store::ExternalMemoryStore;

fn memories_match(i: &IntentWire, m: &MemoryStore, ids: &[MemoryId]) -> bool {
    let entries = i.entries();
    entries.len() == ids.len()
        && entries
            .iter()
            .zip(ids)
            .all(|(e, id)| memory_matches_entry(m, *id, &e.draft))
}

fn memory_matches_entry(m: &MemoryStore, id: MemoryId, d: &MemoryDraft) -> bool {
    if reliquary_memory_core::memory_id(&d.mutation_id) != id {
        return false;
    }
    let Ok(history) = m.history(id) else {
        return false;
    };
    let Some(first) = history.revisions.first() else {
        return false;
    };
    let r = &first.record;
    r.revision == 1
        && r.id == id
        && r.body_id == reliquary_memory_core::memory_body_id(&d.title, &d.content)
        && r.mutation_id == d.mutation_id
        && r.category == d.category
        && r.memory_type == d.memory_type
        && r.authority_kind == d.authority_kind
        && r.temporal_status == d.temporal_status
        && r.scope == d.scope
        && r.lifecycle_state == d.lifecycle_state
        && r.archived == d.archived
        && r.source_time_ns == d.source_time_ns
        && r.created_at_ns == d.created_at_ns
        && r.updated_at_ns == d.updated_at_ns
        && r.temporal_inference.is_none()
        && r.parent_id.is_none()
        && r.superseded_by.is_none()
        && r.source_ref.is_none()
        && r.source_node_id.is_none()
        && r.source_episode_id.is_none()
        && r.content_source_node_id.is_none()
        && r.content_source_conversation_id.is_none()
        && r.grounding_source_node_id.is_none()
        && r.grounding_source_conversation_id.is_none()
}
fn encode_receipt(r: &ExternalMemoryReceipt) -> Result<Vec<u8>, ExternalSourceError> {
    let w = ReceiptWire {
        version: 1,
        owner: r.owner_id,
        principal: r.principal.clone(),
        window: r.window,
        memory_ids: r.memory_ids.iter().map(|i| i.0).collect(),
        sources: r.sources.clone(),
        digest: r.digest,
    };
    let b = serde_json::to_vec(&w).map_err(|_| ExternalSourceError::InvalidContract)?;
    if b.len() + 8 > MAX_EXTERNAL_INDEX_BYTES {
        return Err(ExternalSourceError::Bounds);
    }
    let mut o = RECEIPT.to_vec();
    o.extend(b);
    Ok(o)
}
fn decode_receipt(b: &[u8]) -> Result<ExternalMemoryReceipt, ExternalSourceError> {
    if b.len() > MAX_EXTERNAL_INDEX_BYTES || !b.starts_with(RECEIPT) {
        return Err(ExternalSourceError::Bounds);
    }
    let w: ReceiptWire =
        serde_json::from_slice(&b[8..]).map_err(|_| ExternalSourceError::InvalidContract)?;
    if w.version != 1 || w.memory_ids.len() > MAX_ITEMS || w.sources.len() != w.memory_ids.len() {
        return Err(ExternalSourceError::InvalidContract);
    }
    Ok(ExternalMemoryReceipt {
        owner_id: w.owner,
        principal: w.principal,
        window: w.window,
        memory_ids: w.memory_ids.into_iter().map(MemoryId).collect(),
        sources: w.sources,
        digest: w.digest,
    })
}

pub(crate) fn new_intent(
    owner: [u8; 16],
    p: &str,
    w: [u8; 32],
    entries: &[ExternalMemoryDraft],
) -> Result<IntentWire, ExternalSourceError> {
    IntentWire::new(owner, p, w, entries)
}
