//! Plan canonical compact replacements for selected Entity descriptions.
use crate::entity_compact_codec::publication::{self, EntityPublication};
use crate::entity_compact_codec::{decode_creation, decode_delta};
use crate::entity_compact_contract::{EntityCreation, EntityDelta, EntityDescriptionProof};
use crate::entity_model::EntityRecord;
use crate::entity_store::{EntityReplayPublication, EntityStore};
use crate::{Container, EntityError, EntityId, ObjectRef};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

pub(crate) struct EntityDescriptionPrunePlan {
    pub(crate) replacements: HashMap<ObjectRef, Vec<u8>>,
    pub(crate) discard: HashSet<ObjectRef>,
}

/// Prepare redacted carriers without writing to the source container or allocating clocks.
pub(crate) fn prepare_description_prune(
    store: &EntityStore,
    container: &mut Container,
    selected: &HashSet<EntityId>,
    manifest_digest: [u8; 32],
) -> Result<EntityDescriptionPrunePlan, EntityError> {
    let mut plan = EntityDescriptionPrunePlan {
        replacements: HashMap::new(),
        discard: HashSet::new(),
    };
    if selected.is_empty() {
        return Ok(plan);
    }

    let mut previous_redacted = HashMap::<EntityId, EntityRecord>::new();
    let mut accepted_carriers = HashSet::<ObjectRef>::new();
    let mut seen_selected = HashSet::<EntityId>::new();

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
                crate::entity_codec::decode_version(&payload)?
                    .ok_or(EntityError::InvalidVersion)?
                    .record
            }
        };
        match publication {
            EntityReplayPublication::Record(record) if selected.contains(&record.id) => {
                if record.kind == crate::entity_principal::PRINCIPAL_ENTITY_KIND {
                    return Err(EntityError::PayloadRequiredByLiveTarget);
                }
                let original = if let Some(envelope) = envelope {
                    let body = publication::decode(envelope.body, envelope.global_version)?
                        .ok_or(EntityError::InvalidVersion)?;
                    publication::comparison_carrier(&body)?
                } else {
                    container.read(carrier)?
                };
                let proof = EntityDescriptionProof {
                    original_carrier_digest: Sha256::digest(&original).into(),
                    manifest_digest,
                };
                let mut redacted = record.clone();
                redacted.canonical_name = "[pruned]".into();
                redacted.aliases.clear();
                redacted.summary.clear();

                let replacement_publication = if record.revision == 1 {
                    if previous_redacted.contains_key(&record.id) {
                        return Err(EntityError::RevisionConflict);
                    }
                    EntityPublication::Creation(EntityCreation::new_pruned(
                        redacted.clone(),
                        proof,
                    )?)
                } else {
                    let previous = previous_redacted
                        .get(&record.id)
                        .ok_or(EntityError::RevisionConflict)?;
                    EntityPublication::Delta(EntityDelta::between_pruned(
                        previous, &redacted, proof,
                    )?)
                };
                let replacement = if let Some(envelope) = envelope {
                    let body = publication::encode(&replacement_publication)?;
                    crate::container::publication_envelope::encode(
                        crate::container::publication_envelope::PublicationEnvelope {
                            body: &body,
                            ..envelope
                        },
                    )?
                } else {
                    publication::comparison_carrier(&replacement_publication)?
                };

                if plan.replacements.insert(carrier, replacement).is_some() {
                    return Err(EntityError::CorruptRecord(
                        "duplicate accepted Entity carrier",
                    ));
                }
                accepted_carriers.insert(carrier);
                previous_redacted.insert(record.id, redacted);
                seen_selected.insert(record.id);
            }
            EntityReplayPublication::PrunedRecord { id, .. } if selected.contains(&id) => {
                return Err(EntityError::PayloadUnavailable(
                    crate::owner_lifecycle::PayloadUnavailable::Pruned,
                ));
            }
            _ => {}
        }
    }

    if selected.iter().any(|id| !seen_selected.contains(id)) {
        return Err(EntityError::MissingEntity);
    }

    // Discard unreferenced raw carriers for selected identities so an older
    // plaintext copy cannot survive the rewrite. Unselected carriers are left
    // byte-for-byte alone, and terminal/version frames are never candidates.
    for object in container.chunks()? {
        if accepted_carriers.contains(&object) {
            continue;
        }
        let payload = container.read(object)?;
        let id = if crate::entity_compact_codec::is_compact_record(&payload)? {
            if payload.starts_with(b"CVAENC") {
                Some(decode_creation(&payload)?.record.id)
            } else if payload.starts_with(b"CVAEND") {
                Some(decode_delta(&payload)?.id)
            } else {
                None
            }
        } else {
            crate::entity_codec::decode_record(&payload)?.map(|record| record.id)
        };
        if id.is_some_and(|id| selected.contains(&id)) {
            plan.discard.insert(object);
        }
    }

    Ok(plan)
}
