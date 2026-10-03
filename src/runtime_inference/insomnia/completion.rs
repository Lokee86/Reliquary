use crate::EpisodeId;
use crate::memory_codec::{decode_record, encode_record};
use crate::memory_model::MemoryRecord;
use crate::{MemoryBodyId, MemoryRef, MemoryRoutingMetadata};

const COMPLETION_MAGIC_V1: [u8; 8] = *b"CVAINSC1";
const COMPLETION_MAGIC_V2: [u8; 8] = *b"CVAINSC2";
const COMPLETION_MAGIC_V3: [u8; 8] = *b"CVAINSC3";
const COMPLETION_MAGIC_V4: [u8; 8] = *b"CVAINSC4";
const COMPLETION_MAGIC_V5: [u8; 8] = *b"CVAINSC5";
const COMPLETION_MAGIC_V6: [u8; 8] = *b"CVAINSC6";

#[derive(Clone, Debug)]
pub(crate) struct InsomniaCompletionBody {
    pub(crate) id: MemoryBodyId,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct InsomniaCompletion {
    pub(crate) episode_id: EpisodeId,
    pub(crate) attempt: u32,
    pub(crate) started_at_ns: i64,
    pub(crate) completed_at_ns: i64,
    pub(crate) transaction_time_ns: i64,
    pub(crate) extractor_model: String,
    pub(crate) extractor_version: String,
    pub(crate) rejected_count: u32,
    pub(crate) memory_ids: Vec<crate::MemoryId>,
    pub(crate) external_memory_refs: Vec<MemoryRef>,
    pub(crate) global_version_start: u64,
    pub(crate) bodies: Vec<InsomniaCompletionBody>,
    pub(crate) records: Vec<MemoryRecord>,
    pub(crate) routing_metadata: Vec<MemoryRoutingMetadata>,
    pub(crate) freshness_birth_turn: Option<u64>,
    pub(crate) freshness_policy_version: Option<u64>,
    pub(crate) freshness_birth_memory_ids: Vec<crate::MemoryId>,
}

pub(crate) fn encode_completion(value: &InsomniaCompletion) -> Result<Vec<u8>, &'static str> {
    let global_version_count =
        u32::try_from(value.records.len()).map_err(|_| "too many completion versions")?;
    if global_version_count > 0 && value.global_version_start == 0 {
        return Err("invalid completion global version start");
    }
    let mut out = Vec::with_capacity(
        192 + value.records.len() * 256
            + value
                .bodies
                .iter()
                .map(|body| body.bytes.len())
                .sum::<usize>(),
    );
    let has_birth_proof =
        value.freshness_birth_turn.is_some() && value.freshness_policy_version.is_some();
    if value.freshness_birth_turn.is_some() != value.freshness_policy_version.is_some()
        || (!has_birth_proof && !value.freshness_birth_memory_ids.is_empty())
    {
        return Err("incomplete Freshness birth proof");
    }
    if has_birth_proof {
        if value.freshness_policy_version != Some(crate::freshness::FRESHNESS_POLICY_VERSION as u64)
        {
            return Err("unsupported Freshness birth policy");
        }
        let expected = value
            .records
            .iter()
            .filter(|record| record.revision == 1)
            .map(|record| record.id)
            .collect::<std::collections::BTreeSet<_>>();
        let actual = value
            .freshness_birth_memory_ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if actual.len() != value.freshness_birth_memory_ids.len() || actual != expected {
            return Err("Freshness birth cohort differs from exact first-revision records");
        }
    }
    out.extend_from_slice(if has_birth_proof {
        &COMPLETION_MAGIC_V6
    } else {
        &COMPLETION_MAGIC_V5
    });
    out.extend_from_slice(&value.episode_id.0);
    out.extend_from_slice(&value.attempt.to_le_bytes());
    out.extend_from_slice(&value.started_at_ns.to_le_bytes());
    out.extend_from_slice(&value.completed_at_ns.to_le_bytes());
    out.extend_from_slice(&value.rejected_count.to_le_bytes());
    out.extend_from_slice(&value.global_version_start.to_le_bytes());
    out.extend_from_slice(&global_version_count.to_le_bytes());
    out.extend_from_slice(&value.transaction_time_ns.to_le_bytes());
    write_string(&mut out, &value.extractor_model)?;
    write_string(&mut out, &value.extractor_version)?;
    let memory_count =
        u32::try_from(value.memory_ids.len()).map_err(|_| "too many completion memory ids")?;
    out.extend_from_slice(&memory_count.to_le_bytes());
    for id in &value.memory_ids {
        out.extend_from_slice(&id.0);
    }
    let external_count = u32::try_from(value.external_memory_refs.len())
        .map_err(|_| "too many completion external memory refs")?;
    out.extend_from_slice(&external_count.to_le_bytes());
    for memory_ref in &value.external_memory_refs {
        if memory_ref.owner_id.trim().is_empty() {
            return Err("empty external memory owner id");
        }
        write_string(&mut out, &memory_ref.owner_id)?;
        out.extend_from_slice(&memory_ref.memory_id.0);
    }
    let body_count = u32::try_from(value.bodies.len()).map_err(|_| "too many completion bodies")?;
    out.extend_from_slice(&body_count.to_le_bytes());
    for body in &value.bodies {
        out.extend_from_slice(&body.id.0);
        let len = u32::try_from(body.bytes.len()).map_err(|_| "completion body too large")?;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&body.bytes);
    }
    let count = u32::try_from(value.records.len()).map_err(|_| "too many completion records")?;
    out.extend_from_slice(&count.to_le_bytes());
    for (index, record) in value.records.iter().enumerate() {
        let expected_global = value
            .global_version_start
            .checked_add(u64::try_from(index).map_err(|_| "completion version overflow")?)
            .ok_or("completion version overflow")?;
        if record.global_version != expected_global {
            return Err("non-contiguous completion global versions");
        }
        out.extend_from_slice(&record.global_version.to_le_bytes());
        out.extend_from_slice(&record.memory_version.to_le_bytes());
        let encoded = encode_record(record).map_err(|_| "invalid completion memory record")?;
        let len = u32::try_from(encoded.len()).map_err(|_| "completion record too large")?;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&encoded);
    }
    if value.routing_metadata.len() > super::contract::MAX_INSOMNIA_CANDIDATES {
        return Err("too many completion routing metadata records");
    }
    let routing_count = u32::try_from(value.routing_metadata.len())
        .map_err(|_| "too many completion routing metadata records")?;
    out.extend_from_slice(&routing_count.to_le_bytes());
    for metadata in &value.routing_metadata {
        let encoded = crate::memory_routing_codec::encode(metadata)
            .map_err(|_| "invalid completion routing metadata")?;
        let len =
            u32::try_from(encoded.len()).map_err(|_| "completion routing metadata too large")?;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&encoded);
    }
    if has_birth_proof {
        out.extend_from_slice(&value.freshness_birth_turn.unwrap().to_le_bytes());
        out.extend_from_slice(&value.freshness_policy_version.unwrap().to_le_bytes());
        let count = u32::try_from(value.freshness_birth_memory_ids.len())
            .map_err(|_| "too many Freshness birth IDs")?;
        out.extend_from_slice(&count.to_le_bytes());
        for id in &value.freshness_birth_memory_ids {
            out.extend_from_slice(&id.0);
        }
    }
    Ok(out)
}

