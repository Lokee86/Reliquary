//! Checksummed compact Entity creation and delta carriers.
use crate::EntityError;
use crate::compact_delta::FieldChange;
use crate::entity_compact_contract::{EntityCreation, EntityDelta, EntityDeltaFields};
use crate::entity_compact_values::{
    EntityValue, read_entity_id, read_fixed, read_i64, read_optional_global_id, read_string,
    read_tag, read_u64, write_optional_global_id, write_string, write_strings, write_tag,
};
use crate::entity_model::EntityRecord;
use sha2::{Digest, Sha256};

const CREATION_MAGIC: &[u8; 8] = b"CVAENC01";
const DELTA_MAGIC: &[u8; 8] = b"CVAEND01";
const CREATION_FAMILY: &[u8; 6] = b"CVAENC";
const DELTA_FAMILY: &[u8; 6] = b"CVAEND";
const CHECKSUM_LEN: usize = 32;

/// Recognize and structurally validate only compact Entity carriers.
/// A recognized family with an unknown version fails closed.
pub(crate) fn is_compact_record(bytes: &[u8]) -> Result<bool, EntityError> {
    if bytes.starts_with(CREATION_FAMILY) {
        decode_creation(bytes)?;
        return Ok(true);
    }
    if bytes.starts_with(DELTA_FAMILY) {
        decode_delta(bytes)?;
        return Ok(true);
    }
    Ok(false)
}

pub(crate) fn encode_creation(creation: &EntityCreation) -> Result<Vec<u8>, EntityError> {
    validate_creation(creation)?;
    let record = &creation.record;
    let mut out = Vec::new();
    out.extend_from_slice(CREATION_MAGIC);
    out.extend_from_slice(&record.id.0);
    write_optional_global_id(&mut out, record.global_id);
    out.extend_from_slice(&record.global_version.to_le_bytes());
    out.extend_from_slice(&record.entity_version.to_le_bytes());
    write_string(&mut out, &record.mutation_id)?;
    write_string(&mut out, &record.canonical_name)?;
    write_strings(&mut out, &record.aliases)?;
    write_string(&mut out, &record.kind)?;
    write_string(&mut out, &record.summary)?;
    out.extend_from_slice(&record.created_at_ns.to_le_bytes());
    out.extend_from_slice(&record.updated_at_ns.to_le_bytes());
    write_description_proof(&mut out, creation.description_proof.as_ref());
    append_checksum(&mut out);
    Ok(out)
}

pub(crate) fn decode_creation(bytes: &[u8]) -> Result<EntityCreation, EntityError> {
    let body = checked_body(bytes, CREATION_MAGIC)?;
    let mut cursor = 8;
    let id = read_entity_id(body, &mut cursor)?;
    let global_id = read_optional_global_id(body, &mut cursor)?;
    let global_version = read_u64(body, &mut cursor)?;
    let entity_version = read_u64(body, &mut cursor)?;
    let mutation_id = read_string(body, &mut cursor)?;
    let canonical_name = read_string(body, &mut cursor)?;
    let aliases = <Vec<String> as EntityValue>::read(body, &mut cursor)?;
    let kind = read_string(body, &mut cursor)?;
    let summary = read_string(body, &mut cursor)?;
    let created_at_ns = read_i64(body, &mut cursor)?;
    let updated_at_ns = read_i64(body, &mut cursor)?;
    let description_proof = read_description_proof(body, &mut cursor)?;
    require_end(body, cursor)?;
    let record = EntityRecord {
        id,
        global_id,
        revision: 1,
        canonical_name,
        aliases,
        kind,
        summary,
        mutation_id,
        created_at_ns,
        updated_at_ns,
        global_version,
        entity_version,
    };
    let creation = match description_proof {
        Some(proof) => EntityCreation::new_pruned(record, proof)?,
        None => EntityCreation::new(record)?,
    };
    Ok(creation)
}

pub(crate) fn encode_delta(delta: &EntityDelta) -> Result<Vec<u8>, EntityError> {
    validate_delta_envelope(delta)?;
    EntityDelta::validate_mask(delta.fields.mask())?;
    let mut out = Vec::new();
    out.extend_from_slice(DELTA_MAGIC);
    out.extend_from_slice(&delta.id.0);
    for value in [
        delta.previous_revision,
        delta.revision,
        delta.global_version,
        delta.entity_version,
    ] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    write_string(&mut out, &delta.mutation_id)?;
    out.extend_from_slice(&delta.fields.mask().to_le_bytes());
    write_delta_fields(&delta.fields, &mut out)?;
    write_description_proof(&mut out, delta.description_proof.as_ref());
    append_checksum(&mut out);
    Ok(out)
}

