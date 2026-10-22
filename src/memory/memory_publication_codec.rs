//! Memory-owned bodies for physical publication envelopes.
//! The physical header supplies global order; local/revision facts stay here.
use super::{
    creation_record, read_fields, require_end, require_magic, validate_delta_envelope,
    write_fields, write_record_fields,
};
use crate::memory_compact_contract::{MEMORY_DELTA_KNOWN_MASK, MemoryCreation, MemoryDelta};
use crate::memory_compact_values::{MemoryValue, read_id, read_u64};
use crate::{MemoryBodyId, MemoryError, MemoryId};

const CREATION_MAGIC: &[u8; 8] = b"CVAMC002";
const DELTA_MAGIC: &[u8; 8] = b"CVAMD002";
const TOMBSTONE_MAGIC: &[u8; 8] = b"CVAMT002";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MemoryPublication {
    Creation(MemoryCreation),
    Delta(MemoryDelta),
    Tombstone {
        id: MemoryId,
        revision: u64,
        memory_version: u64,
    },
}

pub(crate) fn encode(publication: &MemoryPublication) -> Result<Vec<u8>, MemoryError> {
    let mut out = Vec::new();
    match publication {
        MemoryPublication::Creation(creation) => {
            creation.validate()?;
            let record = &creation.record;
            out.extend_from_slice(CREATION_MAGIC);
            out.extend_from_slice(&record.id.0);
            out.extend_from_slice(&record.body_id.0);
            out.extend_from_slice(&record.memory_version.to_le_bytes());
            record.mutation_id.write(&mut out)?;
            write_record_fields(record, &mut out)?;
        }
        MemoryPublication::Delta(delta) => {
            validate_delta_envelope(delta)?;
            out.extend_from_slice(DELTA_MAGIC);
            out.extend_from_slice(&delta.id.0);
            for value in [
                delta.previous_revision,
                delta.revision,
                delta.memory_version,
            ] {
                out.extend_from_slice(&value.to_le_bytes());
            }
            delta.mutation_id.write(&mut out)?;
            out.extend_from_slice(&delta.fields.mask().to_le_bytes());
            write_fields(&delta.fields, &mut out)?;
        }
        MemoryPublication::Tombstone {
            id,
            revision,
            memory_version,
        } => {
            validate_terminal(*revision, *memory_version)?;
            out.extend_from_slice(TOMBSTONE_MAGIC);
            out.extend_from_slice(&id.0);
            out.extend_from_slice(&revision.to_le_bytes());
            out.extend_from_slice(&memory_version.to_le_bytes());
        }
    }
    Ok(out)
}

pub(crate) fn decode(
    bytes: &[u8],
    global_version: u64,
) -> Result<Option<MemoryPublication>, MemoryError> {
    let recognized =
        bytes.starts_with(b"CVAMC") || bytes.starts_with(b"CVAMD") || bytes.starts_with(b"CVAMT");
    if !recognized {
        return Ok(None);
    }
    if global_version == 0 {
        return Err(MemoryError::InvalidVersion);
    }
    if bytes.starts_with(b"CVAMC") {
        let mut cursor = require_magic(bytes, CREATION_MAGIC)?;
        let id = MemoryId(read_id(bytes, &mut cursor)?);
        let body_id = MemoryBodyId(read_id(bytes, &mut cursor)?);
        let memory_version = read_u64(bytes, &mut cursor)?;
        let mutation_id = String::read(bytes, &mut cursor)?;
        let fields = read_fields(bytes, &mut cursor, MEMORY_DELTA_KNOWN_MASK)?;
        require_end(bytes, cursor)?;
        return Ok(Some(MemoryPublication::Creation(MemoryCreation::new(
            creation_record(
                id,
                body_id,
                global_version,
                memory_version,
                mutation_id,
                fields,
            )?,
        )?)));
    }
    if bytes.starts_with(b"CVAMD") {
        let mut cursor = require_magic(bytes, DELTA_MAGIC)?;
        let id = MemoryId(read_id(bytes, &mut cursor)?);
        let previous_revision = read_u64(bytes, &mut cursor)?;
        let revision = read_u64(bytes, &mut cursor)?;
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
        return Ok(Some(MemoryPublication::Delta(delta)));
    }
    let mut cursor = require_magic(bytes, TOMBSTONE_MAGIC)?;
    let id = MemoryId(read_id(bytes, &mut cursor)?);
    let revision = read_u64(bytes, &mut cursor)?;
    let memory_version = read_u64(bytes, &mut cursor)?;
    require_end(bytes, cursor)?;
    validate_terminal(revision, memory_version)?;
    Ok(Some(MemoryPublication::Tombstone {
        id,
        revision,
        memory_version,
    }))
}

/// Canonical source-history evidence shared with the existing marker-bound form.
/// This reconstructs comparison bytes only; it does not persist an old carrier.
pub(crate) fn history_evidence(
    payload: &[u8],
) -> Result<Option<(u64, Vec<u8>, Vec<u8>)>, MemoryError> {
    let Some(envelope) = crate::container::publication_envelope::decode(payload)? else {
        return Ok(None);
    };
    let Some(publication) = decode(envelope.body, envelope.global_version)? else {
        return Ok(None);
    };
    let (local, carrier) = match publication {
        MemoryPublication::Creation(creation) => (
            creation.record.memory_version,
            super::encode_creation(&creation)?,
        ),
        MemoryPublication::Delta(delta) => (delta.memory_version, super::encode_delta(&delta)?),
        MemoryPublication::Tombstone {
            id,
            revision,
            memory_version,
        } => (
            memory_version,
            crate::memory_codec::encode_tombstone(crate::memory_codec::MemoryTombstone {
                id,
                revision,
            }),
        ),
    };
    let marker = crate::memory_codec::encode_version(crate::memory_codec::MemoryVersion {
        global_version: envelope.global_version,
        memory_version: local,
        record: crate::ObjectRef::from_legacy_bytes([0; 16]),
    });
    Ok(Some((envelope.global_version, marker, carrier)))
}

/// Inspect this owner's direct publication coordinates, never inert carriers.
/// Callers resolving accepted facts must still consult the admitted Memory owner.
pub(crate) fn coordinates(payload: &[u8]) -> Result<Option<(u64, u64)>, MemoryError> {
    let Some(envelope) = crate::container::publication_envelope::decode(payload)? else {
        return Ok(None);
    };
    let Some(publication) = decode(envelope.body, envelope.global_version)? else {
        return Ok(None);
    };
    let local = match publication {
        MemoryPublication::Creation(creation) => creation.record.memory_version,
        MemoryPublication::Delta(delta) => delta.memory_version,
        MemoryPublication::Tombstone { memory_version, .. } => memory_version,
    };
    Ok(Some((envelope.global_version, local)))
}

fn validate_terminal(revision: u64, memory_version: u64) -> Result<(), MemoryError> {
    if revision == 0 || memory_version == 0 {
        return Err(MemoryError::InvalidVersion);
    }
    Ok(())
}

#[cfg(test)]
#[path = "memory_publication_codec_tests.rs"]
mod tests;
