//! Entity-owned bodies within physical publication envelopes.
//! The envelope supplies global order; Entity retains all semantic fields.
use super::*;

const CREATION: &[u8; 8] = b"CVAENC02";
const DELTA: &[u8; 8] = b"CVAEND02";
const DIRECT: &[u8; 8] = b"CVAETD02";
const MERGE: &[u8; 8] = b"CVAETM02";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum EntityPublication {
    Creation(EntityCreation),
    Delta(EntityDelta),
    DirectTombstone {
        id: crate::EntityId,
        revision: u64,
        entity_version: u64,
    },
    MergeTombstone {
        id: crate::EntityId,
        revision: u64,
        entity_version: u64,
        replacement_id: crate::EntityId,
    },
}

pub(crate) fn encode(publication: &EntityPublication) -> Result<Vec<u8>, EntityError> {
    let mut out = Vec::new();
    match publication {
        EntityPublication::Creation(creation) => {
            validate_creation(creation)?;
            let record = &creation.record;
            out.extend_from_slice(CREATION);
            out.extend_from_slice(&record.id.0);
            write_optional_global_id(&mut out, record.global_id);
            out.extend_from_slice(&record.entity_version.to_le_bytes());
            write_string(&mut out, &record.mutation_id)?;
            write_string(&mut out, &record.canonical_name)?;
            write_strings(&mut out, &record.aliases)?;
            write_string(&mut out, &record.kind)?;
            write_string(&mut out, &record.summary)?;
            out.extend_from_slice(&record.created_at_ns.to_le_bytes());
            out.extend_from_slice(&record.updated_at_ns.to_le_bytes());
            write_description_proof(&mut out, creation.description_proof.as_ref());
            if creation.description_proof.is_some() {
                append_checksum(&mut out);
            }
        }
        EntityPublication::Delta(delta) => {
            validate_delta_envelope(delta)?;
            out.extend_from_slice(DELTA);
            out.extend_from_slice(&delta.id.0);
            for value in [
                delta.previous_revision,
                delta.revision,
                delta.entity_version,
            ] {
                out.extend_from_slice(&value.to_le_bytes());
            }
            write_string(&mut out, &delta.mutation_id)?;
            out.extend_from_slice(&delta.fields.mask().to_le_bytes());
            write_delta_fields(&delta.fields, &mut out)?;
            write_description_proof(&mut out, delta.description_proof.as_ref());
            if delta.description_proof.is_some() {
                append_checksum(&mut out);
            }
        }
        EntityPublication::DirectTombstone {
            id,
            revision,
            entity_version,
        } => {
            validate_terminal(*revision, *entity_version)?;
            out.extend_from_slice(DIRECT);
            out.extend_from_slice(&id.0);
            out.extend_from_slice(&revision.to_le_bytes());
            out.extend_from_slice(&entity_version.to_le_bytes());
        }
        EntityPublication::MergeTombstone {
            id,
            revision,
            entity_version,
            replacement_id,
        } => {
            validate_terminal(*revision, *entity_version)?;
            if id == replacement_id {
                return Err(EntityError::RevisionConflict);
            }
            out.extend_from_slice(MERGE);
            out.extend_from_slice(&id.0);
            out.extend_from_slice(&revision.to_le_bytes());
            out.extend_from_slice(&entity_version.to_le_bytes());
            out.extend_from_slice(&replacement_id.0);
        }
    }
    Ok(out)
}