pub(crate) fn decode_delta(bytes: &[u8]) -> Result<EntityDelta, EntityError> {
    let body = checked_body(bytes, DELTA_MAGIC)?;
    let mut cursor = 8;
    let id = read_entity_id(body, &mut cursor)?;
    let previous_revision = read_u64(body, &mut cursor)?;
    let revision = read_u64(body, &mut cursor)?;
    let global_version = read_u64(body, &mut cursor)?;
    let entity_version = read_u64(body, &mut cursor)?;
    let mutation_id = read_string(body, &mut cursor)?;
    let mask = read_u64(body, &mut cursor)?;
    let fields = read_delta_fields(body, &mut cursor, mask)?;
    let description_proof = read_description_proof(body, &mut cursor)?;
    require_end(body, cursor)?;
    let delta = EntityDelta {
        id,
        previous_revision,
        revision,
        global_version,
        entity_version,
        mutation_id,
        fields,
        description_proof,
    };
    validate_delta_envelope(&delta)?;
    Ok(delta)
}

fn validate_creation(creation: &EntityCreation) -> Result<(), EntityError> {
    match &creation.description_proof {
        Some(proof) => {
            EntityCreation::new_pruned(creation.record.clone(), proof.clone())?;
        }
        None => {
            EntityCreation::new(creation.record.clone())?;
        }
    }
    Ok(())
}

fn validate_delta_envelope(delta: &EntityDelta) -> Result<(), EntityError> {
    if delta.previous_revision == 0
        || delta.previous_revision.checked_add(1) != Some(delta.revision)
    {
        return Err(EntityError::RevisionConflict);
    }
    if delta.global_version == 0 || delta.entity_version == 0 {
        return Err(EntityError::InvalidVersion);
    }
    if invalid_mutation_id(&delta.mutation_id) {
        return Err(EntityError::InvalidField("Entity mutation ID"));
    }
    EntityDelta::validate_mask(delta.fields.mask())
}

fn invalid_mutation_id(value: &str) -> bool {
    value.is_empty() || value.chars().any(char::is_control)
}

macro_rules! delta_field_codecs {
    ($($bit:literal => $field:ident),+ $(,)?) => {
        fn write_delta_fields(
            fields: &EntityDeltaFields,
            out: &mut Vec<u8>,
        ) -> Result<(), EntityError> {
            $(if let FieldChange::Set(value) = &fields.$field {
                value.write(out)?;
            })+
            Ok(())
        }

        fn read_delta_fields(
            bytes: &[u8],
            cursor: &mut usize,
            mask: u64,
        ) -> Result<EntityDeltaFields, EntityError> {
            EntityDelta::validate_mask(mask)?;
            Ok(EntityDeltaFields {
                $($field: if mask & (1 << $bit) != 0 {
                    FieldChange::Set(EntityValue::read(bytes, cursor)?)
                } else {
                    FieldChange::Unchanged
                },)+
            })
        }
    };
}

delta_field_codecs! {
    0 => global_id,
    1 => canonical_name,
    2 => aliases,
    3 => kind,
    4 => summary,
    5 => updated_at_ns,
}

fn write_description_proof(
    out: &mut Vec<u8>,
    proof: Option<&crate::entity_compact_contract::EntityDescriptionProof>,
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

fn read_description_proof(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<Option<crate::entity_compact_contract::EntityDescriptionProof>, EntityError> {
    match read_tag(
        bytes,
        cursor,
        "invalid compact Entity description proof tag",
    )? {
        0 => Ok(None),
        1 => Ok(Some(
            crate::entity_compact_contract::EntityDescriptionProof {
                original_carrier_digest: read_fixed(
                    bytes,
                    cursor,
                    "truncated compact Entity original carrier digest",
                )?,
                manifest_digest: read_fixed(
                    bytes,
                    cursor,
                    "truncated compact Entity prune manifest digest",
                )?,
            },
        )),
        _ => unreachable!("read_tag validates its value"),
    }
}

fn checked_body<'a>(bytes: &'a [u8], magic: &[u8; 8]) -> Result<&'a [u8], EntityError> {
    if bytes.get(..8) != Some(magic.as_slice()) {
        return Err(EntityError::CorruptRecord("invalid compact Entity magic"));
    }
    if bytes.len() < 8 + CHECKSUM_LEN {
        return Err(EntityError::CorruptRecord("short compact Entity frame"));
    }
    let body_end = bytes.len() - CHECKSUM_LEN;
    let expected = Sha256::digest(&bytes[..body_end]);
    if expected.as_slice() != &bytes[body_end..] {
        return Err(EntityError::CorruptRecord(
            "compact Entity checksum mismatch",
        ));
    }
    Ok(&bytes[..body_end])
}

fn require_end(bytes: &[u8], cursor: usize) -> Result<(), EntityError> {
    if cursor != bytes.len() {
        return Err(EntityError::CorruptRecord("compact Entity trailing bytes"));
    }
    Ok(())
}

fn append_checksum(out: &mut Vec<u8>) {
    let checksum = Sha256::digest(&*out);
    out.extend_from_slice(&checksum);
}

#[cfg(test)]
#[path = "entity_compact_codec_tests.rs"]
mod tests;
