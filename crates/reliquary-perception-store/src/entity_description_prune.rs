//! Plan canonical compact replacements for selected Entity descriptions.
use crate::entity_compact_codec::publication::{self, EntityPublication};
use crate::entity_compact_contract::{EntityCreation, EntityDelta, EntityDescriptionProof};
use crate::entity_model::EntityRecord;
use crate::entity_store::{EntityReplayPublication, EntityStore};
use crate::{EntityError, EntityId};
use reliquary_container::{Container, ObjectRef};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

pub struct EntityDescriptionPrunePlan {
    pub replacements: HashMap<ObjectRef, Vec<u8>>,
    pub discard: HashSet<ObjectRef>,
}

/// Prepare redacted carriers without writing to the source container or allocating clocks.
pub fn prepare_description_prune(
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
    let mut seen_selected = HashSet::<EntityId>::new();

    // Walk accepted publications in the physical marker order. That order is
    // the source of truth for compact delta predecessors, including late-bound
    // global tickets that need not be numerically increasing.
    for object in container.chunks()? {
        let payload = container.read(object)?;
        let Some((global, local)) = publication::coordinates(&payload)? else {
            // A bare current body has no accepted publication clock. The probe
            // value is used only for structural decoding, never for admission.
            let id = match publication::decode(&payload, 1)? {
                Some(EntityPublication::Creation(c)) => Some(c.record.id),
                Some(EntityPublication::Delta(d)) => Some(d.id),
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
            .ok_or(EntityError::InvalidVersion)?;
        let carrier = object;
        match publication {
            EntityReplayPublication::Record(record) if selected.contains(&record.id) => {
                if record.kind == crate::entity_identity::PRINCIPAL_ENTITY_KIND {
                    return Err(EntityError::PayloadRequiredByLiveTarget);
                }
                let body = publication::decode(envelope.body, envelope.global_version)?
                    .ok_or(EntityError::InvalidVersion)?;
                let original = publication::comparison_carrier(&body)?;
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
                let body = publication::encode(&replacement_publication)?;
                let replacement = reliquary_container::publication_envelope::encode(
                    reliquary_container::publication_envelope::PublicationEnvelope {
                        body: &body,
                        ..envelope
                    },
                )?;

                if plan.replacements.insert(carrier, replacement).is_some() {
                    return Err(EntityError::CorruptRecord(
                        "duplicate accepted Entity carrier",
                    ));
                }
                previous_redacted.insert(record.id, redacted);
                seen_selected.insert(record.id);
            }
            EntityReplayPublication::PrunedRecord { id, .. } if selected.contains(&id) => {
                return Err(EntityError::PayloadUnavailable(
                    crate::EntityPayloadUnavailable::Pruned,
                ));
            }
            _ => {}
        }
    }

    if selected.iter().any(|id| !seen_selected.contains(id)) {
        return Err(EntityError::MissingEntity);
    }

    Ok(plan)
}