pub(crate) fn decode(
    bytes: &[u8],
    global_version: u64,
) -> Result<Option<EntityPublication>, EntityError> {
    let creation = bytes.starts_with(b"CVAENC");
    let delta = bytes.starts_with(b"CVAEND");
    let direct = bytes.starts_with(b"CVAETD");
    let merge = bytes.starts_with(b"CVAETM");
    if !(creation || delta || direct || merge) {
        return Ok(None);
    }
    if global_version == 0 {
        return Err(EntityError::InvalidVersion);
    }
    let mut cursor = 8;
    if creation {
        require_magic(bytes, CREATION)?;
        let id = read_entity_id(bytes, &mut cursor)?;
        let global_id = read_optional_global_id(bytes, &mut cursor)?;
        let entity_version = read_u64(bytes, &mut cursor)?;
        let mutation_id = read_string(bytes, &mut cursor)?;
        let canonical_name = read_string(bytes, &mut cursor)?;
        let aliases = <Vec<String> as EntityValue>::read(bytes, &mut cursor)?;
        let kind = read_string(bytes, &mut cursor)?;
        let summary = read_string(bytes, &mut cursor)?;
        let created_at_ns = read_i64(bytes, &mut cursor)?;
        let updated_at_ns = read_i64(bytes, &mut cursor)?;
        let description_proof = read_description_proof(bytes, &mut cursor)?;
        finish_frame(bytes, cursor, description_proof.is_some())?;
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
        return Ok(Some(EntityPublication::Creation(creation)));
    }
    if delta {
        require_magic(bytes, DELTA)?;
        let id = read_entity_id(bytes, &mut cursor)?;
        let previous_revision = read_u64(bytes, &mut cursor)?;
        let revision = read_u64(bytes, &mut cursor)?;
        let entity_version = read_u64(bytes, &mut cursor)?;
        let mutation_id = read_string(bytes, &mut cursor)?;
        let mask = read_u64(bytes, &mut cursor)?;
        let fields = read_delta_fields(bytes, &mut cursor, mask)?;
        let description_proof = read_description_proof(bytes, &mut cursor)?;
        finish_frame(bytes, cursor, description_proof.is_some())?;
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
        return Ok(Some(EntityPublication::Delta(delta)));
    }
    require_magic(bytes, if direct { DIRECT } else { MERGE })?;
    let id = read_entity_id(bytes, &mut cursor)?;
    let revision = read_u64(bytes, &mut cursor)?;
    let entity_version = read_u64(bytes, &mut cursor)?;
    validate_terminal(revision, entity_version)?;
    let publication = if direct {
        EntityPublication::DirectTombstone {
            id,
            revision,
            entity_version,
        }
    } else {
        let replacement_id = read_entity_id(bytes, &mut cursor)?;
        if id == replacement_id {
            return Err(EntityError::RevisionConflict);
        }
        EntityPublication::MergeTombstone {
            id,
            revision,
            entity_version,
            replacement_id,
        }
    };
    finish_frame(bytes, cursor, false)?;
    Ok(Some(publication))
}

fn validate_terminal(revision: u64, entity_version: u64) -> Result<(), EntityError> {
    if revision < 2 || entity_version == 0 {
        return Err(EntityError::InvalidVersion);
    }
    Ok(())
}

/// Publication coordinates only; consumers resolve facts through EntityStore.
pub(crate) fn coordinates(payload: &[u8]) -> Result<Option<(u64, u64)>, EntityError> {
    let Some(envelope) = reliquary_container::publication_envelope::decode(payload)? else {
        return Ok(None);
    };
    let Some(publication) = decode(envelope.body, envelope.global_version)? else {
        return Ok(None);
    };
    let local = match publication {
        EntityPublication::Creation(creation) => creation.record.entity_version,
        EntityPublication::Delta(delta) => delta.entity_version,
        EntityPublication::DirectTombstone { entity_version, .. }
        | EntityPublication::MergeTombstone { entity_version, .. } => entity_version,
    };
    Ok(Some((envelope.global_version, local)))
}

/// Comparison-only authority in the established marker-bound representation.
pub(crate) fn comparison_carrier(publication: &EntityPublication) -> Result<Vec<u8>, EntityError> {
    match publication {
        EntityPublication::Creation(creation) => super::encode_creation(creation),
        EntityPublication::Delta(delta) => super::encode_delta(delta),
        EntityPublication::DirectTombstone { id, revision, .. } => {
            Ok(crate::entity_codec::encode_direct_tombstone(
                crate::entity_codec::DirectEntityTombstone {
                    id: *id,
                    revision: *revision,
                },
            ))
        }
        EntityPublication::MergeTombstone {
            id,
            revision,
            replacement_id,
            ..
        } => Ok(crate::entity_codec::encode_tombstone(
            crate::entity_codec::EntityTombstone {
                id: *id,
                revision: *revision,
                replacement_id: *replacement_id,
            },
        )),
    }
}

pub(crate) fn history_evidence(
    payload: &[u8],
) -> Result<Option<(u64, Vec<u8>, [u8; 32])>, EntityError> {
    let Some(envelope) = reliquary_container::publication_envelope::decode(payload)? else {
        return Ok(None);
    };
    let Some(publication) = decode(envelope.body, envelope.global_version)? else {
        return Ok(None);
    };
    let (local, proof) = match &publication {
        EntityPublication::Creation(creation) => (
            creation.record.entity_version,
            creation.description_proof.as_ref(),
        ),
        EntityPublication::Delta(delta) => (delta.entity_version, delta.description_proof.as_ref()),
        EntityPublication::DirectTombstone { entity_version, .. }
        | EntityPublication::MergeTombstone { entity_version, .. } => (*entity_version, None),
    };
    let digest = match proof {
        Some(proof) => proof.original_carrier_digest,
        None => Sha256::digest(comparison_carrier(&publication)?).into(),
    };
    let marker = crate::entity_codec::encode_version(crate::entity_codec::EntityVersion {
        global_version: envelope.global_version,
        entity_version: local,
        record: reliquary_container::ObjectRef::from_legacy_bytes([0; 16]),
    });
    Ok(Some((envelope.global_version, marker, digest)))
}

#[cfg(test)]
#[path = "entity_publication_codec_tests.rs"]
mod tests;