pub(crate) fn decode_embedded_version_range(
    bytes: &[u8],
) -> Result<Option<(u64, u32, Option<i64>)>, &'static str> {
    if bytes.len() < 8
        || (bytes[..8] != COMPLETION_MAGIC_V2
            && bytes[..8] != COMPLETION_MAGIC_V3
            && bytes[..8] != COMPLETION_MAGIC_V4
            && bytes[..8] != COMPLETION_MAGIC_V5
            && bytes[..8] != COMPLETION_MAGIC_V6)
    {
        return Ok(None);
    }
    if bytes.len() < 76 {
        return Err("short Insomnia completion");
    }
    let start = u64::from_le_bytes(bytes[64..72].try_into().unwrap());
    let count = u32::from_le_bytes(bytes[72..76].try_into().unwrap());
    if count > 0 && start == 0 {
        return Err("invalid completion global version range");
    }
    let transaction_time_ns = if bytes[..8] == COMPLETION_MAGIC_V4
        || bytes[..8] == COMPLETION_MAGIC_V5
        || bytes[..8] == COMPLETION_MAGIC_V6
    {
        if bytes.len() < 84 {
            return Err("short Insomnia completion with transaction time");
        }
        Some(i64::from_le_bytes(bytes[76..84].try_into().unwrap()))
    } else {
        None
    };
    Ok(Some((start, count, transaction_time_ns)))
}

