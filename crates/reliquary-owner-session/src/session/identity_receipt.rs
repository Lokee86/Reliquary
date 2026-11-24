use super::identity_evidence::{exact_quote_span, quote_names_entity};
use super::perception_lifecycle::support::{has_direct_source, owner_hex, text_field};
use super::*;
use reliquary_perception_store::identity::{
    IdentityEvidenceRef, IdentityLimit, IdentityMemoryRef, IdentityOutcome, IdentityPair,
    IdentityReason, IdentityReceipt, IdentityRelationshipRef, IdentitySide, IdentitySpan,
    MAX_IDENTITY_RELATIONSHIPS_PER_SIDE,
};
use reliquary_perception_store::identity_evidence::{IdentityEvidence, IdentityEvidenceCut};
use reliquary_perception_store::{
    MAX_RELATIONSHIP_EVIDENCE, MAX_RELATIONSHIP_PARTICIPANTS, PRINCIPAL_ENTITY_KIND,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ops::Bound::{Excluded, Unbounded};

pub(crate) const MARKER: &[u8; 8] = b"RELIDR01";
pub(super) const PREFIX: &[u8] = b"RELIDR";
const MAX_RECEIPT_BYTES: usize = 192 * 1024;
const MAX_CITATIONS: usize = 16;

#[derive(Default, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct IdentityHistory {
    entries: Vec<StoredReceipt>,
    by_pair: BTreeMap<IdentityPair, Vec<usize>>,
}

#[derive(Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct StoredReceipt {
    pub(super) publication_version: u64,
    pub(super) memory_cut: u64,
    pub(super) perception_cut: u64,
    pub(super) receipt: IdentityReceipt,
}

impl IdentityHistory {
    pub(super) fn frontier(&self) -> u64 {
        self.entries.len() as u64
    }

    pub(super) fn at(&self, frontier: u64) -> Option<&[StoredReceipt]> {
        self.entries.get(..usize::try_from(frontier).ok()?)
    }

    pub(super) fn get(
        &self,
        pair: &IdentityPair,
        revision: Option<u64>,
    ) -> Option<&IdentityReceipt> {
        let history = self.by_pair.get(pair)?;
        let offset = match revision {
            Some(revision) => history.get(usize::try_from(revision.checked_sub(1)?).ok()?)?,
            None => history.last()?,
        };
        Some(&self.entries[*offset].receipt)
    }

    fn check_successor(&self, receipt: &IdentityReceipt) -> Result<(), OwnerSessionError> {
        let head = self.get(&receipt.pair, None);
        let predecessor = head.map(|head| head.revision);
        if receipt.predecessor != predecessor
            || predecessor.unwrap_or(0).checked_add(1) != Some(receipt.revision)
            || head.is_some_and(|head| same_inputs(head, receipt))
        {
            return Err(invalid("revision chain"));
        }
        Ok(())
    }

    fn push(&mut self, stored: StoredReceipt) {
        self.by_pair
            .entry(stored.receipt.pair)
            .or_default()
            .push(self.entries.len());
        self.entries.push(stored);
    }

    pub(super) fn replay(
        &mut self,
        payload: &[u8],
        owner: [u8; 16],
        projection: LocalProjection<'_>,
    ) -> Result<bool, OwnerSessionError> {
        let envelope = reliquary_container::decode_publication(payload)?;
        let body = envelope.as_ref().map_or(payload, |value| value.body);
        if !body.starts_with(PREFIX) {
            return Ok(false);
        }
        let envelope = envelope
            .filter(|envelope| !envelope.bound_reservation)
            .ok_or_else(|| invalid("missing allocating publication envelope"))?;
        let receipt = decode(body)?;
        validate_structure(&receipt, owner)?;
        self.check_successor(&receipt)?;
        projection.check(&receipt)?;
        self.push(StoredReceipt {
            publication_version: envelope.version,
            memory_cut: projection.memory_cut,
            perception_cut: projection.perception_cut,
            receipt,
        });
        Ok(true)
    }
}

/// Owner-local evidence captured at one admitted cut. Publication rechecks its
/// file identity, exact references and mutable evidence coordinates.
pub struct IdentityPublicationProof {
    owner: [u8; 16],
    file: (u64, u64),
    history_generation: [u8; 32],
    cut: IdentityEvidenceCut,
    entity: EntityId,
    references: IdentityEvidenceRef,
}

