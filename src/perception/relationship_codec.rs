use crate::relationship_model::RelationshipRecord;
use crate::{
    EntityId, EntityRef, MemoryId, MemoryRef, ObjectRef, RelationshipError, RelationshipId,
    RelationshipParticipant,
};

const FORMAT_MAGIC: [u8; 8] = *b"CVARELF1";
const RECORD_MAGIC: [u8; 8] = *b"CVARELR1";
const VERSION_MAGIC: [u8; 8] = *b"CVARELV1";
const TOMBSTONE_MAGIC: [u8; 8] = *b"CVARELT1";

#[derive(Clone, Copy, Debug)]
pub(crate) struct RelationshipTombstone {
    pub id: RelationshipId,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RelationshipVersion {
    pub global_version: u64,
    pub relationship_version: u64,
    pub record: ObjectRef,
}

pub(crate) fn encode_format() -> [u8; 8] {
    FORMAT_MAGIC
}

pub(crate) fn decode_format(bytes: &[u8]) -> bool {
    bytes == FORMAT_MAGIC
}

pub(crate) fn encode_record(record: &RelationshipRecord) -> Result<Vec<u8>, RelationshipError> {
    let mut out = Vec::with_capacity(256);
    out.extend_from_slice(&RECORD_MAGIC);
    out.extend_from_slice(&record.id.0);
    out.extend_from_slice(&record.revision.to_le_bytes());
    out.extend_from_slice(&record.created_at_ns.to_le_bytes());
    out.extend_from_slice(&record.updated_at_ns.to_le_bytes());
    write_string(&mut out, &record.kind)?;
    write_participants(&mut out, &record.participants)?;
    write_evidence(&mut out, &record.evidence)?;
    write_string(&mut out, &record.summary)?;
    write_string(&mut out, &record.mutation_id)?;
    Ok(out)
}

pub(crate) fn decode_record(bytes: &[u8]) -> Result<Option<RelationshipRecord>, RelationshipError> {
    if bytes.len() < 8 || bytes[..8] != RECORD_MAGIC {
        return Ok(None);
    }
    if bytes.len() < 64 {
        return Err(RelationshipError::CorruptRecord(
            "short Relationship record",
        ));
    }
    let id = RelationshipId(bytes[8..40].try_into().unwrap());
    let revision = u64::from_le_bytes(bytes[40..48].try_into().unwrap());
    let created_at_ns = i64::from_le_bytes(bytes[48..56].try_into().unwrap());
    let updated_at_ns = i64::from_le_bytes(bytes[56..64].try_into().unwrap());
    let mut cursor = 64;
    let kind = read_string(bytes, &mut cursor)?;
    let participants = read_participants(bytes, &mut cursor)?;
    let evidence = read_evidence(bytes, &mut cursor)?;
    let summary = read_string(bytes, &mut cursor)?;
    let mutation_id = read_string(bytes, &mut cursor)?;
    if cursor != bytes.len() {
        return Err(RelationshipError::CorruptRecord(
            "Relationship record trailing bytes",
        ));
    }
    Ok(Some(RelationshipRecord {
        id,
        revision,
        kind,
        participants,
        evidence,
        summary,
        mutation_id,
        created_at_ns,
        updated_at_ns,
        global_version: 0,
        relationship_version: 0,
    }))
}

pub(crate) fn encode_version(version: RelationshipVersion) -> Vec<u8> {
    let mut out = Vec::with_capacity(40);
    out.extend_from_slice(&VERSION_MAGIC);
    out.extend_from_slice(&version.global_version.to_le_bytes());
    out.extend_from_slice(&version.relationship_version.to_le_bytes());
    out.extend_from_slice(&version.record.legacy_bytes());
    out
}

pub(crate) fn decode_version(
    bytes: &[u8],
) -> Result<Option<RelationshipVersion>, RelationshipError> {
    if bytes.len() < 8 || bytes[..8] != VERSION_MAGIC {
        return Ok(None);
    }
    if bytes.len() != 40 {
        return Err(RelationshipError::CorruptRecord(
            "invalid Relationship version record",
        ));
    }
    Ok(Some(RelationshipVersion {
        global_version: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
        relationship_version: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
        record: ObjectRef::from_legacy_bytes(bytes[24..40].try_into().unwrap()),
    }))
}

pub(crate) fn encode_tombstone(value: RelationshipTombstone) -> Vec<u8> {
    let mut out = Vec::with_capacity(48);
    out.extend_from_slice(&TOMBSTONE_MAGIC);
    out.extend_from_slice(&value.id.0);
    out.extend_from_slice(&value.revision.to_le_bytes());
    out
}

pub(crate) fn decode_tombstone(
    bytes: &[u8],
) -> Result<Option<RelationshipTombstone>, RelationshipError> {
    if bytes.len() < 8 || bytes[..8] != TOMBSTONE_MAGIC {
        return Ok(None);
    }
    if bytes.len() != 48 {
        return Err(RelationshipError::CorruptRecord(
            "invalid Relationship tombstone",
        ));
    }
    Ok(Some(RelationshipTombstone {
        id: RelationshipId(bytes[8..40].try_into().unwrap()),
        revision: u64::from_le_bytes(bytes[40..48].try_into().unwrap()),
    }))
}

fn write_participants(
    out: &mut Vec<u8>,
    values: &[RelationshipParticipant],
) -> Result<(), RelationshipError> {
    write_count(out, values.len())?;
    for value in values {
        write_string(out, &value.entity.owner_id)?;
        out.extend_from_slice(&value.entity.entity_id.0);
        match &value.role {
            Some(role) => {
                out.push(1);
                write_string(out, role)?;
            }
            None => out.push(0),
        }
    }
    Ok(())
}

fn read_participants(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Vec<RelationshipParticipant>, RelationshipError> {
    let count = read_u16(bytes, cursor)? as usize;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let owner_id = read_string(bytes, cursor)?;
        let entity_id = EntityId(read_fixed::<32>(bytes, cursor)?);
        let role = match read_u8(bytes, cursor)? {
            0 => None,
            1 => Some(read_string(bytes, cursor)?),
            _ => {
                return Err(RelationshipError::CorruptRecord(
                    "invalid participant role tag",
                ));
            }
        };
        values.push(RelationshipParticipant {
            entity: EntityRef {
                owner_id,
                entity_id,
            },
            role,
        });
    }
    Ok(values)
}

fn write_evidence(out: &mut Vec<u8>, values: &[MemoryRef]) -> Result<(), RelationshipError> {
    write_count(out, values.len())?;
    for value in values {
        write_string(out, &value.owner_id)?;
        out.extend_from_slice(&value.memory_id.0);
    }
    Ok(())
}

fn read_evidence(bytes: &[u8], cursor: &mut usize) -> Result<Vec<MemoryRef>, RelationshipError> {
    let count = read_u16(bytes, cursor)? as usize;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(MemoryRef {
            owner_id: read_string(bytes, cursor)?,
            memory_id: MemoryId(read_fixed::<32>(bytes, cursor)?),
        });
    }
    Ok(values)
}

