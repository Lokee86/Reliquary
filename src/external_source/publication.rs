//! One atomic owner-local Memory publication and processing receipt.
//! Semantic Memory bodies are legitimate derivations, not external index text.
use super::model::*;
use super::provenance::ExternalMemoryEvidence;
use crate::insomnia::completion::InsomniaCompletionBody;
use crate::memory_model::MemoryRecord;
use crate::{Memory, MemoryBodyId, MemoryError, MemoryId, MemoryRoutingMetadata};
use reliquary_container::{Container, ObjectRef};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ops::Range;
const MAGIC: &[u8; 8] = b"CVAXMEM2";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRecord {
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
struct WirePublication<R> {
    window: [u8; 32],
    global_start: u64,
    transaction_ns: i64,
    birth_turn: Option<u64>,
    records: Vec<R>,
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
    pub body_spans: Vec<crate::memory_rebuild::MemoryGroupedBodySpan>,
}

/// External Source's accepted Memory provenance and retry receipts. These facts
/// stay with the upper External Source owner; Memory Core stores only records.
#[derive(Default)]
pub(crate) struct ExternalPublicationState {
    evidence: HashMap<(crate::MemoryId, crate::MemoryBodyId), ExternalMemoryEvidence>,
    windows: HashMap<[u8; 32], Vec<crate::MemoryId>>,
}

impl ExternalPublicationState {
    pub(crate) fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn clear_windows(&mut self) {
        self.windows.clear();
    }

    pub(crate) fn evidence(
        &self,
        id: crate::MemoryId,
        body: crate::MemoryBodyId,
    ) -> Option<ExternalMemoryEvidence> {
        self.evidence.get(&(id, body)).cloned()
    }

    pub(crate) fn window(
        &self,
        container: &mut Container,
        memories: &crate::memory_store::MemoryStore,
        window: [u8; 32],
    ) -> Result<Option<Vec<Memory>>, MemoryError> {
        self.windows
            .get(&window)
            .map(|ids| {
                ids.iter()
                    .map(|id| memories.memory(container, *id))
                    .collect()
            })
            .transpose()
    }

    pub(crate) fn accept(&mut self, value: &Publication) -> Result<(), MemoryError> {
        if self.windows.contains_key(&value.window) {
            return Err(MemoryError::MutationConflict);
        }
        for binding in &value.bindings {
            self.evidence.insert(
                (
                    crate::MemoryId(binding.memory),
                    crate::MemoryBodyId(binding.body),
                ),
                binding.evidence.clone(),
            );
        }
        self.windows
            .insert(value.window, value.records.iter().map(|r| r.id).collect());
        Ok(())
    }
}
pub(crate) fn encode(value: &Publication) -> Result<Vec<u8>, MemoryError> {
    let records = value
        .records
        .iter()
        .map(|r| {
            Ok(WireRecord {
                bytes: crate::memory_compact_codec::encode_creation(
                    &crate::memory_compact_contract::MemoryCreation::new(r.clone())?,
                )?,
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
    let wire: WirePublication<MemoryRecord> = {
        let raw: WirePublication<WireRecord> =
            serde_json::from_slice(&bytes[36..bytes.len() - 32]).map_err(|_| invalid())?;
        let records = raw
            .records
            .into_iter()
            .map(|r| crate::memory_compact_codec::decode_creation(&r.bytes).map(|v| v.record))
            .collect::<Result<Vec<_>, MemoryError>>()?;
        WirePublication {
            window: raw.window,
            global_start: raw.global_start,
            transaction_ns: raw.transaction_ns,
            birth_turn: raw.birth_turn,
            records,
            bodies: raw.bodies,
            bindings: raw.bindings,
            routing: raw.routing,
        }
    };
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
    for (i, record) in wire.records.into_iter().enumerate() {
        if record.global_version
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
    let bodies: Vec<InsomniaCompletionBody> = wire
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
    let body_ranges = json_body_byte_array_ranges(&bytes[36..bytes.len() - 32])?;
    if body_ranges.len() != bodies.len() {
        return Err(invalid());
    }
    let body_spans = bodies
        .iter()
        .zip(body_ranges)
        .map(
            |(body, range)| crate::memory_rebuild::MemoryGroupedBodySpan {
                id: body.id,
                range: (range.start + 36)..(range.end + 36),
                encoding: crate::memory_rebuild::MemoryGroupedBodyEncoding::JsonByteArray,
            },
        )
        .collect();
    Ok(Some(Publication {
        window: wire.window,
        global_start: wire.global_start,
        transaction_ns: wire.transaction_ns,
        birth_turn: wire.birth_turn,
        records,
        bodies,
        bindings: wire.bindings,
        routing,
        body_spans,
    }))
}

/// Finds the exact JSON byte-array tokens used for grouped bodies. The normal
/// serde decode above validates the carrier; this scanner only records spans
/// into its original bytes so replay need not retain a second body copy.
fn json_body_byte_array_ranges(json: &[u8]) -> Result<Vec<Range<usize>>, MemoryError> {
    let invalid = || MemoryError::CorruptRecord("external publication");
    let mut cursor = skip_json_ws(json, 0);
    if json.get(cursor) != Some(&b'{') {
        return Err(invalid());
    }
    cursor += 1;
    loop {
        cursor = skip_json_ws(json, cursor);
        if json.get(cursor) == Some(&b'}') {
            return Err(invalid());
        }
        let (key, next) = json_string(json, cursor).ok_or_else(invalid)?;
        cursor = skip_json_ws(json, next);
        if json.get(cursor) != Some(&b':') {
            return Err(invalid());
        }
        cursor = skip_json_ws(json, cursor + 1);
        if key == "bodies" {
            return json_body_ranges(json, cursor);
        }
        cursor = json_value_end(json, cursor).ok_or_else(invalid)?;
        cursor = skip_json_ws(json, cursor);
        match json.get(cursor) {
            Some(b',') => cursor += 1,
            Some(b'}') => return Err(invalid()),
            _ => return Err(invalid()),
        }
    }
}

fn json_body_ranges(json: &[u8], mut cursor: usize) -> Result<Vec<Range<usize>>, MemoryError> {
    let invalid = || MemoryError::CorruptRecord("external publication");
    if json.get(cursor) != Some(&b'[') {
        return Err(invalid());
    }
    cursor += 1;
    let mut ranges = Vec::new();
    loop {
        cursor = skip_json_ws(json, cursor);
        if json.get(cursor) == Some(&b']') {
            return Ok(ranges);
        }
        if json.get(cursor) != Some(&b'{') {
            return Err(invalid());
        }
        cursor += 1;
        let mut body_range = None;
        loop {
            cursor = skip_json_ws(json, cursor);
            let (key, next) = json_string(json, cursor).ok_or_else(invalid)?;
            cursor = skip_json_ws(json, next);
            if json.get(cursor) != Some(&b':') {
                return Err(invalid());
            }
            cursor = skip_json_ws(json, cursor + 1);
            let end = json_value_end(json, cursor).ok_or_else(invalid)?;
            if key == "bytes" {
                if json.get(cursor) != Some(&b'[') {
                    return Err(invalid());
                }
                body_range = Some(cursor..end);
            }
            cursor = skip_json_ws(json, end);
            match json.get(cursor) {
                Some(b',') => cursor += 1,
                Some(b'}') => {
                    cursor += 1;
                    break;
                }
                _ => return Err(invalid()),
            }
        }
        ranges.push(body_range.ok_or_else(invalid)?);
        cursor = skip_json_ws(json, cursor);
        match json.get(cursor) {
            Some(b',') => cursor += 1,
            Some(b']') => return Ok(ranges),
            _ => return Err(invalid()),
        }
    }
}

fn skip_json_ws(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes
        .get(cursor)
        .is_some_and(|byte| matches!(byte, b' ' | b'\n' | b'\r' | b'\t'))
    {
        cursor += 1;
    }
    cursor
}

fn json_string(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor = cursor.checked_add(2)?,
            b'"' => {
                let end = cursor + 1;
                return serde_json::from_slice(&bytes[start..end])
                    .ok()
                    .map(|value| (value, end));
            }
            _ => cursor += 1,
        }
    }
    None
}

fn json_value_end(bytes: &[u8], start: usize) -> Option<usize> {
    match *bytes.get(start)? {
        b'"' => json_string(bytes, start).map(|(_, end)| end),
        b'[' => {
            let mut cursor = start + 1;
            loop {
                cursor = skip_json_ws(bytes, cursor);
                if bytes.get(cursor) == Some(&b']') {
                    return Some(cursor + 1);
                }
                cursor = json_value_end(bytes, cursor)?;
                cursor = skip_json_ws(bytes, cursor);
                match bytes.get(cursor)? {
                    b',' => cursor += 1,
                    b']' => return Some(cursor + 1),
                    _ => return None,
                }
            }
        }
        b'{' => {
            let mut cursor = start + 1;
            loop {
                cursor = skip_json_ws(bytes, cursor);
                if bytes.get(cursor) == Some(&b'}') {
                    return Some(cursor + 1);
                }
                let (_, end) = json_string(bytes, cursor)?;
                cursor = skip_json_ws(bytes, end);
                if bytes.get(cursor) != Some(&b':') {
                    return None;
                }
                cursor = skip_json_ws(bytes, cursor + 1);
                cursor = json_value_end(bytes, cursor)?;
                cursor = skip_json_ws(bytes, cursor);
                match bytes.get(cursor)? {
                    b',' => cursor += 1,
                    b'}' => return Some(cursor + 1),
                    _ => return None,
                }
            }
        }
        _ => {
            let mut cursor = start;
            while bytes
                .get(cursor)
                .is_some_and(|b| !matches!(b, b',' | b']' | b'}' | b' ' | b'\n' | b'\r' | b'\t'))
            {
                cursor += 1;
            }
            (cursor > start).then_some(cursor)
        }
    }
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
pub(crate) fn publish(
    container: &mut Container,
    memories: &mut crate::memory_store::MemoryStore,
    external: &mut ExternalPublicationState,
    window: [u8; 32],
    prepared: &[super::processing::PreparedExternalMemory],
    birth_turn: Option<u64>,
) -> Result<Vec<Memory>, MemoryError> {
    if let Some(existing) = external.window(container, memories, window)? {
        return Ok(existing);
    }
    let drafts = prepared
        .iter()
        .map(|m| (m.draft.clone(), m.temporal.clone(), m.routing.clone()))
        .collect();
    let batch = memories.stage_grouped(container, drafts)?;
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
        body_spans: Vec::new(),
    };
    let bytes = encode(&value)?;
    let body_spans = decode(&bytes)?
        .ok_or(MemoryError::CorruptRecord("external publication"))?
        .body_spans;
    let object: ObjectRef = container.append(&bytes)?;
    container.commit_embedded_version_range_at(
        value.global_start,
        value.records.len(),
        Some(value.transaction_ns),
    )?;
    container.sync()?;
    memories.apply_grouped_batch(
        object,
        &bytes,
        crate::memory_rebuild::MemoryGroupedBatch::new(
            value.records.clone(),
            body_spans,
            value.routing.clone(),
        ),
        container.latest_version(),
    )?;
    let results = value
        .records
        .iter()
        .map(|record| memories.memory(container, record.id))
        .collect::<Result<Vec<_>, _>>()?;
    external.accept(&value)?;
    Ok(results)
}
