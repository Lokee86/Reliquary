use super::*;
use crate::PerceptionSourceCut;

pub(crate) const PHY_MARKER: &[u8; 8] = b"PHYOBS01";
const MAX_PROGRESS_BYTES: usize = 32 * 1024;

/// Owner-local frontiers used at the native PHY Observation publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhyObservationCut {
    pub audit: PhyAuditCut,
    pub entity: u64,
    pub retained_resolution: Option<u64>,
    pub observation: u64,
}

impl From<PhyObservationCut> for PerceptionSourceCut {
    fn from(cut: PhyObservationCut) -> Self {
        Self {
            owner_id: cut.audit.owner_id,
            history_generation: cut.audit.history_generation,
            memory: cut.audit.memory,
            graph: cut.audit.graph,
            entity: cut.entity,
            observation: cut.observation,
            source_removal: 0,
            freshness: 0,
            settlements: cut.audit.settlements,
            rel_turn: 0,
            freshness_event: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhyObservationBatch {
    pub cut: PhyObservationCut,
    pub batch: ObservationBatch,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct PhyObservationRecord {
    cut: PhyObservationCut,
    batch: ObservationBatch,
    lineages: Vec<Vec<ObservationMemoryLineage>>,
    source_availability: [u8; 32],
    claim_vectors: Option<ObservationClaimVectors>,
}

#[derive(serde::Serialize)]
enum SourceAvailability {
    RetainedImport,
    Dream(u64),
    Unavailable,
}

fn availability_proof(
    prefix: ObservationPrefix<'_>,
    store: &ObservationStore,
    batch: &ObservationBatch,
    vectors: Option<&ObservationClaimVectors>,
) -> Result<[u8; 32], OwnerSessionError> {
    let ObservationAuthority::Phy(settlements) = prefix.source_eligibility else {
        return Err(invalid("PHY source proof is missing"));
    };
    let citations = batch
        .operations
        .iter()
        .flat_map(ObservationOperation::citations)
        .chain(
            vectors
                .into_iter()
                .flat_map(|vectors| vectors.rows.iter().map(|row| &row.citation)),
        );
    let mut effective_memories = BTreeSet::new();
    for citation in citations {
        match citation.source {
            ObservationSource::Memory(memory_id) => {
                effective_memories.insert(ObservationMemoryLineage {
                    memory_id,
                    revision: citation.revision,
                    body_id: citation.body_id,
                });
            }
            ObservationSource::Observation(id) => {
                if let Some(row) = store.revision(id, citation.revision) {
                    effective_memories.extend(row.lineage.iter().copied());
                }
            }
        }
    }
    let proof: Vec<_> = effective_memories
        .into_iter()
        .map(|source| {
            let id = MemoryId(source.memory_id);
            let available = prefix
                .memories
                .revision_record(id, Some(source.revision))
                .is_ok_and(|record| {
                    record.memory_version <= prefix.memory_cut
                        && record.body_id.0 == source.body_id
                        && prefix.allows_memory_source(record)
                        && prefix.body_in_prefix(record.body_id)
                        && prefix.memories.contains_body_payload(record.body_id)
                });
            // A review can retain a previous exact citation after source
            // withdrawal. Its availability differs from newly grounded evidence.
            let authority = if !available {
                SourceAvailability::Unavailable
            } else if let Some(position) = settlements.position(id) {
                SourceAvailability::Dream(position)
            } else if prefix
                .memories
                .has_retained_phy_import_authority_at(id, prefix.memory_cut)
            {
                SourceAvailability::RetainedImport
            } else {
                SourceAvailability::Unavailable
            };
            (source, authority)
        })
        .collect();
    let bytes =
        rmp_serde::to_vec_named(&proof).map_err(|_| invalid("PHY evidence proof encoding"))?;
    let mut hash = Sha256::new();
    hash.update(b"reliquary-phy-observation-source-availability-v1");
    hash.update(bytes);
    Ok(hash.finalize().into())
}

impl PhyObservationRecord {
    fn encode(&self) -> Result<Vec<u8>, OwnerSessionError> {
        let mut body = PHY_MARKER.to_vec();
        body.extend(rmp_serde::to_vec_named(self).map_err(|_| invalid("PHY encoding"))?);
        if body.len() > MAX_RECORD_BYTES {
            return Err(invalid("PHY record size"));
        }
        Ok(body)
    }

    fn decode(frame: &[u8]) -> Result<(u64, Self), OwnerSessionError> {
        let envelope = reliquary_container::decode_publication(frame)?
            .filter(|entry| entry.transaction_time_ns.is_none() && !entry.bound_reservation)
            .ok_or_else(|| invalid("expected an untimed PHY publication envelope"))?;
        let body = envelope.body;
        if !body.starts_with(PHY_MARKER) || body.len() > MAX_RECORD_BYTES {
            return Err(invalid("unsupported PHY Observation publication"));
        }
        let record: Self = rmp_serde::from_slice(&body[PHY_MARKER.len()..])
            .map_err(|_| invalid("PHY encoding"))?;
        if record.encode()? != body {
            return Err(invalid("noncanonical PHY encoding"));
        }
        Ok((envelope.version, record))
    }

    fn checked(
        self,
        store: &ObservationStore,
        prefix: ObservationPrefix<'_>,
        cut: PhyObservationCut,
        version: u64,
        active_profile: impl FnOnce(CompatibilityProfileId) -> bool,
    ) -> Result<(ObservationRecord, ObservationBatchReceipt), OwnerSessionError> {
        if self.cut != cut {
            return Err(invalid("PHY publication differs from its physical cut"));
        }
        if self
            .batch
            .operations
            .iter()
            .any(|operation| match operation {
                ObservationOperation::SemanticAward { .. } => true,
                ObservationOperation::Successor {
                    credited_activity, ..
                } => credited_activity.is_some(),
                _ => false,
            })
        {
            return Err(invalid("REL activity on a PHY Observation"));
        }
        if self
            .batch
            .progress
            .as_ref()
            .is_some_and(|progress| progress.bytes.len() > MAX_PROGRESS_BYTES)
        {
            return Err(invalid("PHY Observation progress exceeds its bound"));
        }
        if self
            .claim_vectors
            .as_ref()
            .is_some_and(|vectors| !active_profile(CompatibilityProfileId(vectors.profile)))
        {
            return Err(invalid(
                "PHY claim vector profile is inactive at publication",
            ));
        }
        if self.source_availability
            != availability_proof(prefix, store, &self.batch, self.claim_vectors.as_ref())?
        {
            return Err(invalid("PHY source availability proof differs"));
        }
        let record = ObservationRecord {
            batch: self.batch,
            memory_cut: cut.audit.memory,
            perception_cut: cut.entity,
            archive_cut: 0,
            rel_turn: 0,
            lineages: self.lineages,
            claim_vectors: self.claim_vectors,
        };
        let receipt = prefix.check_record(store, &record, version)?;
        Ok((record, receipt))
    }
}

/// The semantic store is shared with REL. Only the native prefix proof differs.
#[derive(Default)]
pub(in crate::session) struct PhyObservationHistory {
    pub store: ObservationStore,
    pub proofs: BTreeMap<u64, PhyObservationPublicationProof>,
}

#[derive(serde::Serialize)]
pub(in crate::session) struct PhyObservationPublicationProof {
    pub cut: PhyObservationCut,
    source_availability: [u8; 32],
}

impl PhyObservationHistory {
    pub(in crate::session) fn admit(
        &mut self,
        frame: &[u8],
        prefix: ObservationPrefix<'_>,
        cut: PhyObservationCut,
        active_profiles: &BTreeSet<[u8; 32]>,
    ) -> Result<(), OwnerSessionError> {
        let (version, native) = PhyObservationRecord::decode(frame)?;
        let source_availability = native.source_availability;
        let (record, receipt) = native.checked(&self.store, prefix, cut, version, |profile| {
            active_profiles.contains(&profile.0)
        })?;
        self.apply(cut, source_availability, record, receipt);
        Ok(())
    }

    fn apply(
        &mut self,
        cut: PhyObservationCut,
        source_availability: [u8; 32],
        record: ObservationRecord,
        receipt: ObservationBatchReceipt,
    ) {
        self.proofs.insert(
            receipt.publication_version,
            PhyObservationPublicationProof {
                cut,
                source_availability,
            },
        );
        self.store.apply(record, receipt);
    }
}

macro_rules! phy_observation_reads {
    ($session:ty) => {
        impl $session {
            pub fn phy_observation_cut(&self) -> PhyObservationCut {
                PhyObservationCut {
                    audit: self.phy_audit_cut(),
                    entity: self.common.perception.version(),
                    retained_resolution: self.common.perception.retained_resolution_ordinal(),
                    observation: self.observation_store().frontier(),
                }
            }

            pub fn perception_source_cut(&self) -> PerceptionSourceCut {
                let mut cut = PerceptionSourceCut::from(self.phy_observation_cut());
                cut.observation = self.perception_review_version();
                cut
            }

            pub fn perception_review_version(&self) -> u64 {
                self.observation_store().semantic_frontier()
            }

            pub(super) fn observation_store(&self) -> &ObservationStore {
                &self.common.phy_observations.store
            }

            pub(super) fn observation_prefix(&self) -> ObservationPrefix<'_> {
                let cut = self.phy_observation_cut();
                ObservationPrefix {
                    owner: cut.audit.owner_id,
                    generation: cut.audit.history_generation,
                    memories: &self.common.memories,
                    profiles: &self.common.profiles,
                    memory_cut: cut.audit.memory,
                    perception: &self.common.perception,
                    perception_cut: cut.entity,
                    retained_ordinal_cut: cut.retained_resolution,
                    observed_bodies: None,
                    unobserved_suffix_bodies: None,
                    source_eligibility: ObservationAuthority::Phy(PhySettlements::Admitted(
                        &self.common.dreams,
                        cut.audit.settlements,
                    )),
                }
            }
        }
    };
}

phy_observation_reads!(PhySession);
phy_observation_reads!(PhyWriter);

impl PhyWriter {
    pub fn publish_observation_batch(
        &mut self,
        request: PhyObservationBatch,
    ) -> Result<ObservationBatchReceipt, OwnerSessionError> {
        self.publish_phy_observation(request, None)
    }

    pub fn publish_observation_claim_vectors(
        &mut self,
        profile: CompatibilityProfileId,
        mut rows: Vec<ObservationClaimVector>,
    ) -> Result<(), OwnerSessionError> {
        self.check_lifecycle_lease("Observation claim vectors")?;
        let store = self.observation_store();
        for row in &rows {
            if store
                .claim_vector(profile.0, &row.citation)
                .is_some_and(|accepted| accepted != row.vector)
            {
                return Err(invalid(
                    "PHY claim vector retry differs from the accepted vector",
                ));
            }
        }
        let Some((batch, vectors)) = prepare_claim_vectors(
            store,
            self.active_compatibility_profile(profile).is_some(),
            self.owner_id,
            self.history_generation,
            profile,
            &mut rows,
        )?
        else {
            return Ok(());
        };
        let request = PhyObservationBatch {
            cut: self.phy_observation_cut(),
            batch,
        };
        self.publish_phy_observation(request, Some(vectors))?;
        Ok(())
    }

    fn publish_phy_observation(
        &mut self,
        request: PhyObservationBatch,
        claim_vectors: Option<ObservationClaimVectors>,
    ) -> Result<ObservationBatchReceipt, OwnerSessionError> {
        self.check_lifecycle_lease("Observation publication")?;
        let history = &self.common.phy_observations;
        if let Some(receipt) = history.store.lookup_receipt(&request.batch)? {
            if history
                .proofs
                .get(&receipt.publication_version)
                .map(|proof| proof.cut)
                != Some(request.cut)
            {
                return Err(invalid("PHY receipt retry differs from the accepted cut"));
            }
            return Ok(receipt.clone());
        }
        let cut = self.phy_observation_cut();
        if request.cut != cut {
            return Err(OwnerSessionError::ManagementStale);
        }
        let prefix = self.observation_prefix();
        let version = self
            .container
            .latest_version()
            .checked_add(1)
            .ok_or_else(|| invalid("publication version exhausted"))?;
        let source_availability = availability_proof(
            prefix,
            &history.store,
            &request.batch,
            claim_vectors.as_ref(),
        )?;
        let native = PhyObservationRecord {
            cut,
            source_availability,
            lineages: prefix.lineages(&history.store, &request.batch)?,
            batch: request.batch,
            claim_vectors,
        };
        let payload = native.encode()?;
        let (record, receipt) =
            native.checked(&history.store, prefix, cut, version, |profile| {
                self.active_compatibility_profile(profile).is_some()
            })?;
        let publication = self
            .container
            .append_publication_envelope(&payload, None)
            .and_then(|(published, _)| self.container.sync().map(|()| published));
        match publication {
            Ok(published) if published == version => {
                self.common.phy_observations.apply(
                    cut,
                    source_availability,
                    record,
                    receipt.clone(),
                );
                self.writer_lease_proof = self.container.writer_lease_proof();
                Ok(receipt)
            }
            Ok(_) => {
                self.poisoned = true;
                Err(invalid("unexpected PHY publication version"))
            }
            Err(error) => {
                self.poisoned = true;
                Err(error.into())
            }
        }
    }
}

#[cfg(test)]
mod tests;
