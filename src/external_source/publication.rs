//! One atomic owner-local Memory publication and processing receipt.
//! Semantic Memory bodies are legitimate derivations, not external index text.
use super::model::*;
use crate::memory_model::{
    ExternalMemoryBinding, GroupedBodyEncoding, GroupedBodySpan, MemoryRecord,
};
use crate::{Container, Memory, MemoryBodyId, MemoryError, MemoryRoutingMetadata, ObjectRef};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
#[derive(Clone)]
pub(crate) struct PublicationBody {
    pub(crate) id: MemoryBodyId,
    pub(crate) bytes: Vec<u8>,
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
    bindings: Vec<ExternalMemoryBinding>,
    routing: Vec<Vec<u8>>,
}
pub(crate) struct Publication {
    pub window: [u8; 32],
    pub global_start: u64,
    pub transaction_ns: i64,
    pub birth_turn: Option<u64>,
    pub records: Vec<MemoryRecord>,
    pub bodies: Vec<PublicationBody>,
    pub bindings: Vec<ExternalMemoryBinding>,
    pub routing: Vec<MemoryRoutingMetadata>,
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
            .map(|body| WireBody {
                id: body.id.0,
                bytes: body.bytes.clone(),
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
    let bodies = wire
        .bodies
        .into_iter()
        .map(|b| PublicationBody {
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
    crate::container::grouped_clock::external_range(bytes)
        .map_err(|_| MemoryError::CorruptRecord("external publication"))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBodySpan<'a> {
    id: [u8; 32],
    #[serde(borrow)]
    bytes: &'a serde_json::value::RawValue,
}

#[derive(Deserialize)]
struct PublicationBodySpans<'a> {
    #[serde(borrow)]
    bodies: Vec<WireBodySpan<'a>>,
}

/// Return exact JSON byte-array source ranges for bodies in a validated frame.
pub(crate) fn body_spans(bytes: &[u8]) -> Result<Option<Vec<GroupedBodySpan>>, MemoryError> {
    if !bytes.starts_with(MAGIC) {
        return Ok(None);
    }
    if bytes.len() < 68 {
        return Err(MemoryError::CorruptRecord("external publication"));
    }
    let json = bytes
        .get(36..bytes.len() - 32)
        .ok_or(MemoryError::CorruptRecord("external publication"))?;
    let parsed: PublicationBodySpans<'_> = serde_json::from_slice(json)
        .map_err(|_| MemoryError::CorruptRecord("external publication"))?;
    let base = json.as_ptr() as usize;
    let mut spans = Vec::with_capacity(parsed.bodies.len());
    for body in parsed.bodies {
        let raw = body.bytes.get().as_bytes();
        let start = (raw.as_ptr() as usize)
            .checked_sub(base)
            .ok_or(MemoryError::CorruptRecord("external publication"))?;
        let end = start
            .checked_add(raw.len())
            .ok_or(MemoryError::CorruptRecord("external publication"))?;
        let frame_start = 36usize
            .checked_add(start)
            .ok_or(MemoryError::CorruptRecord("external publication"))?;
        let frame_end = 36usize
            .checked_add(end)
            .ok_or(MemoryError::CorruptRecord("external publication"))?;
        if json.get(start..end) != Some(raw) || frame_end > bytes.len() - 32 {
            return Err(MemoryError::CorruptRecord("external publication"));
        }
        spans.push(GroupedBodySpan {
            id: MemoryBodyId(body.id),
            range: frame_start..frame_end,
            encoding: GroupedBodyEncoding::JsonByteArray,
        });
    }
    Ok(Some(spans))
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
    let batch = memories.stage_grouped_publication(container, drafts)?;
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
            .map(|(m, r)| ExternalMemoryBinding {
                memory: r.id.0,
                body: r.body_id.0,
                evidence: m.evidence.clone(),
            })
            .collect(),
        records: batch.records,
        bodies: batch
            .bodies
            .into_iter()
            .map(|(id, bytes)| PublicationBody { id, bytes })
            .collect(),
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
    let body_spans = body_spans(&bytes)?.ok_or(MemoryError::CorruptBody)?;
    memories.apply_grouped_spans(object, &bytes, &body_spans)?;
    let results = memories.apply_grouped_records(container, &value.records)?;
    memories.apply_grouped_routing_metadata(&value.routing)?;
    memories.apply_external_bindings(value.window, &value.bindings)?;
    Ok(results)
}

#[cfg(test)]
mod body_span_tests {
    use super::*;

    #[test]
    fn body_spans_preserve_whitespace_inside_json_byte_arrays() {
        let id = [0x2a; 32];
        let array = format!(
            "[  {}  ]",
            (0..16).map(|_| "7").collect::<Vec<_>>().join(", ")
        );
        let json = format!(
            "{{\"bodies\":[{{\"id\":[{}],\"bytes\":{array}}}]}}",
            (0..32).map(|_| "42").collect::<Vec<_>>().join(",")
        );
        let mut frame = MAGIC.to_vec();
        frame.extend_from_slice(&(json.len() as u64).to_le_bytes());
        frame.extend_from_slice(&1u64.to_le_bytes());
        frame.extend_from_slice(&0u32.to_le_bytes());
        frame.extend_from_slice(&0i64.to_le_bytes());
        frame.extend_from_slice(json.as_bytes());
        frame.extend_from_slice(&Sha256::digest(&frame));
        let spans = body_spans(&frame).unwrap().unwrap();
        let start = frame
            .windows(array.len())
            .position(|window| window == array.as_bytes())
            .unwrap();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].id.0, id);
        assert_eq!(&frame[spans[0].range.clone()], array.as_bytes());
        assert_eq!(spans[0].range.start, start);
    }
}