impl IdentityPublicationProof {
    fn check(
        &self,
        session: &RelSession,
        receipt: &IdentityReceipt,
    ) -> Result<(), OwnerSessionError> {
        let actual = &receipt.left;
        if self.owner != session.owner_id
            || self.file != session.container_file_identity
            || self.history_generation != session.history_generation
            || self.cut != session.identity_evidence_cut()
            || self.entity != receipt.pair.local_entity
            || self.references.endpoint_revision != actual.endpoint_revision
            || self.references.relationships != actual.relationships
            || !(self.references.limit == actual.limit
                || (self.references.limit.is_none() && actual.limit == Some(IdentityLimit::Budget)))
        {
            return Err(invalid("prepared local evidence changed"));
        }
        Ok(())
    }
}

impl RelSession {
    pub fn identity_evidence_with_proof(
        &self,
        entity: EntityId,
    ) -> Result<Option<(IdentityEvidence, IdentityPublicationProof)>, OwnerSessionError> {
        let Some(evidence) = self.identity_evidence(entity)? else {
            return Ok(None);
        };
        let proof = IdentityPublicationProof {
            owner: self.owner_id,
            file: self.container_file_identity,
            history_generation: self.history_generation,
            cut: self.identity_evidence_cut(),
            entity,
            references: evidence.references(),
        };
        Ok(Some((evidence, proof)))
    }

    pub fn identity_receipt(
        &self,
        pair: &IdentityPair,
        revision: Option<u64>,
    ) -> Option<&IdentityReceipt> {
        self.identity_history.get(pair, revision)
    }

    pub fn identity_pairs_after(
        &self,
        after: Option<&IdentityPair>,
        limit: usize,
    ) -> Vec<IdentityPair> {
        self.identity_history
            .by_pair
            .range((after.map_or(Unbounded, Excluded), Unbounded))
            .take(limit)
            .map(|(pair, _)| *pair)
            .collect()
    }
}

impl RelWriter {
    /// Append a validated decision within this owner's existing writer lease.
    /// The caller must revalidate peer access and inputs at the publication fence.
    pub fn publish_identity_receipt(
        &mut self,
        receipt: IdentityReceipt,
    ) -> Result<IdentityReceipt, OwnerSessionError> {
        self.publish_identity_receipt_at_cut(receipt, None)
    }

    /// Publish against a local proof captured before the comparison was evaluated.
    /// The caller revalidates the authorized peer and pair fingerprint.
    pub fn publish_prepared_identity_receipt(
        &mut self,
        receipt: IdentityReceipt,
        proof: &IdentityPublicationProof,
    ) -> Result<IdentityReceipt, OwnerSessionError> {
        self.publish_identity_receipt_at_cut(receipt, Some(proof))
    }

    fn publish_identity_receipt_at_cut(
        &mut self,
        receipt: IdentityReceipt,
        proof: Option<&IdentityPublicationProof>,
    ) -> Result<IdentityReceipt, OwnerSessionError> {
        self.ensure_write_ready()?;
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(invalid("owner writer requires reopen"));
        }
        validate_structure(&receipt, self.session.owner_id)?;
        let projection = LocalProjection {
            memories: &self.session.common.memories,
            memory_cut: self.session.common.memories.memory_version(),
            observed_bodies: None,
            pruned_bodies: &BTreeSet::new(),
            perception: &self.session.common.perception,
            perception_cut: self.session.common.perception.version(),
        };
        match proof {
            Some(proof) => proof.check(&self.session, &receipt)?,
            None => projection.check(&receipt)?,
        }
        let history = &mut self.session.identity_history;
        if let Some(head) = history.get(&receipt.pair, None)
            && same_inputs(head, &receipt)
        {
            return Ok(head.clone());
        }
        history.check_successor(&receipt)?;
        let payload = encode(&receipt)?;
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
        history.push(StoredReceipt {
            publication_version,
            memory_cut: projection.memory_cut,
            perception_cut: projection.perception_cut,
            receipt: receipt.clone(),
        });
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        Ok(receipt)
    }
}

pub(super) struct LocalProjection<'a> {
    pub(super) memories: &'a MemoryStore,
    pub(super) memory_cut: u64,
    pub(super) observed_bodies: Option<&'a BTreeSet<MemoryBodyId>>,
    pub(super) pruned_bodies: &'a BTreeSet<MemoryBodyId>,
    pub(super) perception: &'a PerceptionStore,
    pub(super) perception_cut: u64,
}

