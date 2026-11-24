use super::identity_receipt::IdentityHistory;
use super::*;
use crate::model::identity::{
    IdentityEndpointProof, IdentityPair, IdentitySeparationAction, IdentitySeparationEvent,
    MAX_IDENTITY_SEPARATION_OPERATOR_ID_BYTES,
};
use reliquary_perception_store::PRINCIPAL_ENTITY_KIND;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const MARKER: &[u8; 8] = b"RELIDS01";
pub(super) const PREFIX: &[u8] = b"RELIDS";
const MAX_EVENT_BYTES: usize = 4096;

#[derive(Default, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct SeparationHistory {
    entries: Vec<StoredSeparation>,
    by_pair: BTreeMap<IdentityPair, Vec<usize>>,
    by_action: BTreeMap<[u8; 16], BTreeMap<String, usize>>,
}

#[derive(Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct StoredSeparation {
    pub(super) publication_version: u64,
    pub(super) memory_cut: u64,
    pub(super) perception_cut: u64,
    pub(super) identity_cut: u64,
    pub(super) event: IdentitySeparationEvent,
}

impl SeparationHistory {
    pub(super) fn frontier(&self) -> u64 {
        self.entries.len() as u64
    }

    pub(super) fn at(&self, frontier: u64) -> Option<&[StoredSeparation]> {
        self.entries.get(..usize::try_from(frontier).ok()?)
    }

    fn get(&self, pair: &IdentityPair, revision: Option<u64>) -> Option<&IdentitySeparationEvent> {
        let history = self.by_pair.get(pair)?;
        let offset = match revision {
            Some(revision) => history.get(usize::try_from(revision.checked_sub(1)?).ok()?)?,
            None => history.last()?,
        };
        Some(&self.entries[*offset].event)
    }

    fn by_action(
        &self,
        action_id: [u8; 16],
        operator_id: &str,
    ) -> Option<&IdentitySeparationEvent> {
        let offset = self.by_action.get(&action_id)?.get(operator_id)?;
        Some(&self.entries[*offset].event)
    }

    fn check_successor(&self, event: &IdentitySeparationEvent) -> Result<(), OwnerSessionError> {
        let head = self.get(&event.pair, None);
        let predecessor = head.map(|head| head.revision);
        if event.predecessor != predecessor
            || predecessor.unwrap_or(0).checked_add(1) != Some(event.revision)
        {
            return Err(invalid("revision chain"));
        }
        if let IdentitySeparationAction::Undo { target_revision } = event.action
            && !head.is_some_and(|head| {
                target_revision == head.revision
                    && matches!(head.action, IdentitySeparationAction::Confirm { .. })
            })
        {
            return Err(invalid("undo must withdraw the current confirmation"));
        }
        if self
            .by_action(event.action_id, &event.operator_id)
            .is_some()
        {
            return Err(invalid("action ID already used by operator"));
        }
        Ok(())
    }

    fn push(&mut self, stored: StoredSeparation) {
        let event = &stored.event;
        let offset = self.entries.len();
        self.by_pair.entry(event.pair).or_default().push(offset);
        self.by_action
            .entry(event.action_id)
            .or_default()
            .insert(event.operator_id.clone(), offset);
        self.entries.push(stored);
    }

    pub(super) fn replay(
        &mut self,
        payload: &[u8],
        projection: SeparationProjection<'_>,
    ) -> Result<bool, OwnerSessionError> {
        let envelope = reliquary_container::decode_publication(payload)?;
        let body = envelope.as_ref().map_or(payload, |value| value.body);
        if !body.starts_with(PREFIX) {
            return Ok(false);
        }
        let envelope = envelope
            .filter(|envelope| !envelope.bound_reservation)
            .ok_or_else(|| invalid("missing allocating publication envelope"))?;
        let event = decode(body)?;
        self.check_successor(&event)?;
        projection.check(&event)?;
        self.push(projection.stored(envelope.version, event));
        Ok(true)
    }
}

pub(super) struct SeparationProjection<'a> {
    pub(super) owner: [u8; 16],
    pub(super) history_generation: [u8; 32],
    pub(super) receipts: &'a IdentityHistory,
    pub(super) perception: &'a PerceptionStore,
    pub(super) memory_cut: u64,
    pub(super) perception_cut: u64,
}

impl SeparationProjection<'_> {
    fn check(&self, event: &IdentitySeparationEvent) -> Result<(), OwnerSessionError> {
        let pair = &event.pair;
        let operator = &event.operator_id;
        if pair.local_owner != self.owner
            || pair.peer_owner == self.owner
            || pair.peer_owner == [0; 16]
            || pair.peer_entity.0 == [0; 32]
            || event.action_id == [0; 16]
            || operator.is_empty()
            || operator.len() > MAX_IDENTITY_SEPARATION_OPERATOR_ID_BYTES
            || operator.trim() != operator
            || operator.chars().any(char::is_control)
        {
            return Err(invalid("owner, peer endpoint or action identity"));
        }
        let IdentitySeparationAction::Confirm {
            receipt_revision,
            receipt_fingerprint,
            left,
            right,
        } = event.action
        else {
            return Ok(());
        };
        if !self.receipts.get(pair, None).is_some_and(|receipt| {
            receipt.revision == receipt_revision
                && receipt.fingerprint == receipt_fingerprint
                && receipt.left.endpoint_revision == left.revision
                && receipt.right.endpoint_revision == right.revision
        }) {
            return Err(invalid(
                "comparison receipt differs from the publication prefix",
            ));
        }
        let local = self
            .perception
            .entity_at_cut(pair.local_entity, self.perception_cut)
            .filter(|entity| entity.revision == left.revision)
            .and_then(|entity| endpoint_proof(self.perception, self.history_generation, entity));
        if local != Some(left) {
            return Err(invalid(
                "local Entity proof differs from the publication prefix",
            ));
        }
        if right.history_generation == [0; 32] || right.digest == [0; 32] {
            return Err(invalid("invalid peer Entity proof"));
        }
        Ok(())
    }

    fn stored(&self, publication_version: u64, event: IdentitySeparationEvent) -> StoredSeparation {
        StoredSeparation {
            publication_version,
            memory_cut: self.memory_cut,
            perception_cut: self.perception_cut,
            identity_cut: self.receipts.frontier(),
            event,
        }
    }
}