fn write_count(out: &mut Vec<u8>, count: usize) -> Result<(), RelationshipError> {
    let count = u16::try_from(count).map_err(|_| RelationshipError::FieldTooLarge)?;
    out.extend_from_slice(&count.to_le_bytes());
    Ok(())
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), RelationshipError> {
    let len = u32::try_from(value.len()).map_err(|_| RelationshipError::FieldTooLarge)?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, RelationshipError> {
    let len = read_u32(bytes, cursor)? as usize;
    let raw = read_slice(bytes, cursor, len)?;
    String::from_utf8(raw.to_vec()).map_err(|_| RelationshipError::InvalidUtf8)
}

fn read_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8, RelationshipError> {
    Ok(read_slice(bytes, cursor, 1)?[0])
}

fn read_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, RelationshipError> {
    Ok(u16::from_le_bytes(read_fixed::<2>(bytes, cursor)?))
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, RelationshipError> {
    Ok(u32::from_le_bytes(read_fixed::<4>(bytes, cursor)?))
}

fn read_fixed<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], RelationshipError> {
    Ok(read_slice(bytes, cursor, N)?.try_into().unwrap())
}

fn read_slice<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    len: usize,
) -> Result<&'a [u8], RelationshipError> {
    let end = cursor
        .checked_add(len)
        .ok_or(RelationshipError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(RelationshipError::CorruptRecord(
            "truncated Relationship record",
        ))?;
    *cursor = end;
    Ok(raw)
}