impl LocalProjection<'_> {
    fn active_entity(&self, id: EntityId, revision: u64) -> bool {
        self.perception
            .entity_at_cut(id, self.perception_cut)
            .is_some_and(|entity| {
                entity.revision == revision && entity.kind != PRINCIPAL_ENTITY_KIND
            })
    }

    fn check(&self, receipt: &IdentityReceipt) -> Result<(), OwnerSessionError> {
        if !self.active_entity(receipt.pair.local_entity, receipt.left.endpoint_revision) {
            return Err(invalid(
                "local Entity revision outside the publication prefix",
            ));
        }
        let owner = owner_hex(&receipt.pair.local_owner);
        for reference in &receipt.left.relationships {
            let relationship = self
                .perception
                .relationship_at_cut(reference.id, self.perception_cut)
                .filter(|value| value.revision == reference.revision)
                .ok_or_else(|| {
                    invalid("local Relationship revision outside the publication prefix")
                })?;
            if relationship.participants.iter().any(|participant| {
                participant.entity.owner_id != owner
                    || reference
                        .participants
                        .binary_search_by_key(&participant.entity.entity_id, |entry| entry.id)
                        .is_err()
            }) || reference.participants.iter().any(|participant| {
                !self.active_entity(participant.id, participant.revision)
                    || !relationship
                        .participants
                        .iter()
                        .any(|entry| entry.entity.entity_id == participant.id)
            }) || relationship
                .evidence
                .iter()
                .any(|memory| memory.owner_id != owner)
            {
                return Err(invalid("local Relationship scope or participant revision"));
            }
            for memory in &reference.memories {
                let record = self
                    .memories
                    .record_at_cut(MemoryId(memory.id), self.memory_cut)
                    .filter(|record| {
                        record.revision == memory.revision
                            && record.body_id.0 == memory.body_id
                            && !record.archived
                            && record.superseded_by.is_none()
                    })
                    .ok_or_else(|| {
                        invalid("local Memory revision outside the publication prefix")
                    })?;
                if !relationship
                    .evidence
                    .iter()
                    .any(|item| item.memory_id == memory.id)
                {
                    return Err(invalid("local Relationship Memory reference"));
                }
                if let Some(span) = memory.span {
                    if !has_direct_source(record) {
                        return Err(invalid("local witness source provenance"));
                    }
                    self.check_span(relationship, reference, memory, span)?;
                }
            }
        }
        Ok(())
    }

    fn check_span(
        &self,
        relationship: &Relationship,
        reference: &IdentityRelationshipRef,
        memory: &IdentityMemoryRef,
        span: IdentitySpan,
    ) -> Result<(), OwnerSessionError> {
        let body_id = MemoryBodyId(memory.body_id);
        let pruned = self.pruned_bodies.contains(&body_id);
        if !pruned
            && self
                .observed_bodies
                .is_some_and(|bodies| !bodies.contains(&body_id))
        {
            return Err(invalid("local witness body outside the publication prefix"));
        }
        let (_, body) = self
            .memories
            .revision_projection(MemoryId(memory.id), Some(memory.revision))?;
        let retained_quote = relationship.summary.as_str();
        let prune = self
            .perception
            .relationship_description_prune(relationship.id, relationship.revision);
        let quote = match (body.as_deref(), prune) {
            (Some(body), Some(proof)) => {
                let quote = text_field(body, span.field)
                    .get(span.start_byte as usize..span.end_byte as usize)
                    .ok_or_else(|| invalid("local witness span"))?;
                if <[u8; 32]>::from(Sha256::digest(quote.as_bytes())) != proof.summary_digest {
                    return Err(invalid(
                        "local witness span differs from the retained quote digest",
                    ));
                }
                quote
            }
            (None, Some(_)) if pruned => return Ok(()),
            (_, _) => retained_quote,
        };
        if quote.trim().is_empty()
            || quote.len() != (span.end_byte - span.start_byte) as usize
            || reference.participants.iter().any(|participant| {
                self.perception
                    .entity_at_cut(participant.id, self.perception_cut)
                    .is_none_or(|entity| {
                        !quote_names_entity(quote, &entity.canonical_name, &entity.aliases)
                    })
            })
        {
            return Err(invalid("local witness quote or participant names"));
        }
        match body.as_deref() {
            Some(body) if exact_quote_span(body, quote).ok() == Some(span) => Ok(()),
            None if pruned => Ok(()),
            _ => Err(invalid("local witness span or missing body")),
        }
    }
}

fn same_inputs(left: &IdentityReceipt, right: &IdentityReceipt) -> bool {
    left.fingerprint == right.fingerprint
        && left.policy == right.policy
        && left.route == right.route
}

fn invalid(message: &str) -> OwnerSessionError {
    OwnerSessionError::Record(format!("invalid identity receipt: {message}"))
}

fn encode(receipt: &IdentityReceipt) -> Result<Vec<u8>, OwnerSessionError> {
    let mut body = MARKER.to_vec();
    body.extend(rmp_serde::to_vec_named(receipt).map_err(|_| invalid("encoding"))?);
    if body.len() > MAX_RECEIPT_BYTES {
        return Err(invalid("record size"));
    }
    Ok(body)
}