pub(crate) fn decode_completion(bytes: &[u8]) -> Result<Option<InsomniaCompletion>, &'static str> {
    if bytes.len() < 8 {
        return Ok(None);
    }
    if bytes[..8] == COMPLETION_MAGIC_V6 {
        return decode_completion_current(bytes, true, true).map(Some);
    }
    if bytes[..8] == COMPLETION_MAGIC_V5 {
        return decode_completion_current(bytes, true, false).map(Some);
    }
    if bytes[..8] == COMPLETION_MAGIC_V4 {
        return decode_completion_current(bytes, false, false).map(Some);
    }
    if bytes[..8] == COMPLETION_MAGIC_V3 {
        return decode_completion_v3(bytes).map(Some);
    }
    if bytes[..8] == COMPLETION_MAGIC_V2 {
        return decode_completion_v2(bytes).map(Some);
    }
    if bytes[..8] == COMPLETION_MAGIC_V1 {
        return decode_completion_v1(bytes).map(Some);
    }
    Ok(None)
}

fn decode_completion_current(
    bytes: &[u8],
    has_routing_metadata: bool,
    has_birth_proof: bool,
) -> Result<InsomniaCompletion, &'static str> {
    if bytes.len() < 96 {
        return Err("short Insomnia current completion");
    }
    let episode_id = EpisodeId(bytes[8..40].try_into().unwrap());
    let attempt = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
    let started_at_ns = i64::from_le_bytes(bytes[44..52].try_into().unwrap());
    let completed_at_ns = i64::from_le_bytes(bytes[52..60].try_into().unwrap());
    let rejected_count = u32::from_le_bytes(bytes[60..64].try_into().unwrap());
    let global_version_start = u64::from_le_bytes(bytes[64..72].try_into().unwrap());
    let global_version_count = u32::from_le_bytes(bytes[72..76].try_into().unwrap());
    let transaction_time_ns = i64::from_le_bytes(bytes[76..84].try_into().unwrap());
    if global_version_count > 0 && global_version_start == 0 {
        return Err("invalid completion global version range");
    }
    let mut cursor = 84;
    let extractor_model = read_string(bytes, &mut cursor)?;
    let extractor_version = read_string(bytes, &mut cursor)?;
    let memory_ids = read_memory_ids(bytes, &mut cursor)?;
    let external_memory_refs = read_memory_refs(bytes, &mut cursor)?;
    let body_count = read_u32(bytes, &mut cursor)? as usize;
    let mut bodies = Vec::with_capacity(body_count);
    for _ in 0..body_count {
        let id_end = cursor
            .checked_add(32)
            .ok_or("completion body id overflow")?;
        let id_raw = bytes
            .get(cursor..id_end)
            .ok_or("truncated completion body id")?;
        cursor = id_end;
        let len = read_u32(bytes, &mut cursor)? as usize;
        let end = cursor.checked_add(len).ok_or("completion body overflow")?;
        let body = bytes
            .get(cursor..end)
            .ok_or("truncated completion body")?
            .to_vec();
        cursor = end;
        bodies.push(InsomniaCompletionBody {
            id: MemoryBodyId(id_raw.try_into().unwrap()),
            bytes: body,
        });
    }
    let records = read_records(bytes, &mut cursor)?;
    validate_record_versions(global_version_start, global_version_count, &records)?;
    let routing_metadata = if has_routing_metadata {
        read_routing_metadata(bytes, &mut cursor)?
    } else {
        Vec::new()
    };
    let (freshness_birth_turn, freshness_policy_version, freshness_birth_memory_ids) =
        if has_birth_proof {
            let turn = read_u64(bytes, &mut cursor)?;
            let policy = read_u64(bytes, &mut cursor)?;
            let ids = read_memory_ids(bytes, &mut cursor)?;
            let expected = records
                .iter()
                .filter(|record| record.revision == 1)
                .map(|record| record.id)
                .collect::<std::collections::BTreeSet<_>>();
            let actual = ids
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>();
            if policy != crate::freshness::FRESHNESS_POLICY_VERSION as u64
                || actual.len() != ids.len()
                || actual != expected
            {
                return Err("invalid Freshness birth policy or exact cohort");
            }
            (Some(turn), Some(policy), ids)
        } else {
            (None, None, Vec::new())
        };
    if cursor != bytes.len() {
        return Err("Insomnia completion trailing bytes");
    }
    Ok(InsomniaCompletion {
        episode_id,
        attempt,
        started_at_ns,
        completed_at_ns,
        transaction_time_ns,
        extractor_model,
        extractor_version,
        rejected_count,
        memory_ids,
        external_memory_refs,
        global_version_start,
        bodies,
        records,
        routing_metadata,
        freshness_birth_turn,
        freshness_policy_version,
        freshness_birth_memory_ids,
    })
}

