//! Relationship creation/delta wire codec; publication marker remains owner-managed.
use crate::relationship_compact_contract::{
    RELATIONSHIP_DELTA_KNOWN_MASK, RelationshipCollectionChange, RelationshipCreation,
    RelationshipDelta, RelationshipDeltaFields,
};
use crate::relationship_compact_values::{
    read_evidence, read_fixed, read_i64, read_participants, read_proof, read_string, read_u64,
    validate_bounded_text, validate_evidence_owners, validate_owner_and_role_values,
    write_evidence, write_participants, write_proof, write_string,
};
use crate::relationship_model::RelationshipRecord;
use crate::{
    MAX_RELATIONSHIP_EVIDENCE, MAX_RELATIONSHIP_KIND_BYTES, MAX_RELATIONSHIP_PARTICIPANTS,
    MAX_RELATIONSHIP_SUMMARY_BYTES, RelationshipError, RelationshipId,
};
use reliquary_container::codec::FieldChange;
use sha2::{Digest, Sha256};

#[path = "relationship_publication_codec.rs"]
pub mod publication;

#[cfg(test)]
#[path = "relationship_publication_codec_tests.rs"]
mod publication_tests;

const CREATION_MAGIC: &[u8; 8] = b"CVARNC01";
const DELTA_MAGIC: &[u8; 8] = b"CVARND01";
const CREATION_FAMILY: &[u8; 6] = b"CVARNC";
const DELTA_FAMILY: &[u8; 6] = b"CVARND";
const CHECKSUM_LEN: usize = 32;