fn decode(body: &[u8]) -> Result<IdentityReceipt, OwnerSessionError> {
    if body.len() > MAX_RECEIPT_BYTES || !body.starts_with(MARKER) {
        return Err(invalid("record type or size"));
    }
    let receipt = rmp_serde::from_slice(&body[MARKER.len()..]).map_err(|_| invalid("encoding"))?;
    if encode(&receipt)? != body {
        return Err(invalid("noncanonical encoding"));
    }
    Ok(receipt)
}

fn validate_structure(receipt: &IdentityReceipt, owner: [u8; 16]) -> Result<(), OwnerSessionError> {
    let deterministic = receipt.route == "grounding-v1";
    let opaque_route = receipt.route.len() == 64
        && receipt
            .route
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
    if receipt.pair.local_owner != owner
        || receipt.pair.peer_owner == owner
        || receipt.pair.peer_owner == [0; 16]
        || receipt.revision == 0
        || receipt.policy == 0
        || (!deterministic && !opaque_route)
    {
        return Err(invalid("owner, revision or policy"));
    }
    validate_side(&receipt.left, receipt.pair.local_entity)?;
    validate_side(&receipt.right, receipt.pair.peer_entity)?;
    let limited = receipt.left.limit.is_some() || receipt.right.limit.is_some();
    let supported_reason = match receipt.outcome {
        IdentityOutcome::SameIdentity => receipt.reason == IdentityReason::Corroborated,
        IdentityOutcome::RelatedDistinct => receipt.reason == IdentityReason::Distinct,
        IdentityOutcome::Unresolved => matches!(
            receipt.reason,
            IdentityReason::InsufficientEvidence
                | IdentityReason::Ambiguity
                | IdentityReason::Conflict
                | IdentityReason::EvidenceLimit
        ),
    };
    let affirmative = receipt.outcome != IdentityOutcome::Unresolved;
    if !supported_reason
        || limited != (receipt.reason == IdentityReason::EvidenceLimit)
        || (affirmative && deterministic)
        || (limited && !receipt.citations.is_empty())
        || receipt.citations.len() > MAX_CITATIONS
        || receipt.citations.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid("outcome or citations"));
    }
    let mut sides = BTreeSet::new();
    for citation in &receipt.citations {
        let side = match citation.side {
            IdentitySide::Left => &receipt.left,
            IdentitySide::Right => &receipt.right,
        };
        let memory = side
            .relationships
            .get(usize::from(citation.relationship))
            .and_then(|value| value.memories.get(usize::from(citation.memory)));
        if memory.is_none_or(|value| value.span.is_none()) {
            return Err(invalid("citation outside grounded evidence"));
        }
        sides.insert(citation.side);
    }
    if affirmative && sides.len() != 2 {
        return Err(invalid("missing two-sided citations"));
    }
    Ok(())
}

fn validate_side(side: &IdentityEvidenceRef, endpoint: EntityId) -> Result<(), OwnerSessionError> {
    if side.endpoint_revision == 0
        || side.relationships.len() > MAX_IDENTITY_RELATIONSHIPS_PER_SIDE
        || side.limit == Some(IdentityLimit::Relationships)
            && side.relationships.len() != MAX_IDENTITY_RELATIONSHIPS_PER_SIDE
        || side
            .relationships
            .windows(2)
            .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(invalid("evidence frontier"));
    }
    let mut participants = BTreeMap::new();
    let mut memories = BTreeMap::new();
    for relationship in &side.relationships {
        if relationship.revision == 0
            || !(2..=MAX_RELATIONSHIP_PARTICIPANTS).contains(&relationship.participants.len())
            || relationship
                .participants
                .windows(2)
                .any(|pair| pair[0].id >= pair[1].id)
            || relationship
                .participants
                .iter()
                .any(|value| value.revision == 0)
            || !relationship
                .participants
                .iter()
                .any(|entry| entry.id == endpoint && entry.revision == side.endpoint_revision)
            || !(1..=MAX_RELATIONSHIP_EVIDENCE).contains(&relationship.memories.len())
            || relationship
                .memories
                .windows(2)
                .any(|pair| pair[0].id >= pair[1].id)
            || relationship.memories.iter().any(|memory| {
                memory.revision == 0
                    || memory
                        .span
                        .is_some_and(|span| span.start_byte >= span.end_byte)
            })
        {
            return Err(invalid("evidence references"));
        }
        for participant in &relationship.participants {
            if participants
                .insert(participant.id, participant.revision)
                .is_some_and(|revision| revision != participant.revision)
            {
                return Err(invalid("inconsistent participant references"));
            }
        }
        for memory in &relationship.memories {
            let revision = (memory.revision, memory.body_id);
            if memories
                .insert(memory.id, revision)
                .is_some_and(|previous| previous != revision)
            {
                return Err(invalid("inconsistent Memory references"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests;
