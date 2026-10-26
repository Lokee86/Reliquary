//! Bounded scalar and ordered-string values for the Entity compact codec.
use crate::{EntityError, EntityGlobalId};

pub(super) trait EntityValue: Sized {
    fn write(&self, out: &mut Vec<u8>) -> Result<(), EntityError>;
    fn read(bytes: &[u8], cursor: &mut usize) -> Result<Self, EntityError>;
}

impl EntityValue for String {
    fn write(&self, out: &mut Vec<u8>) -> Result<(), EntityError> {
        write_string(out, self)
    }

    fn read(bytes: &[u8], cursor: &mut usize) -> Result<Self, EntityError> {
        read_string(bytes, cursor)
    }
}

impl EntityValue for Vec<String> {
    fn write(&self, out: &mut Vec<u8>) -> Result<(), EntityError> {
        write_strings(out, self)
    }

    fn read(bytes: &[u8], cursor: &mut usize) -> Result<Self, EntityError> {
        read_strings(bytes, cursor)
    }
}

impl EntityValue for i64 {
    fn write(&self, out: &mut Vec<u8>) -> Result<(), EntityError> {
        out.extend_from_slice(&self.to_le_bytes());
        Ok(())
    }

    fn read(bytes: &[u8], cursor: &mut usize) -> Result<Self, EntityError> {
        read_i64(bytes, cursor)
    }
}

impl EntityValue for Option<EntityGlobalId> {
    fn write(&self, out: &mut Vec<u8>) -> Result<(), EntityError> {
        write_optional_global_id(out, *self);
        Ok(())
    }

    fn read(bytes: &[u8], cursor: &mut usize) -> Result<Self, EntityError> {
        read_optional_global_id(bytes, cursor)
    }
}

pub(super) fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), EntityError> {
    let len = u32::try_from(value.len()).map_err(|_| EntityError::FieldTooLarge)?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

pub(super) fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, EntityError> {
    let len = read_u32(bytes, cursor)? as usize;
    let end = cursor.checked_add(len).ok_or(EntityError::FieldTooLarge)?;
    let raw = bytes.get(*cursor..end).ok_or(EntityError::CorruptRecord(
        "truncated compact Entity string",
    ))?;
    *cursor = end;
    String::from_utf8(raw.to_vec()).map_err(|_| EntityError::InvalidUtf8)
}

pub(super) fn write_strings(out: &mut Vec<u8>, values: &[String]) -> Result<(), EntityError> {
    if values.len() > crate::MAX_ENTITY_ALIASES {
        return Err(EntityError::FieldTooLarge);
    }
    let count = u16::try_from(values.len()).map_err(|_| EntityError::FieldTooLarge)?;
    out.extend_from_slice(&count.to_le_bytes());
    for value in values {
        write_string(out, value)?;
    }
    Ok(())
}

pub(super) fn read_strings(bytes: &[u8], cursor: &mut usize) -> Result<Vec<String>, EntityError> {
    let count = read_u16(bytes, cursor)? as usize;
    if count > crate::MAX_ENTITY_ALIASES {
        return Err(EntityError::CorruptRecord(
            "too many compact Entity aliases",
        ));
    }
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(read_string(bytes, cursor)?);
    }
    Ok(values)
}

pub(super) fn write_optional_global_id(out: &mut Vec<u8>, value: Option<EntityGlobalId>) {
    match value {
        None => out.push(0),
        Some(id) => {
            out.push(1);
            out.extend_from_slice(&id.0);
        }
    }
}

pub(super) fn read_optional_global_id(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Option<EntityGlobalId>, EntityError> {
    let tag = read_tag(bytes, cursor, "compact Entity global UUID")?;
    match tag {
        0 => Ok(None),
        1 => read_global_id(bytes, cursor).map(Some),
        _ => Err(EntityError::CorruptRecord(
            "invalid compact Entity global UUID tag",
        )),
    }
}

pub(super) fn write_tag(out: &mut Vec<u8>, present: bool) {
    out.push(u8::from(present));
}

pub(super) fn read_tag(
    bytes: &[u8],
    cursor: &mut usize,
    name: &'static str,
) -> Result<u8, EntityError> {
    let value = *bytes.get(*cursor).ok_or(EntityError::CorruptRecord(name))?;
    *cursor = cursor.checked_add(1).ok_or(EntityError::FieldTooLarge)?;
    if value > 1 {
        return Err(EntityError::CorruptRecord(name));
    }
    Ok(value)
}

pub(super) fn read_i64(bytes: &[u8], cursor: &mut usize) -> Result<i64, EntityError> {
    let raw = read_fixed::<8>(bytes, cursor, "truncated compact Entity i64")?;
    Ok(i64::from_le_bytes(raw))
}

pub(super) fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, EntityError> {
    let raw = read_fixed::<8>(bytes, cursor, "truncated compact Entity u64")?;
    Ok(u64::from_le_bytes(raw))
}

pub(super) fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, EntityError> {
    let raw = read_fixed::<4>(bytes, cursor, "truncated compact Entity u32")?;
    Ok(u32::from_le_bytes(raw))
}

pub(super) fn read_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, EntityError> {
    let raw = read_fixed::<2>(bytes, cursor, "truncated compact Entity u16")?;
    Ok(u16::from_le_bytes(raw))
}

pub(super) fn read_entity_id(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<crate::EntityId, EntityError> {
    read_fixed::<32>(bytes, cursor, "truncated compact Entity ID").map(crate::EntityId)
}

pub(super) fn read_global_id(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<EntityGlobalId, EntityError> {
    read_fixed::<16>(bytes, cursor, "truncated compact Entity global UUID").map(EntityGlobalId)
}

pub(super) fn read_fixed<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
    error: &'static str,
) -> Result<[u8; N], EntityError> {
    let end = cursor.checked_add(N).ok_or(EntityError::FieldTooLarge)?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(EntityError::CorruptRecord(error))?;
    *cursor = end;
    Ok(raw.try_into().unwrap())
}
