//! Plan canonical compact replacements for selected Relationship descriptions.
use crate::relationship_codec::decode_version;
use crate::relationship_compact_codec::{
    decode_creation, decode_delta, encode_creation, encode_delta,
};
use crate::relationship_compact_contract::{
    RelationshipCreation, RelationshipDelta, RelationshipDescriptionProof,
};
use crate::relationship_model::RelationshipRecord;
use crate::relationship_store::{RelationshipReplayPublication, RelationshipStore};
use crate::{Container, ObjectRef, RelationshipError, RelationshipId};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

pub(crate) struct RelationshipDescriptionPrunePlan {
    pub(crate) replacements: HashMap<ObjectRef, Vec<u8>>,
    pub(crate) discard: HashSet<ObjectRef>,
}

/// Prepare redacted carriers without writing to the source container or allocating clocks.
pub(crate) fn prepare_description_prune(
    store: &RelationshipStore,
    container: &mut Container,
    selected: &HashSet<RelationshipId>,
    manifest_digest: [u8; 32],
) -> Result<RelationshipDescriptionPrunePlan, RelationshipError> {
    let mut plan = RelationshipDescriptionPrunePlan {
        replacements: HashMap::new(),
        discard: HashSet::new(),
    };
    if selected.is_empty() {
        return Ok(plan);
    }

    let mut previous_redacted = HashMap::<RelationshipId, RelationshipRecord>::new();
    let mut accepted_carriers = HashSet::<ObjectRef>::new();
    let mut seen_selected = HashSet::<RelationshipId>::new();

    // Walk accepted publications in the physical marker order. That order is
    // the source of truth for compact delta predecessors, including late-bound
    // global tickets that need not be numerically increasing.
    for object in container.chunks()? {
        let payload = container.read(object)?;
        let Some(version) = decode_version(&payload)? else {
            continue;
        };
        let Some(publication) =
            store.retained_publication_at(version.global_version, version.relationship_version)?
        else {
            continue;
        };
        match publication {
            RelationshipReplayPublication::Record(record) if selected.contains(&record.id) => {
                let original = container.read(version.record)?;
                let proof = RelationshipDescriptionProof {
                    original_carrier_digest: Sha256::digest(&original).into(),
                    manifest_digest,
                };
                let mut redacted = record.clone();
                redacted.summary.clear();

                let replacement = if record.revision == 1 {
                    if previous_redacted.contains_key(&record.id) {
                        return Err(RelationshipError::RevisionConflict);
                    }
                    encode_creation(&RelationshipCreation::new_pruned(redacted.clone(), proof)?)?
                } else {
                    let previous = previous_redacted
                        .get(&record.id)
                        .ok_or(RelationshipError::RevisionConflict)?;
                    encode_delta(&RelationshipDelta::between_pruned(
                        previous, &redacted, proof,
                    )?)?
                };

                if plan
                    .replacements
                    .insert(version.record, replacement)
                    .is_some()
                {
                    return Err(RelationshipError::CorruptRecord(
                        "duplicate accepted Relationship carrier",
                    ));
                }
                accepted_carriers.insert(version.record);
                previous_redacted.insert(record.id, redacted);
                seen_selected.insert(record.id);
            }
            RelationshipReplayPublication::PrunedRecord { id, .. } if selected.contains(&id) => {
                return Err(RelationshipError::PayloadUnavailable(
                    crate::owner_lifecycle::PayloadUnavailable::Pruned,
                ));
            }
            _ => {}
        }
    }

    if selected.iter().any(|id| !seen_selected.contains(id)) {
        return Err(RelationshipError::MissingRelationship);
    }

    // Discard unreferenced raw carriers for selected identities so an older
    // plaintext copy cannot survive the rewrite. Unselected carriers are left
    // byte-for-byte alone, and terminal/version frames are never candidates.
    for object in container.chunks()? {
        if accepted_carriers.contains(&object) {
            continue;
        }
        let payload = container.read(object)?;
        let id = if crate::relationship_compact_codec::is_compact_record(&payload)? {
            if payload.starts_with(b"CVARNC") {
                Some(decode_creation(&payload)?.record.id)
            } else if payload.starts_with(b"CVARND") {
                Some(decode_delta(&payload)?.id)
            } else {
                None
            }
        } else {
            crate::relationship_codec::decode_record(&payload)?.map(|record| record.id)
        };
        if id.is_some_and(|id| selected.contains(&id)) {
            plan.discard.insert(object);
        }
    }

    Ok(plan)
}