fn decode_completion_v3(bytes: &[u8]) -> Result<InsomniaCompletion, &'static str> {
    if bytes.len() < 88 {
        return Err("short Insomnia v3 completion");
    }
    let episode_id = EpisodeId(bytes[8..40].try_into().unwrap());
    let attempt = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
    let started_at_ns = i64::from_le_bytes(bytes[44..52].try_into().unwrap());
    let completed_at_ns = i64::from_le_bytes(bytes[52..60].try_into().unwrap());
    let rejected_count = u32::from_le_bytes(bytes[60..64].try_into().unwrap());
    let global_version_start = u64::from_le_bytes(bytes[64..72].try_into().unwrap());
    let global_version_count = u32::from_le_bytes(bytes[72..76].try_into().unwrap());
    if global_version_count > 0 && global_version_start == 0 {
        return Err("invalid completion global version range");
    }
    let mut cursor = 76;
    let extractor_model = read_string(bytes, &mut cursor)?;
    let extractor_version = read_string(bytes, &mut cursor)?;
    let memory_ids = read_memory_ids(bytes, &mut cursor)?;
    let external_memory_refs = read_memory_refs(bytes, &mut cursor)?;
    let body_count = read_u32(bytes, &mut cursor)? as usize;
    let mut bodies = Vec::with_capacity(body_count);
    for _ in 0..body_count {
        let id_end = cursor
            .checked_add(32)
            .ok_or("completion body id overflow")?;
        let id_raw = bytes
            .get(cursor..id_end)
            .ok_or("truncated completion body id")?;
        cursor = id_end;
        let len = read_u32(bytes, &mut cursor)? as usize;
        let end = cursor.checked_add(len).ok_or("completion body overflow")?;
        let body = bytes
            .get(cursor..end)
            .ok_or("truncated completion body")?
            .to_vec();
        cursor = end;
        bodies.push(InsomniaCompletionBody {
            id: MemoryBodyId(id_raw.try_into().unwrap()),
            bytes: body,
        });
    }
    let records = read_records(bytes, &mut cursor)?;
    validate_record_versions(global_version_start, global_version_count, &records)?;
    if cursor != bytes.len() {
        return Err("Insomnia completion trailing bytes");
    }
    Ok(InsomniaCompletion {
        episode_id,
        attempt,
        started_at_ns,
        completed_at_ns,
        transaction_time_ns: 0,
        extractor_model,
        extractor_version,
        rejected_count,
        memory_ids,
        external_memory_refs,
        global_version_start,
        bodies,
        records,
        routing_metadata: Vec::new(),
        freshness_birth_turn: None,
        freshness_policy_version: None,
        freshness_birth_memory_ids: Vec::new(),
    })
}

