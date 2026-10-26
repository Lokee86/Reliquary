//! Plan canonical compact replacements for selected Relationship descriptions.
use crate::relationship_compact_codec::publication::{self, RelationshipPublication};
use crate::relationship_compact_contract::{
    RelationshipCreation, RelationshipDelta, RelationshipDescriptionProof,
};
use crate::relationship_model::RelationshipRecord;
use crate::relationship_store::{RelationshipReplayPublication, RelationshipStore};
use crate::{RelationshipError, RelationshipId};
use reliquary_container::{Container, ObjectRef};
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
    let mut seen_selected = HashSet::<RelationshipId>::new();

    // Walk accepted publications in the physical marker order. That order is
    // the source of truth for compact delta predecessors, including late-bound
    // global tickets that need not be numerically increasing.
    for object in container.chunks()? {
        let payload = container.read(object)?;
        let Some((global, local)) = publication::coordinates(&payload)? else {
            // A bare current body has no accepted publication clock. The probe
            // value is used only for structural decoding, never for admission.
            let id = match publication::decode(&payload, 1)? {
                Some(RelationshipPublication::Creation(c)) => Some(c.record.id),
                Some(RelationshipPublication::Delta(d)) => Some(d.id),
                _ => None,
            };
            if id.is_some_and(|id| selected.contains(&id)) {
                plan.discard.insert(object);
            }
            continue;
        };
        let Some(publication) = store.retained_publication_at(global, local)? else {
            continue;
        };
        let envelope = reliquary_container::publication_envelope::decode(&payload)?
            .ok_or(RelationshipError::InvalidVersion)?;
        let carrier = object;
        match publication {
            RelationshipReplayPublication::Record(record) if selected.contains(&record.id) => {
                let body = publication::decode(envelope.body, envelope.global_version)?
                    .ok_or(RelationshipError::InvalidVersion)?;
                let original = publication::comparison_carrier(&body)?;
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

                let body = publication::encode(&replacement_publication)?;
                let replacement = reliquary_container::publication_envelope::encode(
                    reliquary_container::publication_envelope::PublicationEnvelope {
                        body: &body,
                        ..envelope
                    },
                )?;

                if plan.replacements.insert(carrier, replacement).is_some() {
                    return Err(RelationshipError::CorruptRecord(
                        "duplicate accepted Relationship carrier",
                    ));
                }
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

    Ok(plan)
}
