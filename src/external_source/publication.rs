//! One atomic owner-local Memory publication and processing receipt.
//! Semantic Memory bodies are legitimate derivations, not external index text.
use super::model::*;
use super::provenance::ExternalMemoryEvidence;
use crate::insomnia::completion::InsomniaCompletionBody;
use crate::memory_model::MemoryRecord;
use crate::{
    Container, Memory, MemoryBodyId, MemoryError, MemoryId, MemoryRoutingMetadata, ObjectRef,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
const MAGIC: &[u8; 8] = b"CVAXMEM1";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRecord {
    global: u64,
    memory: u64,
    bytes: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBody {
    id: [u8; 32],
    bytes: Vec<u8>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    pub memory: [u8; 32],
    pub body: [u8; 32],
    pub evidence: ExternalMemoryEvidence,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePublication {
    window: [u8; 32],
    global_start: u64,
    transaction_ns: i64,
    birth_turn: Option<u64>,
    records: Vec<WireRecord>,
    bodies: Vec<WireBody>,
    bindings: Vec<Binding>,
    routing: Vec<Vec<u8>>,
}
pub(crate) struct Publication {
    pub window: [u8; 32],
    pub global_start: u64,
    pub transaction_ns: i64,
    pub birth_turn: Option<u64>,
    pub records: Vec<MemoryRecord>,
    pub bodies: Vec<InsomniaCompletionBody>,
    pub bindings: Vec<Binding>,
    pub routing: Vec<MemoryRoutingMetadata>,
}
pub(crate) fn encode(value: &Publication) -> Result<Vec<u8>, MemoryError> {
    let records = value
        .records
        .iter()
        .map(|r| {
            Ok(WireRecord {
                global: r.global_version,
                memory: r.memory_version,
                bytes: crate::memory_codec::encode_record(r)?,
            })
        })
        .collect::<Result<Vec<_>, MemoryError>>()?;
    let routing = value
        .routing
        .iter()
        .map(crate::memory_routing_codec::encode)
        .collect::<Result<Vec<_>, _>>()?;
    let wire = WirePublication {
        window: value.window,
        global_start: value.global_start,
        transaction_ns: value.transaction_ns,
        birth_turn: value.birth_turn,
        records,
        bodies: value
            .bodies
            .iter()
            .map(|b| WireBody {
                id: b.id.0,
                bytes: b.bytes.clone(),
            })
            .collect(),
        bindings: value.bindings.clone(),
        routing,
    };
    let json = serde_json::to_vec(&wire)
        .map_err(|_| MemoryError::CorruptRecord("external publication"))?;
    if json.len() > MAX_EXTERNAL_INDEX_BYTES {
        return Err(MemoryError::FieldTooLarge);
    }
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&(json.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&value.global_start.to_le_bytes());
    bytes.extend_from_slice(&(value.records.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&value.transaction_ns.to_le_bytes());
    bytes.extend_from_slice(&json);
    bytes.extend_from_slice(&Sha256::digest(&bytes));
    Ok(bytes)
}
pub(crate) fn decode(bytes: &[u8]) -> Result<Option<Publication>, MemoryError> {
    if !bytes.starts_with(MAGIC) {
        return Ok(None);
    }
    let invalid = || MemoryError::CorruptRecord("external publication");
    let (start, count, transaction_ns) =
        decode_embedded_version_range(bytes)?.ok_or_else(invalid)?;
    let wire: WirePublication =
        serde_json::from_slice(&bytes[36..bytes.len() - 32]).map_err(|_| invalid())?;
    if wire.global_start != start
        || wire.records.len() != count as usize
        || wire.transaction_ns != transaction_ns
    {
        return Err(invalid());
    }
    if wire.records.len() > 128 || wire.bindings.len() != wire.records.len() {
        return Err(invalid());
    }
    let mut records = Vec::new();
    for (i, r) in wire.records.into_iter().enumerate() {
        let mut record = crate::memory_codec::decode_record(&r.bytes)?.ok_or_else(invalid)?;
        if r.global
            != wire
                .global_start
                .checked_add(i as u64)
                .ok_or_else(invalid)?
            || record.revision != 1
            || record.content_source_conversation_id.is_some()
            || record.grounding_source_conversation_id.is_some()
            || record.source_episode_id.is_some()
            || record.source_node_id.is_some()
            || record.content_source_node_id.is_some()
            || record.grounding_source_node_id.is_some()
            || record.source_ref.is_some()
        {
            return Err(invalid());
        }
        record.global_version = r.global;
        record.memory_version = r.memory;
        let binding = &wire.bindings[i];
        binding.evidence.validate().map_err(|_| invalid())?;
        if binding.memory != record.id.0
            || binding.body != record.body_id.0
            || binding.evidence.processing_window != wire.window
        {
            return Err(invalid());
        }
        records.push(record);
    }
    let bodies = wire
        .bodies
        .into_iter()
        .map(|b| InsomniaCompletionBody {
            id: MemoryBodyId(b.id),
            bytes: b.bytes,
        })
        .collect();
    let routing = wire
        .routing
        .into_iter()
        .map(|r| crate::memory_routing_codec::decode(&r)?.ok_or_else(invalid))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(Publication {
        window: wire.window,
        global_start: wire.global_start,
        transaction_ns: wire.transaction_ns,
        birth_turn: wire.birth_turn,
        records,
        bodies,
        bindings: wire.bindings,
        routing,
    }))
}
pub(crate) fn decode_embedded_version_range(
    bytes: &[u8],
) -> Result<Option<(u64, u32, i64)>, MemoryError> {
    if !bytes.starts_with(MAGIC) {
        return Ok(None);
    }
    let invalid = || MemoryError::CorruptRecord("external publication");
    if bytes.len() < 68 || bytes.len() > MAX_EXTERNAL_INDEX_BYTES + 68 {
        return Err(invalid());
    }
    let len = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    if usize::try_from(len).ok().and_then(|n| n.checked_add(68)) != Some(bytes.len())
        || Sha256::digest(&bytes[..bytes.len() - 32]).as_slice() != &bytes[bytes.len() - 32..]
    {
        return Err(invalid());
    }
    let count = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
    if count > 128 {
        return Err(invalid());
    }
    Ok(Some((
        u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
        count,
        i64::from_le_bytes(bytes[28..36].try_into().unwrap()),
    )))
}
impl crate::memory_store::MemoryStore {
    pub(crate) fn external_window(
        &self,
        container: &mut Container,
        window: [u8; 32],
    ) -> Result<Option<Vec<Memory>>, MemoryError> {
        self.external_windows
            .get(&window)
            .map(|ids| ids.iter().map(|id| self.memory(container, *id)).collect())
            .transpose()
    }
    pub(crate) fn apply_external_publication(
        &mut self,
        value: &Publication,
    ) -> Result<(), MemoryError> {
        if self.external_windows.contains_key(&value.window) {
            return Err(MemoryError::MutationConflict);
        }
        for binding in &value.bindings {
            self.external_evidence.insert(
                (MemoryId(binding.memory), MemoryBodyId(binding.body)),
                binding.evidence.clone(),
            );
        }
        self.external_windows
            .insert(value.window, value.records.iter().map(|r| r.id).collect());
        Ok(())
    }
}
pub(crate) fn publish(
    container: &mut Container,
    memories: &mut crate::memory_store::MemoryStore,
    window: [u8; 32],
    prepared: &[super::processing::PreparedExternalMemory],
    birth_turn: Option<u64>,
) -> Result<Vec<Memory>, MemoryError> {
    if let Some(existing) = memories.external_window(container, window)? {
        return Ok(existing);
    }
    let drafts = prepared
        .iter()
        .map(|m| (m.draft.clone(), m.temporal.clone(), m.routing.clone()))
        .collect();
    let batch = memories.stage_grouped_insomnia(container, drafts)?;
    // All accepted external work has an owner-local atomic receipt; loose mutations cannot impersonate it.
    if !batch.existing.is_empty() {
        return Err(MemoryError::MutationConflict);
    }
    let value = Publication {
        window,
        global_start: batch.global_version_start,
        transaction_ns: Container::transaction_time_now_ns()?,
        birth_turn,
        bindings: prepared
            .iter()
            .zip(&batch.records)
            .map(|(m, r)| Binding {
                memory: r.id.0,
                body: r.body_id.0,
                evidence: m.evidence.clone(),
            })
            .collect(),
        records: batch.records,
        bodies: batch.bodies,
        routing: batch.routing_metadata,
    };
    let bytes = encode(&value)?;
    let object: ObjectRef = container.append(&bytes)?;
    container.commit_embedded_version_range_at(
        value.global_start,
        value.records.len(),
        Some(value.transaction_ns),
    )?;
    container.sync()?;
    memories.apply_grouped_bodies(object, &value.bodies)?;
    let results = memories.apply_grouped_records(container, &value.records)?;
    memories.apply_grouped_routing_metadata(&value.routing)?;
    memories.apply_external_publication(&value)?;
    Ok(results)
}