fn decode_completion_v2(bytes: &[u8]) -> Result<InsomniaCompletion, &'static str> {
    if bytes.len() < 84 {
        return Err("short Insomnia v2 completion");
    }
    let episode_id = EpisodeId(bytes[8..40].try_into().unwrap());
    let attempt = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
    let started_at_ns = i64::from_le_bytes(bytes[44..52].try_into().unwrap());
    let completed_at_ns = i64::from_le_bytes(bytes[52..60].try_into().unwrap());
    let rejected_count = u32::from_le_bytes(bytes[60..64].try_into().unwrap());
    let global_version_start = u64::from_le_bytes(bytes[64..72].try_into().unwrap());
    let global_version_count = u32::from_le_bytes(bytes[72..76].try_into().unwrap());
    if global_version_count > 0 && global_version_start == 0 {
        return Err("invalid completion global version range");
    }
    let mut cursor = 76;
    let extractor_model = read_string(bytes, &mut cursor)?;
    let extractor_version = read_string(bytes, &mut cursor)?;
    let memory_ids = read_memory_ids(bytes, &mut cursor)?;
    let body_count = read_u32(bytes, &mut cursor)? as usize;
    let mut bodies = Vec::with_capacity(body_count);
    for _ in 0..body_count {
        let id_end = cursor
            .checked_add(32)
            .ok_or("completion body id overflow")?;
        let id_raw = bytes
            .get(cursor..id_end)
            .ok_or("truncated completion body id")?;
        cursor = id_end;
        let len = read_u32(bytes, &mut cursor)? as usize;
        let end = cursor.checked_add(len).ok_or("completion body overflow")?;
        let body = bytes
            .get(cursor..end)
            .ok_or("truncated completion body")?
            .to_vec();
        cursor = end;
        bodies.push(InsomniaCompletionBody {
            id: MemoryBodyId(id_raw.try_into().unwrap()),
            bytes: body,
        });
    }
    let records = read_records(bytes, &mut cursor)?;
    if global_version_count as usize != records.len() {
        return Err("completion global version count mismatch");
    }
    for (index, record) in records.iter().enumerate() {
        let expected = global_version_start
            .checked_add(u64::try_from(index).map_err(|_| "completion version overflow")?)
            .ok_or("completion version overflow")?;
        if record.global_version != expected {
            return Err("non-contiguous completion global versions");
        }
    }
    if cursor != bytes.len() {
        return Err("Insomnia completion trailing bytes");
    }
    Ok(InsomniaCompletion {
        episode_id,
        attempt,
        started_at_ns,
        completed_at_ns,
        transaction_time_ns: 0,
        extractor_model,
        extractor_version,
        rejected_count,
        memory_ids,
        external_memory_refs: Vec::new(),
        global_version_start,
        bodies,
        records,
        routing_metadata: Vec::new(),
        freshness_birth_turn: None,
        freshness_policy_version: None,
        freshness_birth_memory_ids: Vec::new(),
    })
}

fn decode_completion_v1(bytes: &[u8]) -> Result<InsomniaCompletion, &'static str> {
    if bytes.len() < 72 {
        return Err("short Insomnia completion");
    }
    let episode_id = EpisodeId(bytes[8..40].try_into().unwrap());
    let attempt = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
    let started_at_ns = i64::from_le_bytes(bytes[44..52].try_into().unwrap());
    let completed_at_ns = i64::from_le_bytes(bytes[52..60].try_into().unwrap());
    let rejected_count = u32::from_le_bytes(bytes[60..64].try_into().unwrap());
    let mut cursor = 64;
    let extractor_model = read_string(bytes, &mut cursor)?;
    let extractor_version = read_string(bytes, &mut cursor)?;
    let memory_ids = read_memory_ids(bytes, &mut cursor)?;
    let records = read_records(bytes, &mut cursor)?;
    if cursor != bytes.len() {
        return Err("Insomnia completion trailing bytes");
    }
    let global_version_start = records
        .first()
        .map(|record| record.global_version)
        .unwrap_or(0);
    Ok(InsomniaCompletion {
        episode_id,
        attempt,
        started_at_ns,
        completed_at_ns,
        transaction_time_ns: 0,
        extractor_model,
        extractor_version,
        rejected_count,
        memory_ids,
        external_memory_refs: Vec::new(),
        global_version_start,
        bodies: Vec::new(),
        records,
        routing_metadata: Vec::new(),
        freshness_birth_turn: None,
        freshness_policy_version: None,
        freshness_birth_memory_ids: Vec::new(),
    })
}

fn read_memory_ids(bytes: &[u8], cursor: &mut usize) -> Result<Vec<crate::MemoryId>, &'static str> {
    let memory_count = read_u32(bytes, cursor)? as usize;
    let mut memory_ids = Vec::with_capacity(memory_count);
    for _ in 0..memory_count {
        let end = cursor
            .checked_add(32)
            .ok_or("completion memory id overflow")?;
        let raw = bytes
            .get(*cursor..end)
            .ok_or("truncated completion memory id")?;
        *cursor = end;
        memory_ids.push(crate::MemoryId(raw.try_into().unwrap()));
    }
    Ok(memory_ids)
}

