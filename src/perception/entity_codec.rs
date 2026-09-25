use crate::entity_model::EntityRecord;
use crate::{EntityError, EntityGlobalId, EntityId, ObjectRef};

const FORMAT_MAGIC: [u8; 8] = *b"CVAENTF1";
const RECORD_MAGIC_V1: [u8; 8] = *b"CVAENTR1";
const RECORD_MAGIC_V2: [u8; 8] = *b"CVAENTR2";
const VERSION_MAGIC: [u8; 8] = *b"CVAENTV1";
const TOMBSTONE_MAGIC: [u8; 8] = *b"CVAENTT1";

#[derive(Clone, Copy, Debug)]
pub(crate) struct EntityVersion {
    pub global_version: u64,
    pub entity_version: u64,
    pub record: ObjectRef,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EntityTombstone {
    pub id: EntityId,
    pub revision: u64,
    pub replacement_id: EntityId,
}

pub(crate) fn encode_format() -> [u8; 8] {
    FORMAT_MAGIC
}

pub(crate) fn decode_format(bytes: &[u8]) -> bool {
    bytes == FORMAT_MAGIC
}

pub(crate) fn encode_record(record: &EntityRecord) -> Result<Vec<u8>, EntityError> {
    let global_id = record
        .global_id
        .ok_or(EntityError::InvalidField("Entity global UUID"))?;
    let mut out = Vec::with_capacity(272);
    out.extend_from_slice(&RECORD_MAGIC_V2);
    out.extend_from_slice(&record.id.0);
    out.extend_from_slice(&global_id.0);
    out.extend_from_slice(&record.revision.to_le_bytes());
    out.extend_from_slice(&record.created_at_ns.to_le_bytes());
    out.extend_from_slice(&record.updated_at_ns.to_le_bytes());
    write_string(&mut out, &record.canonical_name)?;
    write_strings(&mut out, &record.aliases)?;
    write_string(&mut out, &record.kind)?;
    write_string(&mut out, &record.summary)?;
    write_string(&mut out, &record.mutation_id)?;
    Ok(out)
}

pub(crate) fn decode_record(bytes: &[u8]) -> Result<Option<EntityRecord>, EntityError> {
    if bytes.len() < 8 {
        return Ok(None);
    }
    let (global_id, revision_offset) = if bytes[..8] == RECORD_MAGIC_V2 {
        if bytes.len() < 80 {
            return Err(EntityError::CorruptRecord("short Entity record"));
        }
        (
            Some(EntityGlobalId(bytes[40..56].try_into().unwrap())),
            56_usize,
        )
    } else if bytes[..8] == RECORD_MAGIC_V1 {
        if bytes.len() < 64 {
            return Err(EntityError::CorruptRecord("short Entity record"));
        }
        (None, 40_usize)
    } else {
        return Ok(None);
    };
    let id = EntityId(bytes[8..40].try_into().unwrap());
    let revision = u64::from_le_bytes(
        bytes[revision_offset..revision_offset + 8]
            .try_into()
            .unwrap(),
    );
    let created_at_ns = i64::from_le_bytes(
        bytes[revision_offset + 8..revision_offset + 16]
            .try_into()
            .unwrap(),
    );
    let updated_at_ns = i64::from_le_bytes(
        bytes[revision_offset + 16..revision_offset + 24]
            .try_into()
            .unwrap(),
    );
    let mut cursor = revision_offset + 24;
    let canonical_name = read_string(bytes, &mut cursor)?;
    let aliases = read_strings(bytes, &mut cursor)?;
    let kind = read_string(bytes, &mut cursor)?;
    let summary = read_string(bytes, &mut cursor)?;
    let mutation_id = read_string(bytes, &mut cursor)?;
    if cursor != bytes.len() {
        return Err(EntityError::CorruptRecord("Entity record trailing bytes"));
    }
    Ok(Some(EntityRecord {
        id,
        global_id,
        revision,
        canonical_name,
        aliases,
        kind,
        summary,
        mutation_id,
        created_at_ns,
        updated_at_ns,
        global_version: 0,
        entity_version: 0,
    }))
}

pub(crate) fn encode_version(version: EntityVersion) -> Vec<u8> {
    let mut out = Vec::with_capacity(40);
    out.extend_from_slice(&VERSION_MAGIC);
    out.extend_from_slice(&version.global_version.to_le_bytes());
    out.extend_from_slice(&version.entity_version.to_le_bytes());
    out.extend_from_slice(&version.record.legacy_bytes());
    out
}

pub(crate) fn decode_version(bytes: &[u8]) -> Result<Option<EntityVersion>, EntityError> {
    if bytes.len() < 8 || bytes[..8] != VERSION_MAGIC {
        return Ok(None);
    }
    if bytes.len() != 40 {
        return Err(EntityError::CorruptRecord("invalid Entity version record"));
    }
    Ok(Some(EntityVersion {
        global_version: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
        entity_version: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
        record: ObjectRef::from_legacy_bytes(bytes[24..40].try_into().unwrap()),
    }))
}

pub(crate) fn encode_tombstone(value: EntityTombstone) -> Vec<u8> {
    let mut out = Vec::with_capacity(80);
    out.extend_from_slice(&TOMBSTONE_MAGIC);
    out.extend_from_slice(&value.id.0);
    out.extend_from_slice(&value.revision.to_le_bytes());
    out.extend_from_slice(&value.replacement_id.0);
    out
}

pub(crate) fn decode_tombstone(bytes: &[u8]) -> Result<Option<EntityTombstone>, EntityError> {
    if bytes.len() < 8 || bytes[..8] != TOMBSTONE_MAGIC {
        return Ok(None);
    }
    if bytes.len() != 80 {
        return Err(EntityError::CorruptRecord("invalid Entity tombstone"));
    }
    Ok(Some(EntityTombstone {
        id: EntityId(bytes[8..40].try_into().unwrap()),
        revision: u64::from_le_bytes(bytes[40..48].try_into().unwrap()),
        replacement_id: EntityId(bytes[48..80].try_into().unwrap()),
    }))
}

fn write_strings(out: &mut Vec<u8>, values: &[String]) -> Result<(), EntityError> {
    let count = u16::try_from(values.len()).map_err(|_| EntityError::FieldTooLarge)?;
    out.extend_from_slice(&count.to_le_bytes());
    for value in values {
        write_string(out, value)?;
    }
    Ok(())
}

fn read_strings(bytes: &[u8], cursor: &mut usize) -> Result<Vec<String>, EntityError> {
    let count = read_u16(bytes, cursor)? as usize;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(read_string(bytes, cursor)?);
    }
    Ok(values)
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), EntityError> {
    let len = u32::try_from(value.len()).map_err(|_| EntityError::FieldTooLarge)?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, EntityError> {
    let len = read_u32(bytes, cursor)? as usize;
    let end = cursor.checked_add(len).ok_or(EntityError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(EntityError::CorruptRecord("truncated Entity string"))?;
    *cursor = end;
    String::from_utf8(raw.to_vec()).map_err(|_| EntityError::InvalidUtf8)
}

fn read_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, EntityError> {
    let end = cursor.checked_add(2).ok_or(EntityError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(EntityError::CorruptRecord("truncated Entity u16"))?;
    *cursor = end;
    Ok(u16::from_le_bytes(raw.try_into().unwrap()))
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, EntityError> {
    let end = cursor.checked_add(4).ok_or(EntityError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(EntityError::CorruptRecord("truncated Entity u32"))?;
    *cursor = end;
    Ok(u32::from_le_bytes(raw.try_into().unwrap()))
}
