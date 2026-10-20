//! Memory wire value codecs shared by marker-bound carriers and envelope bodies.
#[path = "memory_publication_codec.rs"]
pub(crate) mod publication;
use crate::memory_compact_contract::{
    MEMORY_DELTA_KNOWN_MASK, MemoryCreation, MemoryDelta, MemoryDeltaFields, MemoryFieldChange,
};
use crate::memory_compact_values::{MemoryValue, read_id, read_u64};
use crate::memory_model::MemoryRecord;
use crate::{MemoryBodyId, MemoryError, MemoryId};

const CREATION_MAGIC: &[u8; 8] = b"CVAMC001";
const DELTA_MAGIC: &[u8; 8] = b"CVAMD001";

/// Recognize and structurally validate only this owner's compact carriers.
pub(crate) fn is_compact_record(bytes: &[u8]) -> Result<bool, MemoryError> {
    if bytes.starts_with(b"CVAMC") {
        decode_creation(bytes)?;
        return Ok(true);
    }
    if bytes.starts_with(b"CVAMD") {
        decode_delta(bytes)?;
        return Ok(true);
    }
    Ok(false)
}

macro_rules! field_codecs {
    ($($bit:literal => $field:ident),+ $(,)?) => {
        fn write_fields(fields: &MemoryDeltaFields,out: &mut Vec<u8>) -> Result<(),MemoryError> {
            $(if let MemoryFieldChange::Set(value)=&fields.$field { value.write(out)?; })+
            Ok(())
        }
        fn read_fields(bytes: &[u8],cursor: &mut usize,mask:u64) -> Result<MemoryDeltaFields,MemoryError> {
            MemoryDelta::validate_mask(mask)?;
            Ok(MemoryDeltaFields { $($field: if mask & (1 << $bit) != 0 {
                MemoryFieldChange::Set(MemoryValue::read(bytes,cursor)?)
            } else { MemoryFieldChange::Unchanged },)+ })
        }
        fn write_record_fields(record:&MemoryRecord,out:&mut Vec<u8>)->Result<(),MemoryError> {
            $(record.$field.write(out)?;)+
            Ok(())
        }
        fn creation_record(id:MemoryId,body_id:MemoryBodyId,global_version:u64,memory_version:u64,
            mutation_id:String,fields:MemoryDeltaFields)->Result<MemoryRecord,MemoryError> {
            $(let MemoryFieldChange::Set($field)=fields.$field else {
                return Err(MemoryError::CorruptRecord("missing Memory creation field"));
            };)+
            Ok(MemoryRecord {id,revision:1,body_id,global_version,memory_version,mutation_id,$($field,)+})
        }
    };
}
field_codecs! {0=>category,1=>memory_type,2=>authority_kind,3=>temporal_status,4=>scope,
5=>lifecycle_state,6=>archived,7=>superseded_by,8=>parent_id,9=>source_node_id,
10=>content_source_conversation_id,11=>content_source_node_id,12=>grounding_source_conversation_id,
13=>grounding_source_node_id,14=>source_episode_id,15=>source_time_ns,16=>source_ref,
17=>temporal_inference,18=>created_at_ns,19=>updated_at_ns}

fn require_magic(bytes: &[u8], magic: &[u8; 8]) -> Result<usize, MemoryError> {
    if bytes.get(..8) != Some(magic.as_slice()) {
        return Err(MemoryError::CorruptRecord("invalid compact Memory magic"));
    }
    Ok(8)
}
fn require_end(bytes: &[u8], cursor: usize) -> Result<(), MemoryError> {
    if bytes.len() != cursor {
        return Err(MemoryError::CorruptRecord("compact Memory trailing bytes"));
    }
    Ok(())
}

pub(crate) fn validate_record_values(record: &MemoryRecord) -> Result<(), MemoryError> {
    let mut scratch = Vec::new();
    record.mutation_id.write(&mut scratch)?;
    write_record_fields(record, &mut scratch)
}

pub(crate) fn encode_creation(creation: &MemoryCreation) -> Result<Vec<u8>, MemoryError> {
    creation.validate()?;
    let record = &creation.record;
    let mut out = Vec::new();
    out.extend_from_slice(CREATION_MAGIC);
    out.extend_from_slice(&record.id.0);
    out.extend_from_slice(&record.body_id.0);
    out.extend_from_slice(&record.global_version.to_le_bytes());
    out.extend_from_slice(&record.memory_version.to_le_bytes());
    record.mutation_id.write(&mut out)?;
    write_record_fields(record, &mut out)?;
    Ok(out)
}
pub(crate) fn decode_creation(bytes: &[u8]) -> Result<MemoryCreation, MemoryError> {
    let mut cursor = require_magic(bytes, CREATION_MAGIC)?;
    let id = MemoryId(read_id(bytes, &mut cursor)?);
    let body_id = MemoryBodyId(read_id(bytes, &mut cursor)?);
    let global_version = read_u64(bytes, &mut cursor)?;
    let memory_version = read_u64(bytes, &mut cursor)?;
    let mutation_id = String::read(bytes, &mut cursor)?;
    let fields = read_fields(bytes, &mut cursor, MEMORY_DELTA_KNOWN_MASK)?;
    require_end(bytes, cursor)?;
    MemoryCreation::new(creation_record(
        id,
        body_id,
        global_version,
        memory_version,
        mutation_id,
        fields,
    )?)
}

fn validate_delta_envelope(delta: &MemoryDelta) -> Result<(), MemoryError> {
    if delta.previous_revision == 0
        || delta.previous_revision.checked_add(1) != Some(delta.revision)
    {
        return Err(MemoryError::RevisionConflict);
    }
    if delta.global_version == 0 || delta.memory_version == 0 {
        return Err(MemoryError::InvalidVersion);
    }
    if delta.mutation_id.is_empty() {
        return Err(MemoryError::MutationConflict);
    }
    Ok(())
}
pub(crate) fn encode_delta(delta: &MemoryDelta) -> Result<Vec<u8>, MemoryError> {
    validate_delta_envelope(delta)?;
    let mut out = Vec::new();
    out.extend_from_slice(DELTA_MAGIC);
    out.extend_from_slice(&delta.id.0);
    for value in [
        delta.previous_revision,
        delta.revision,
        delta.global_version,
        delta.memory_version,
    ] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    delta.mutation_id.write(&mut out)?;
    out.extend_from_slice(&delta.fields.mask().to_le_bytes());
    write_fields(&delta.fields, &mut out)?;
    Ok(out)
}
pub(crate) fn decode_delta(bytes: &[u8]) -> Result<MemoryDelta, MemoryError> {
    let mut cursor = require_magic(bytes, DELTA_MAGIC)?;
    let id = MemoryId(read_id(bytes, &mut cursor)?);
    let previous_revision = read_u64(bytes, &mut cursor)?;
    let revision = read_u64(bytes, &mut cursor)?;
    let global_version = read_u64(bytes, &mut cursor)?;
    let memory_version = read_u64(bytes, &mut cursor)?;
    let mutation_id = String::read(bytes, &mut cursor)?;
    let mask = read_u64(bytes, &mut cursor)?;
    let fields = read_fields(bytes, &mut cursor, mask)?;
    require_end(bytes, cursor)?;
    let delta = MemoryDelta {
        id,
        previous_revision,
        revision,
        global_version,
        memory_version,
        mutation_id,
        fields,
    };
    validate_delta_envelope(&delta)?;
    Ok(delta)
}

#[cfg(test)]
#[path = "memory_compact_codec_tests.rs"]
mod tests;