pub fn is_compact_record(bytes: &[u8]) -> Result<bool, RelationshipError> {
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

pub fn encode_creation(
    creation: &RelationshipCreation,
) -> Result<Vec<u8>, RelationshipError> {
    validate_creation(creation)?;
    let record = &creation.record;
    let mut out = Vec::new();
    out.extend_from_slice(CREATION_MAGIC);
    out.extend_from_slice(&record.id.0);
    out.extend_from_slice(&record.global_version.to_le_bytes());
    out.extend_from_slice(&record.relationship_version.to_le_bytes());
    write_string(&mut out, &record.mutation_id)?;
    write_string(&mut out, &record.kind)?;
    write_participants(&mut out, &record.participants)?;
    write_evidence(&mut out, &record.evidence)?;
    write_string(&mut out, &record.summary)?;
    out.extend_from_slice(&record.created_at_ns.to_le_bytes());
    out.extend_from_slice(&record.updated_at_ns.to_le_bytes());
    write_proof(&mut out, creation.description_proof.as_ref());
    if creation.description_proof.is_some() {
        append_checksum(&mut out);
    }
    Ok(out)
}

pub fn decode_creation(bytes: &[u8]) -> Result<RelationshipCreation, RelationshipError> {
    require_magic(bytes, CREATION_MAGIC)?;
    let mut cursor = 8;
    let id = RelationshipId(read_fixed(
        bytes,
        &mut cursor,
        "truncated compact Relationship ID",
    )?);
    let global_version = read_u64(bytes, &mut cursor)?;
    let relationship_version = read_u64(bytes, &mut cursor)?;
    let mutation_id = read_string(bytes, &mut cursor)?;
    let kind = read_string(bytes, &mut cursor)?;
    let participants = read_participants(bytes, &mut cursor)?;
    let evidence = read_evidence(bytes, &mut cursor)?;
    let summary = read_string(bytes, &mut cursor)?;
    let created_at_ns = read_i64(bytes, &mut cursor)?;
    let updated_at_ns = read_i64(bytes, &mut cursor)?;
    let description_proof = read_proof(bytes, &mut cursor)?;
    finish_frame(bytes, cursor, description_proof.is_some())?;
    let record = RelationshipRecord {
        id,
        revision: 1,
        kind,
        participants,
        evidence,
        summary,
        mutation_id,
        created_at_ns,
        updated_at_ns,
        global_version,
        relationship_version,
    };
    match description_proof {
        Some(proof) => RelationshipCreation::new_pruned(record, proof),
        None => RelationshipCreation::new(record),
    }
}

pub fn encode_delta(delta: &RelationshipDelta) -> Result<Vec<u8>, RelationshipError> {
    validate_delta(delta)?;
    let mut out = Vec::new();
    out.extend_from_slice(DELTA_MAGIC);
    out.extend_from_slice(&delta.id.0);
    out.extend_from_slice(&delta.previous_revision.to_le_bytes());
    out.extend_from_slice(&delta.revision.to_le_bytes());
    out.extend_from_slice(&delta.global_version.to_le_bytes());
    out.extend_from_slice(&delta.relationship_version.to_le_bytes());
    write_string(&mut out, &delta.mutation_id)?;
    let mask = delta.fields.mask();
    out.extend_from_slice(&mask.to_le_bytes());
    write_delta_fields(&delta.fields, mask, &mut out)?;
    write_proof(&mut out, delta.description_proof.as_ref());
    if delta.description_proof.is_some() {
        append_checksum(&mut out);
    }
    Ok(out)
}

pub fn decode_delta(bytes: &[u8]) -> Result<RelationshipDelta, RelationshipError> {
    require_magic(bytes, DELTA_MAGIC)?;
    let mut cursor = 8;
    let id = RelationshipId(read_fixed(
        bytes,
        &mut cursor,
        "truncated compact Relationship ID",
    )?);
    let previous_revision = read_u64(bytes, &mut cursor)?;
    let revision = read_u64(bytes, &mut cursor)?;
    let global_version = read_u64(bytes, &mut cursor)?;
    let relationship_version = read_u64(bytes, &mut cursor)?;
    let mutation_id = read_string(bytes, &mut cursor)?;
    let mask = read_u64(bytes, &mut cursor)?;
    RelationshipDelta::validate_mask(mask)?;
    let fields = read_delta_fields(bytes, &mut cursor, mask)?;
    let description_proof = read_proof(bytes, &mut cursor)?;
    finish_frame(bytes, cursor, description_proof.is_some())?;
    let delta = RelationshipDelta {
        id,
        previous_revision,
        revision,
        global_version,
        relationship_version,
        mutation_id,
        fields,
        description_proof,
    };
    validate_delta(&delta)?;
    Ok(delta)
}

fn validate_creation(creation: &RelationshipCreation) -> Result<(), RelationshipError> {
    match &creation.description_proof {
        Some(proof) => {
            RelationshipCreation::new_pruned(creation.record.clone(), proof.clone())?;
        }
        None => {
            RelationshipCreation::new(creation.record.clone())?;
        }
    }
    Ok(())
}

fn validate_delta(delta: &RelationshipDelta) -> Result<(), RelationshipError> {
    if delta.previous_revision == 0
        || delta.previous_revision.checked_add(1) != Some(delta.revision)
    {
        return Err(RelationshipError::RevisionConflict);
    }
    if delta.global_version == 0 || delta.relationship_version == 0 {
        return Err(RelationshipError::InvalidVersion);
    }
    validate_bounded_text(
        &delta.mutation_id,
        usize::MAX,
        true,
        "Relationship mutation ID",
    )?;
    RelationshipDelta::validate_mask(delta.fields.mask())?;
    delta.fields.validate_canonical()?;
    if delta.fields.mask() & !RELATIONSHIP_DELTA_KNOWN_MASK != 0 {
        return Err(RelationshipError::CorruptRecord(
            "unknown Relationship delta field",
        ));
    }
    if let FieldChange::Set(kind) = &delta.fields.kind {
        validate_bounded_text(kind, MAX_RELATIONSHIP_KIND_BYTES, true, "Relationship kind")?;
    }
    if let FieldChange::Set(summary) = &delta.fields.summary {
        validate_bounded_text(
            summary,
            MAX_RELATIONSHIP_SUMMARY_BYTES,
            false,
            "Relationship summary",
        )?;
    }
    if delta.description_proof.is_some()
        && matches!(&delta.fields.summary, FieldChange::Set(value) if !value.is_empty())
    {
        return Err(RelationshipError::CorruptRecord(
            "noncanonical pruned Relationship summary",
        ));
    }
    validate_collection_limits(
        &delta.fields.participants,
        MAX_RELATIONSHIP_PARTICIPANTS,
        "too many compact Relationship participant operations",
    )?;
    validate_owner_and_role_values(&delta.fields.participants.removed)?;
    validate_owner_and_role_values(&delta.fields.participants.added)?;
    validate_collection_limits(
        &delta.fields.evidence,
        MAX_RELATIONSHIP_EVIDENCE,
        "too many compact Relationship evidence operations",
    )?;
    validate_evidence_owners(&delta.fields.evidence.removed)?;
    validate_evidence_owners(&delta.fields.evidence.added)?;
    Ok(())
}

fn validate_collection_limits<T>(
    change: &RelationshipCollectionChange<T>,
    maximum: usize,
    error: &'static str,
) -> Result<(), RelationshipError> {
    if change.removed.len() > maximum || change.added.len() > maximum {
        return Err(RelationshipError::CorruptRecord(error));
    }
    Ok(())
}

fn write_delta_fields(
    fields: &RelationshipDeltaFields,
    mask: u64,
    out: &mut Vec<u8>,
) -> Result<(), RelationshipError> {
    if mask & (1 << 0) != 0
        && let FieldChange::Set(value) = &fields.kind
    {
        write_string(out, value)?;
    }
    if mask & (1 << 1) != 0 {
        write_collection_change(out, &fields.participants, write_participants)?;
    }
    if mask & (1 << 2) != 0 {
        write_collection_change(out, &fields.evidence, write_evidence)?;
    }
    if mask & (1 << 3) != 0
        && let FieldChange::Set(value) = &fields.summary
    {
        write_string(out, value)?;
    }
    if mask & (1 << 4) != 0
        && let FieldChange::Set(value) = &fields.updated_at_ns
    {
        out.extend_from_slice(&value.to_le_bytes());
    }
    Ok(())
}

fn write_collection_change<T>(
    out: &mut Vec<u8>,
    change: &RelationshipCollectionChange<T>,
    write_values: fn(&mut Vec<u8>, &[T]) -> Result<(), RelationshipError>,
) -> Result<(), RelationshipError> {
    write_values(out, &change.removed)?;
    write_values(out, &change.added)
}

fn read_delta_fields(
    bytes: &[u8],
    cursor: &mut usize,
    mask: u64,
) -> Result<RelationshipDeltaFields, RelationshipError> {
    RelationshipDelta::validate_mask(mask)?;
    Ok(RelationshipDeltaFields {
        kind: if mask & (1 << 0) != 0 {
            FieldChange::Set(read_string(bytes, cursor)?)
        } else {
            FieldChange::Unchanged
        },
        participants: if mask & (1 << 1) != 0 {
            read_collection_change(bytes, cursor, read_participants)?
        } else {
            RelationshipCollectionChange {
                removed: Vec::new(),
                added: Vec::new(),
            }
        },
        evidence: if mask & (1 << 2) != 0 {
            read_collection_change(bytes, cursor, read_evidence)?
        } else {
            RelationshipCollectionChange {
                removed: Vec::new(),
                added: Vec::new(),
            }
        },
        summary: if mask & (1 << 3) != 0 {
            FieldChange::Set(read_string(bytes, cursor)?)
        } else {
            FieldChange::Unchanged
        },
        updated_at_ns: if mask & (1 << 4) != 0 {
            FieldChange::Set(read_i64(bytes, cursor)?)
        } else {
            FieldChange::Unchanged
        },
    })
}

fn read_collection_change<T: Clone + Ord>(
    bytes: &[u8],
    cursor: &mut usize,
    read_values: fn(&[u8], &mut usize) -> Result<Vec<T>, RelationshipError>,
) -> Result<RelationshipCollectionChange<T>, RelationshipError> {
    let change = RelationshipCollectionChange {
        removed: read_values(bytes, cursor)?,
        added: read_values(bytes, cursor)?,
    };
    if change.is_empty() {
        return Err(RelationshipError::CorruptRecord(
            "empty encoded Relationship collection change",
        ));
    }
    Ok(change)
}

fn require_magic(bytes: &[u8], magic: &[u8; 8]) -> Result<(), RelationshipError> {
    if bytes.get(..8) != Some(magic.as_slice()) {
        return Err(RelationshipError::CorruptRecord(
            "invalid compact Relationship magic",
        ));
    }
    Ok(())
}

fn finish_frame(bytes: &[u8], cursor: usize, has_proof: bool) -> Result<(), RelationshipError> {
    if has_proof {
        if bytes.len() != cursor.saturating_add(CHECKSUM_LEN) {
            return Err(RelationshipError::CorruptRecord(
                "invalid compact Relationship proof frame length",
            ));
        }
        let expected = Sha256::digest(&bytes[..cursor]);
        if expected.as_slice() != &bytes[cursor..] {
            return Err(RelationshipError::CorruptRecord(
                "compact Relationship proof checksum mismatch",
            ));
        }
    } else if cursor != bytes.len() {
        return Err(RelationshipError::CorruptRecord(
            "compact Relationship trailing bytes",
        ));
    }
    Ok(())
}

fn append_checksum(out: &mut Vec<u8>) {
    let checksum = Sha256::digest(&*out);
    out.extend_from_slice(&checksum);
}

#[cfg(test)]
#[path = "relationship_compact_codec_tests.rs"]
mod tests;
