//! companion value helpers for Relationship compact frames.
use crate::{
    EntityId, EntityRef, MAX_RELATIONSHIP_EVIDENCE, MAX_RELATIONSHIP_OWNER_ID_BYTES,
    MAX_RELATIONSHIP_PARTICIPANTS, MAX_RELATIONSHIP_ROLE_BYTES, RelationshipMemoryRef,
    RelationshipError, RelationshipParticipant,
};

pub fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), RelationshipError> {
    let length = u32::try_from(value.len()).map_err(|_| RelationshipError::FieldTooLarge)?;
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

pub fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, RelationshipError> {
    let length = read_u32(bytes, cursor)? as usize;
    let end = cursor
        .checked_add(length)
        .ok_or(RelationshipError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(RelationshipError::CorruptRecord(
            "truncated compact Relationship string",
        ))?;
    *cursor = end;
    String::from_utf8(raw.to_vec()).map_err(|_| RelationshipError::InvalidUtf8)
}

pub fn write_participants(
    out: &mut Vec<u8>,
    values: &[RelationshipParticipant],
) -> Result<(), RelationshipError> {
    if values.len() > MAX_RELATIONSHIP_PARTICIPANTS {
        return Err(RelationshipError::FieldTooLarge);
    }
    write_count(out, values.len())?;
    for value in values {
        write_string(out, &value.entity.owner_id)?;
        out.extend_from_slice(&value.entity.entity_id.0);
        match &value.role {
            None => out.push(0),
            Some(role) => {
                out.push(1);
                write_string(out, role)?;
            }
        }
    }
    Ok(())
}

pub fn read_participants(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Vec<RelationshipParticipant>, RelationshipError> {
    let count = read_count(bytes, cursor)?;
    if count > MAX_RELATIONSHIP_PARTICIPANTS {
        return Err(RelationshipError::CorruptRecord(
            "too many compact Relationship participants",
        ));
    }
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let owner_id = read_string(bytes, cursor)?;
        let entity_id = EntityId(read_fixed::<32>(
            bytes,
            cursor,
            "truncated compact Relationship Entity ID",
        )?);
        let role = match read_tag(bytes, cursor, "truncated compact Relationship role tag")? {
            0 => None,
            1 => Some(read_string(bytes, cursor)?),
            _ => unreachable!("read_tag restricts tags to zero or one"),
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

pub fn write_evidence(
    out: &mut Vec<u8>,
    values: &[RelationshipMemoryRef],
) -> Result<(), RelationshipError> {
    if values.len() > MAX_RELATIONSHIP_EVIDENCE {
        return Err(RelationshipError::FieldTooLarge);
    }
    write_count(out, values.len())?;
    for value in values {
        write_string(out, &value.owner_id)?;
        out.extend_from_slice(&value.memory_id);
    }
    Ok(())
}

pub fn read_evidence(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Vec<RelationshipMemoryRef>, RelationshipError> {
    let count = read_count(bytes, cursor)?;
    if count > MAX_RELATIONSHIP_EVIDENCE {
        return Err(RelationshipError::CorruptRecord(
            "too many compact Relationship evidence references",
        ));
    }
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(RelationshipMemoryRef {
            owner_id: read_string(bytes, cursor)?,
            memory_id: read_fixed::<32>(
                bytes,
                cursor,
                "truncated compact Relationship Memory ID",
            )?,
        });
    }
    Ok(values)
}

pub fn write_count(out: &mut Vec<u8>, count: usize) -> Result<(), RelationshipError> {
    let count = u16::try_from(count).map_err(|_| RelationshipError::FieldTooLarge)?;
    out.extend_from_slice(&count.to_le_bytes());
    Ok(())
}

pub fn read_count(bytes: &[u8], cursor: &mut usize) -> Result<usize, RelationshipError> {
    Ok(u16::from_le_bytes(read_fixed::<2>(
        bytes,
        cursor,
        "truncated compact Relationship count",
    )?) as usize)
}

pub fn write_tag(out: &mut Vec<u8>, value: bool) {
    out.push(u8::from(value));
}

pub fn read_tag(
    bytes: &[u8],
    cursor: &mut usize,
    error: &'static str,
) -> Result<u8, RelationshipError> {
    let value = *bytes
        .get(*cursor)
        .ok_or(RelationshipError::CorruptRecord(error))?;
    *cursor = cursor
        .checked_add(1)
        .ok_or(RelationshipError::FieldTooLarge)?;
    if value > 1 {
        return Err(RelationshipError::CorruptRecord(error));
    }
    Ok(value)
}

pub fn write_proof(
    out: &mut Vec<u8>,
    proof: Option<&crate::relationship_compact_contract::RelationshipDescriptionProof>,
) {
    match proof {
        None => write_tag(out, false),
        Some(proof) => {
            write_tag(out, true);
            out.extend_from_slice(&proof.original_carrier_digest);
            out.extend_from_slice(&proof.manifest_digest);
        }
    }
}

pub fn read_proof(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<
    Option<crate::relationship_compact_contract::RelationshipDescriptionProof>,
    RelationshipError,
> {
    match read_tag(bytes, cursor, "invalid compact Relationship proof tag")? {
        0 => Ok(None),
        1 => Ok(Some(
            crate::relationship_compact_contract::RelationshipDescriptionProof {
                original_carrier_digest: read_fixed::<32>(
                    bytes,
                    cursor,
                    "truncated compact Relationship carrier digest",
                )?,
                manifest_digest: read_fixed::<32>(
                    bytes,
                    cursor,
                    "truncated compact Relationship manifest digest",
                )?,
            },
        )),
        _ => unreachable!("read_tag restricts tags to zero or one"),
    }
}

pub fn read_i64(bytes: &[u8], cursor: &mut usize) -> Result<i64, RelationshipError> {
    Ok(i64::from_le_bytes(read_fixed::<8>(
        bytes,
        cursor,
        "truncated compact Relationship i64",
    )?))
}

pub fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, RelationshipError> {
    Ok(u32::from_le_bytes(read_fixed::<4>(
        bytes,
        cursor,
        "truncated compact Relationship u32",
    )?))
}

pub fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, RelationshipError> {
    Ok(u64::from_le_bytes(read_fixed::<8>(
        bytes,
        cursor,
        "truncated compact Relationship u64",
    )?))
}

pub fn read_fixed<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
    error: &'static str,
) -> Result<[u8; N], RelationshipError> {
    let end = cursor
        .checked_add(N)
        .ok_or(RelationshipError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(RelationshipError::CorruptRecord(error))?;
    *cursor = end;
    Ok(raw.try_into().unwrap())
}

pub fn validate_owner_and_role_values(
    participants: &[RelationshipParticipant],
) -> Result<(), RelationshipError> {
    for participant in participants {
        validate_bounded_text(
            &participant.entity.owner_id,
            MAX_RELATIONSHIP_OWNER_ID_BYTES,
            true,
            "Relationship participant owner",
        )?;
        if let Some(role) = &participant.role {
            validate_bounded_text(
                role,
                MAX_RELATIONSHIP_ROLE_BYTES,
                true,
                "Relationship participant role",
            )?;
        }
    }
    Ok(())
}

pub fn validate_evidence_owners(values: &[RelationshipMemoryRef]) -> Result<(), RelationshipError> {
    for value in values {
        validate_bounded_text(
            &value.owner_id,
            MAX_RELATIONSHIP_OWNER_ID_BYTES,
            true,
            "Relationship evidence owner",
        )?;
    }
    Ok(())
}

pub fn validate_bounded_text(
    value: &str,
    max_bytes: usize,
    nonempty: bool,
    field: &'static str,
) -> Result<(), RelationshipError> {
    if (nonempty && value.is_empty())
        || value.len() > max_bytes
        || value.chars().any(char::is_control)
    {
        return Err(RelationshipError::InvalidField(field));
    }
    Ok(())
}
