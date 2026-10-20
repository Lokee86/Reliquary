//! Plan canonical compact replacements for selected Relationship descriptions.
use crate::relationship_compact_codec::publication::{self, RelationshipPublication};
use crate::relationship_compact_codec::{decode_creation, decode_delta};
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
        let Some((global, local)) = publication::coordinates(&payload)? else {
            continue;
        };
        let Some(publication) = store.retained_publication_at(global, local)? else {
            continue;
        };
        let envelope = crate::container::publication_envelope::decode(&payload)?;
        let carrier = match envelope {
            Some(_) => object,
            None => {
                crate::relationship_codec::decode_version(&payload)?
                    .ok_or(RelationshipError::InvalidVersion)?
                    .record
            }
        };
        match publication {
            RelationshipReplayPublication::Record(record) if selected.contains(&record.id) => {
                let original = if let Some(envelope) = envelope {
                    let body = publication::decode(envelope.body, envelope.global_version)?
                        .ok_or(RelationshipError::InvalidVersion)?;
                    publication::comparison_carrier(&body)?
                } else {
                    container.read(carrier)?
                };
                let proof = RelationshipDescriptionProof {
                    original_carrier_digest: Sha256::digest(&original).into(),
                    manifest_digest,
                };
                let mut redacted = record.clone();
                redacted.summary.clear();

                let replacement_publication = if record.revision == 1 {
                    if previous_redacted.contains_key(&record.id) {
                        return Err(RelationshipError::RevisionConflict);
                    }
                    RelationshipPublication::Creation(RelationshipCreation::new_pruned(
                        redacted.clone(),
                        proof,
                    )?)
                } else {
                    let previous = previous_redacted
                        .get(&record.id)
                        .ok_or(RelationshipError::RevisionConflict)?;
                    RelationshipPublication::Delta(RelationshipDelta::between_pruned(
                        previous, &redacted, proof,
                    )?)
                };

                let replacement = if let Some(envelope) = envelope {
                    let body = publication::encode(&replacement_publication)?;
                    crate::container::publication_envelope::encode(
                        crate::container::publication_envelope::PublicationEnvelope {
                            global_version: envelope.global_version,
                            clock: envelope.clock,
                            body: &body,
                        },
                    )?
                } else {
                    publication::comparison_carrier(&replacement_publication)?
                };

                if plan.replacements.insert(carrier, replacement).is_some() {
                    return Err(RelationshipError::CorruptRecord(
                        "duplicate accepted Relationship carrier",
                    ));
                }
                accepted_carriers.insert(carrier);
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
