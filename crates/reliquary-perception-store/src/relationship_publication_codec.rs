//! Relationship-owned bodies carried by physical publication envelopes.
//! The envelope supplies global order; Relationship retains local semantics.
use super::*;

const CREATION: &[u8; 8] = b"CVARNC02";
const DELTA: &[u8; 8] = b"CVARND02";
const TOMBSTONE: &[u8; 8] = b"CVARTD02";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RelationshipPublication {
    Creation(RelationshipCreation),
    Delta(RelationshipDelta),
    Tombstone {
        id: RelationshipId,
        revision: u64,
        relationship_version: u64,
    },
}

pub(crate) fn encode(publication: &RelationshipPublication) -> Result<Vec<u8>, RelationshipError> {
    let mut out = Vec::new();
    match publication {
        RelationshipPublication::Creation(creation) => {
            validate_creation(creation)?;
            let record = &creation.record;
            out.extend_from_slice(CREATION);
            out.extend_from_slice(&record.id.0);
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
        }
        RelationshipPublication::Delta(delta) => {
            validate_delta(delta)?;
            out.extend_from_slice(DELTA);
            out.extend_from_slice(&delta.id.0);
            out.extend_from_slice(&delta.previous_revision.to_le_bytes());
            out.extend_from_slice(&delta.revision.to_le_bytes());
            out.extend_from_slice(&delta.relationship_version.to_le_bytes());
            write_string(&mut out, &delta.mutation_id)?;
            let mask = delta.fields.mask();
            out.extend_from_slice(&mask.to_le_bytes());
            write_delta_fields(&delta.fields, mask, &mut out)?;
            write_proof(&mut out, delta.description_proof.as_ref());
            if delta.description_proof.is_some() {
                append_checksum(&mut out);
            }
        }
        RelationshipPublication::Tombstone {
            id,
            revision,
            relationship_version,
        } => {
            validate_terminal(*revision, *relationship_version)?;
            out.extend_from_slice(TOMBSTONE);
            out.extend_from_slice(&id.0);
            out.extend_from_slice(&revision.to_le_bytes());
            out.extend_from_slice(&relationship_version.to_le_bytes());
        }
    }
    Ok(out)
}

pub(crate) fn decode(
    bytes: &[u8],
    global_version: u64,
) -> Result<Option<RelationshipPublication>, RelationshipError> {
    let creation = bytes.starts_with(b"CVARNC");
    let delta = bytes.starts_with(b"CVARND");
    let tombstone = bytes.starts_with(b"CVARTD");
    if !(creation || delta || tombstone) {
        return Ok(None);
    }
    if global_version == 0 {
        return Err(RelationshipError::InvalidVersion);
    }

    let mut cursor = 8;
    if creation {
        require_magic(bytes, CREATION)?;
        let id = RelationshipId(read_fixed::<32>(
            bytes,
            &mut cursor,
            "truncated compact Relationship ID",
        )?);
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
        let creation = match description_proof {
            Some(proof) => RelationshipCreation::new_pruned(record, proof)?,
            None => RelationshipCreation::new(record)?,
        };
        return Ok(Some(RelationshipPublication::Creation(creation)));
    }

    if delta {
        require_magic(bytes, DELTA)?;
        let id = RelationshipId(read_fixed::<32>(
            bytes,
            &mut cursor,
            "truncated compact Relationship ID",
        )?);
        let previous_revision = read_u64(bytes, &mut cursor)?;
        let revision = read_u64(bytes, &mut cursor)?;
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
        return Ok(Some(RelationshipPublication::Delta(delta)));
    }

    require_magic(bytes, TOMBSTONE)?;
    let id = RelationshipId(read_fixed::<32>(
        bytes,
        &mut cursor,
        "truncated compact Relationship ID",
    )?);
    let revision = read_u64(bytes, &mut cursor)?;
    let relationship_version = read_u64(bytes, &mut cursor)?;
    validate_terminal(revision, relationship_version)?;
    finish_frame(bytes, cursor, false)?;
    Ok(Some(RelationshipPublication::Tombstone {
        id,
        revision,
        relationship_version,
    }))
}

fn validate_terminal(revision: u64, relationship_version: u64) -> Result<(), RelationshipError> {
    if revision < 2 || relationship_version == 0 {
        return Err(RelationshipError::InvalidVersion);
    }
    Ok(())
}

/// Publication coordinates only; consumers resolve semantic facts in the owner store.
pub(crate) fn coordinates(payload: &[u8]) -> Result<Option<(u64, u64)>, RelationshipError> {
    let Some(envelope) = reliquary_container::publication_envelope::decode(payload)? else {
        return Ok(None);
    };
    let Some(publication) = decode(envelope.body, envelope.global_version)? else {
        return Ok(None);
    };
    let local = match publication {
        RelationshipPublication::Creation(creation) => creation.record.relationship_version,
        RelationshipPublication::Delta(delta) => delta.relationship_version,
        RelationshipPublication::Tombstone {
            relationship_version,
            ..
        } => relationship_version,
    };
    Ok(Some((envelope.global_version, local)))
}

/// Comparison-only carrier in the established marker-bound representation.
pub(crate) fn comparison_carrier(
    publication: &RelationshipPublication,
) -> Result<Vec<u8>, RelationshipError> {
    match publication {
        RelationshipPublication::Creation(creation) => super::encode_creation(creation),
        RelationshipPublication::Delta(delta) => super::encode_delta(delta),
        RelationshipPublication::Tombstone { id, revision, .. } => {
            Ok(crate::relationship_codec::encode_tombstone(
                crate::relationship_codec::RelationshipTombstone {
                    id: *id,
                    revision: *revision,
                },
            ))
        }
    }
}

pub(crate) fn history_evidence(
    payload: &[u8],
) -> Result<Option<(u64, Vec<u8>, [u8; 32])>, RelationshipError> {
    let Some(envelope) = reliquary_container::publication_envelope::decode(payload)? else {
        return Ok(None);
    };
    let Some(publication) = decode(envelope.body, envelope.global_version)? else {
        return Ok(None);
    };
    let (local, proof) = match &publication {
        RelationshipPublication::Creation(creation) => (
            creation.record.relationship_version,
            creation.description_proof.as_ref(),
        ),
        RelationshipPublication::Delta(delta) => {
            (delta.relationship_version, delta.description_proof.as_ref())
        }
        RelationshipPublication::Tombstone {
            relationship_version,
            ..
        } => (*relationship_version, None),
    };
    let digest = match proof {
        Some(proof) => proof.original_carrier_digest,
        None => Sha256::digest(comparison_carrier(&publication)?).into(),
    };
    let marker =
        crate::relationship_codec::encode_version(crate::relationship_codec::RelationshipVersion {
            global_version: envelope.global_version,
            relationship_version: local,
            record: reliquary_container::ObjectRef::from_legacy_bytes([0; 16]),
        });
    Ok(Some((envelope.global_version, marker, digest)))
}