impl RelSession {
    pub fn identity_separation(
        &self,
        pair: &IdentityPair,
        revision: Option<u64>,
    ) -> Option<&IdentitySeparationEvent> {
        self.identity_separation_history.get(pair, revision)
    }

    pub fn identity_separation_by_action(
        &self,
        action_id: [u8; 16],
        operator_id: &str,
    ) -> Option<&IdentitySeparationEvent> {
        self.identity_separation_history
            .by_action(action_id, operator_id)
    }

    /// Resolve the current active Entity or a particular retained revision.
    pub fn identity_endpoint_proof(
        &self,
        entity: EntityId,
        revision: Option<u64>,
    ) -> Option<IdentityEndpointProof> {
        let perception = &self.common.perception;
        let entity = match revision {
            Some(revision) => perception.entity_revision(entity, revision)?,
            None => perception.entity(entity)?,
        };
        endpoint_proof(perception, self.history_generation, entity)
    }
}

impl RelWriter {
    /// Publish within the recording owner's writer lease. The caller fences
    /// operator authority and revalidates the peer at the publication boundary.
    pub fn publish_identity_separation(
        &mut self,
        event: IdentitySeparationEvent,
    ) -> Result<IdentitySeparationEvent, OwnerSessionError> {
        self.ensure_write_ready()?;
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(invalid("owner writer requires reopen"));
        }
        let history = &mut self.session.identity_separation_history;
        if let Some(stored) = history.by_action(event.action_id, &event.operator_id) {
            if stored == &event {
                return Ok(stored.clone());
            }
            return Err(invalid("action ID already used by operator"));
        }
        history.check_successor(&event)?;
        let projection = SeparationProjection {
            owner: self.session.owner_id,
            history_generation: self.session.history_generation,
            receipts: &self.session.identity_history,
            perception: &self.session.common.perception,
            memory_cut: self.session.common.memories.memory_version(),
            perception_cut: self.session.common.perception.version(),
        };
        projection.check(&event)?;
        let payload = encode(&event)?;
        let publication = self
            .container
            .append_publication_envelope(&payload, None)
            .and_then(|(version, _)| self.container.sync().map(|()| version));
        let publication_version = match publication {
            Ok(version) => version,
            Err(error) => {
                self.session.write_poisoned = true;
                return Err(error.into());
            }
        };
        history.push(projection.stored(publication_version, event.clone()));
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        Ok(event)
    }
}

fn endpoint_proof(
    perception: &PerceptionStore,
    history_generation: [u8; 32],
    reviewed: &Entity,
) -> Option<IdentityEndpointProof> {
    if reviewed.kind == PRINCIPAL_ENTITY_KIND {
        return None;
    }
    let mut digest = Sha256::new();
    digest.update(b"reliquary-entity-separation-endpoint-v1\0");
    digest.update(history_generation);
    digest.update(reviewed.id.0);
    digest.update(reviewed.revision.to_le_bytes());
    // Include each predecessor to distinguish histories that converge on an
    // identical later snapshot. A verified description prune retains its digest.
    for revision in 1..=reviewed.revision {
        let entity = perception.entity_revision(reviewed.id, revision)?;
        digest.update(entity.created_at_ns.to_le_bytes());
        digest.update(entity.updated_at_ns.to_le_bytes());
        hash_text(&mut digest, &entity.mutation_id);
        hash_text(&mut digest, &entity.kind);
        hash_text(&mut digest, &entity.canonical_name);
        digest.update((entity.aliases.len() as u64).to_le_bytes());
        for alias in &entity.aliases {
            hash_text(&mut digest, alias);
        }
        let summary = perception
            .description_prune(reviewed.id, revision)
            .map_or_else(
                || Sha256::digest(entity.summary.as_bytes()).into(),
                |proof| proof.summary_digest,
            );
        digest.update(summary);
    }
    Some(IdentityEndpointProof {
        digest: digest.finalize().into(),
        history_generation,
        revision: reviewed.revision,
    })
}

fn hash_text(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value.as_bytes());
}

fn invalid(message: &str) -> OwnerSessionError {
    OwnerSessionError::Record(format!("invalid Entity separation event: {message}"))
}

fn encode(event: &IdentitySeparationEvent) -> Result<Vec<u8>, OwnerSessionError> {
    let mut body = MARKER.to_vec();
    body.extend(rmp_serde::to_vec_named(event).map_err(|_| invalid("encoding"))?);
    if body.len() > MAX_EVENT_BYTES {
        return Err(invalid("record size"));
    }
    Ok(body)
}

fn decode(body: &[u8]) -> Result<IdentitySeparationEvent, OwnerSessionError> {
    if body.len() > MAX_EVENT_BYTES || !body.starts_with(MARKER) {
        return Err(invalid("record type or size"));
    }
    let event = rmp_serde::from_slice(&body[MARKER.len()..]).map_err(|_| invalid("encoding"))?;
    if encode(&event)? != body {
        return Err(invalid("noncanonical encoding"));
    }
    Ok(event)
}

#[cfg(test)]
mod tests;
