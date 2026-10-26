//! Typed value dispatch confined to the compact Memory codec.
use crate::memory_codec_scalar::*;
use crate::memory_codec_values::*;
use crate::{EpisodeId, MemoryError, MemoryId, MemorySourceRef, MemoryTemporalInference};

pub(super) trait MemoryValue: Sized {
    fn write(&self, out: &mut Vec<u8>) -> Result<(), MemoryError>;
    fn read(bytes: &[u8], cursor: &mut usize) -> Result<Self, MemoryError>;
}

impl MemoryValue for String {
    fn write(&self, out: &mut Vec<u8>) -> Result<(), MemoryError> {
        write_string(out, self)
    }
    fn read(bytes: &[u8], cursor: &mut usize) -> Result<Self, MemoryError> {
        read_string(bytes, cursor)
    }
}
impl MemoryValue for bool {
    fn write(&self, out: &mut Vec<u8>) -> Result<(), MemoryError> {
        out.push(u8::from(*self));
        Ok(())
    }
    fn read(bytes: &[u8], cursor: &mut usize) -> Result<Self, MemoryError> {
        let value = *bytes
            .get(*cursor)
            .ok_or(MemoryError::CorruptRecord("truncated compact Memory bool"))?;
        *cursor += 1;
        match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(MemoryError::CorruptRecord("invalid compact Memory bool")),
        }
    }
}
impl MemoryValue for i64 {
    fn write(&self, out: &mut Vec<u8>) -> Result<(), MemoryError> {
        out.extend_from_slice(&self.to_le_bytes());
        Ok(())
    }
    fn read(bytes: &[u8], cursor: &mut usize) -> Result<Self, MemoryError> {
        read_i64(bytes, cursor)
    }
}

macro_rules! optional_value {
    ($ty:ty, $write:expr, $read:expr) => {
        impl MemoryValue for Option<$ty> {
            fn write(&self, out: &mut Vec<u8>) -> Result<(), MemoryError> {
                ($write)(out, self)
            }
            fn read(bytes: &[u8], cursor: &mut usize) -> Result<Self, MemoryError> {
                ($read)(bytes, cursor)
            }
        }
    };
}
optional_value!(
    MemoryId,
    |out: &mut Vec<u8>, value: &Option<MemoryId>| {
        write_optional_id(out, *value);
        Ok(())
    },
    read_optional_id
);
optional_value!(
    EpisodeId,
    |out: &mut Vec<u8>, value: &Option<EpisodeId>| {
        write_optional_episode(out, *value);
        Ok(())
    },
    read_optional_episode
);
optional_value!(
    i64,
    |out: &mut Vec<u8>, value: &Option<i64>| {
        write_optional_i64(out, *value);
        Ok(())
    },
    read_optional_i64
);
optional_value!(
    String,
    |out: &mut Vec<u8>, value: &Option<String>| write_optional_string(out, value.as_deref()),
    read_optional_string
);
optional_value!(
    MemorySourceRef,
    |out: &mut Vec<u8>, value: &Option<MemorySourceRef>| write_optional_source_ref(
        out,
        value.as_ref()
    ),
    |bytes: &[u8], cursor: &mut usize| read_optional_source_ref(bytes, cursor, true)
);
optional_value!(
    MemoryTemporalInference,
    |out: &mut Vec<u8>, value: &Option<MemoryTemporalInference>| {
        if value.as_ref().is_some_and(|value| {
            value.inference.model.trim().is_empty()
                || value.inference.contract_version.trim().is_empty()
        }) {
            return Err(MemoryError::InvalidField("temporal inference header"));
        }
        crate::memory_temporal_codec::write_optional(out, value.as_ref())
    },
    crate::memory_temporal_codec::read_optional
);

pub(super) fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, MemoryError> {
    let end = cursor.checked_add(8).ok_or(MemoryError::FieldTooLarge)?;
    let raw = bytes.get(*cursor..end).ok_or(MemoryError::CorruptRecord(
        "truncated compact Memory integer",
    ))?;
    *cursor = end;
    Ok(u64::from_le_bytes(raw.try_into().unwrap()))
}
pub(super) fn read_id(bytes: &[u8], cursor: &mut usize) -> Result<[u8; 32], MemoryError> {
    let end = cursor.checked_add(32).ok_or(MemoryError::FieldTooLarge)?;
    let raw = bytes.get(*cursor..end).ok_or(MemoryError::CorruptRecord(
        "truncated compact Memory identity",
    ))?;
    *cursor = end;
    Ok(raw.try_into().unwrap())
}