fn read_memory_refs(bytes: &[u8], cursor: &mut usize) -> Result<Vec<MemoryRef>, &'static str> {
    let count = read_u32(bytes, cursor)? as usize;
    let mut refs = Vec::with_capacity(count);
    for _ in 0..count {
        let owner_id = read_string(bytes, cursor)?;
        if owner_id.trim().is_empty() {
            return Err("empty external memory owner id");
        }
        let end = cursor
            .checked_add(32)
            .ok_or("completion external memory id overflow")?;
        let raw = bytes
            .get(*cursor..end)
            .ok_or("truncated completion external memory id")?;
        *cursor = end;
        refs.push(MemoryRef {
            owner_id,
            memory_id: crate::MemoryId(raw.try_into().unwrap()),
        });
    }
    Ok(refs)
}

fn validate_record_versions(
    global_version_start: u64,
    global_version_count: u32,
    records: &[MemoryRecord],
) -> Result<(), &'static str> {
    if global_version_count as usize != records.len() {
        return Err("completion global version count mismatch");
    }
    for (index, record) in records.iter().enumerate() {
        let expected = global_version_start
            .checked_add(u64::try_from(index).map_err(|_| "completion version overflow")?)
            .ok_or("completion version overflow")?;
        if record.global_version != expected {
            return Err("non-contiguous completion global versions");
        }
    }
    Ok(())
}

fn read_records(bytes: &[u8], cursor: &mut usize) -> Result<Vec<MemoryRecord>, &'static str> {
    let count = read_u32(bytes, cursor)? as usize;
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        let global_version = read_u64(bytes, cursor)?;
        let memory_version = read_u64(bytes, cursor)?;
        let len = read_u32(bytes, cursor)? as usize;
        let end = cursor
            .checked_add(len)
            .ok_or("completion record overflow")?;
        let encoded = bytes
            .get(*cursor..end)
            .ok_or("truncated completion record")?;
        *cursor = end;
        let mut record = decode_record(encoded)
            .map_err(|_| "invalid completion memory record")?
            .ok_or("missing completion memory record")?;
        record.global_version = global_version;
        record.memory_version = memory_version;
        records.push(record);
    }
    Ok(records)
}

fn read_routing_metadata(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Vec<MemoryRoutingMetadata>, &'static str> {
    let count = read_u32(bytes, cursor)? as usize;
    if count > super::contract::MAX_INSOMNIA_CANDIDATES {
        return Err("too many completion routing metadata records");
    }
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let len = read_u32(bytes, cursor)? as usize;
        let end = cursor
            .checked_add(len)
            .ok_or("completion routing metadata overflow")?;
        let encoded = bytes
            .get(*cursor..end)
            .ok_or("truncated completion routing metadata")?;
        *cursor = end;
        let metadata = crate::memory_routing_codec::decode(encoded)
            .map_err(|_| "invalid completion routing metadata")?
            .ok_or("missing completion routing metadata")?;
        values.push(metadata);
    }
    Ok(values)
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), &'static str> {
    let len = u32::try_from(value.len()).map_err(|_| "completion string too large")?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, &'static str> {
    let len = read_u32(bytes, cursor)? as usize;
    let end = cursor
        .checked_add(len)
        .ok_or("completion string overflow")?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or("truncated completion string")?;
    *cursor = end;
    String::from_utf8(raw.to_vec()).map_err(|_| "invalid completion utf8")
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, &'static str> {
    let end = cursor.checked_add(4).ok_or("completion integer overflow")?;
    let raw = bytes.get(*cursor..end).ok_or("truncated completion u32")?;
    *cursor = end;
    Ok(u32::from_le_bytes(raw.try_into().unwrap()))
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, &'static str> {
    let end = cursor.checked_add(8).ok_or("completion integer overflow")?;
    let raw = bytes.get(*cursor..end).ok_or("truncated completion u64")?;
    *cursor = end;
    Ok(u64::from_le_bytes(raw.try_into().unwrap()))
}
