use super::*;
use reliquary_ego_store::{
    EgoAnchorPriority, EgoCitation, EgoCompletion, EgoIdentitySelection, EgoPhyPublication,
    EgoPhySummary, EgoRelPublication, EgoRelSummary, EgoTextField,
};
use reliquary_memory_core::{MemoryBody, MemoryRecord};
use reliquary_perception_store::{
    MemoryTextField,
    observation::{ObservationCitation, ObservationSource},
};

macro_rules! private_ego_reads {
    ($session:ty) => {
        impl $session {
            pub fn ego_identities(&self) -> impl Iterator<Item = &EgoIdentity> {
                self.common.ego.identities()
            }

            pub fn ego_identity_revision(
                &self,
                id: EgoIdentityId,
                revision: u64,
            ) -> Option<&EgoIdentity> {
                self.common.ego.identity_revision(id, revision)
            }

            pub fn ego_identity_selection(&self) -> Option<EgoIdentitySelection> {
                self.common.ego.identity_selection()
            }

            pub fn ego_active_identity(&self) -> Option<&EgoIdentity> {
                self.common.ego.active_identity()
            }

            pub fn ego_rel_summary(&self, owner_id: [u8; 16]) -> Option<&EgoRelSummary> {
                self.common.ego.rel_summary(owner_id)
            }

            pub fn ego_rel_summaries<'a>(
                &'a self,
                owner_ids: &'a [[u8; 16]],
            ) -> impl Iterator<Item = ([u8; 16], &'a EgoRelSummary)> + 'a {
                owner_ids
                    .iter()
                    .filter_map(|id| Some((*id, self.ego_rel_summary(*id)?)))
            }

            pub fn ego_rel_summary_revision(
                &self,
                owner_id: [u8; 16],
                revision: u64,
            ) -> Result<Option<EgoRelSummary>, OwnerSessionError> {
                Ok(self.common.ego.rel_summary_revision(owner_id, revision)?)
            }

            pub fn ego_phy_summary(&self) -> Option<&EgoPhySummary> {
                self.common.ego.phy_summary()
            }

            pub fn ego_phy_summary_revision(&self, revision: u64) -> Option<&EgoPhySummary> {
                self.common.ego.phy_summary_revision(revision)
            }

            pub fn ego_completion(&self, id: [u8; 32]) -> Option<&EgoCompletion> {
                self.common.ego.completion(id)
            }

            pub fn ego_completion_for_batch(&self, batch_id: [u8; 32]) -> Option<&EgoCompletion> {
                self.common.ego.completion_for_batch(batch_id)
            }

            pub fn ego_citation_current(&self, citation: &EgoCitation) -> bool {
                self.ego_citation_text(citation).is_ok()
            }

            pub fn ego_citation_text(
                &self,
                citation: &EgoCitation,
            ) -> Result<String, OwnerSessionError> {
                if citation.owner_id != self.owner_id
                    || citation.history_generation != self.history_generation
                {
                    return Err(OwnerSessionError::ManagementEvidence);
                }
                let (record, body) = self
                    .phy_audit_evidence(self.phy_audit_cut(), MemoryId(citation.memory_id))?
                    .ok_or(OwnerSessionError::ManagementEvidence)?;
                citation_excerpt(citation, &record, &body)
                    .map(str::to_owned)
                    .ok_or(OwnerSessionError::ManagementEvidence)
            }
        }
    };
}

private_ego_reads!(PhySession);
private_ego_reads!(PhyWriter);

impl RelSession {
    pub fn ego_citation_current(&self, citation: &EgoCitation) -> bool {
        self.ego_citation_text(citation).is_ok()
    }

    pub fn ego_citation_text(&self, citation: &EgoCitation) -> Result<String, OwnerSessionError> {
        self.observation_citation_text(&observation_citation(citation))
    }

    pub fn historical_ego_citation_text(
        &self,
        citation: &EgoCitation,
    ) -> Result<String, OwnerSessionError> {
        self.historical_observation_citation_text(&observation_citation(citation))
    }

    /// Project the supporting Memory's current score without reinforcing it.
    pub fn ego_citation_score(
        &self,
        citation: &EgoCitation,
    ) -> Result<Option<i16>, OwnerSessionError> {
        if !self.ego_citation_current(citation) {
            return Ok(None);
        }
        let id = FreshnessMemoryId(citation.memory_id);
        let current_turn = self.rel_turn_count();
        let records = self
            .freshness
            .records_for_memories_at(&self.freshness.cut_through_turn(current_turn), [id])
            .map_err(OwnerSessionError::Record)?;
        records
            .get(&id)
            .map(|record| {
                record
                    .score_at_with_policy(current_turn, &self.freshness.policy())
                    .map_err(|error| OwnerSessionError::Record(error.to_string()))
            })
            .transpose()
    }

    pub fn ego_citation_source_time(&self, citation: &EgoCitation) -> Option<i64> {
        if !self.ego_citation_current(citation) {
            return None;
        }
        let record = self
            .common
            .memories
            .revision_record(MemoryId(citation.memory_id), None)
            .ok()?;
        Some(record.source_time_ns.unwrap_or(record.created_at_ns))
    }
}

fn observation_citation(citation: &EgoCitation) -> ObservationCitation {
    ObservationCitation {
        owner_id: citation.owner_id,
        history_generation: citation.history_generation,
        source: ObservationSource::Memory(citation.memory_id),
        revision: citation.revision,
        body_id: citation.body_id,
        field: match citation.field {
            EgoTextField::Title => MemoryTextField::Title,
            EgoTextField::Content => MemoryTextField::Content,
        },
        start_byte: citation.start_byte,
        end_byte: citation.end_byte,
    }
}

fn citation_excerpt<'a>(
    citation: &EgoCitation,
    record: &MemoryRecord,
    body: &'a MemoryBody,
) -> Option<&'a str> {
    if record.revision != citation.revision
        || record.body_id.0 != citation.body_id
        || record.archived
        || record.superseded_by.is_some()
        || citation.start_byte >= citation.end_byte
    {
        return None;
    }
    let text = match citation.field {
        EgoTextField::Title => &body.title,
        EgoTextField::Content => &body.content,
    };
    text.get(citation.start_byte as usize..citation.end_byte as usize)
        .filter(|passage| !passage.trim().is_empty())
}

pub(super) fn validate_phy_publication_at_prefix(
    publication: &EgoPhyPublication,
    generation: [u8; 32],
    memories: &MemoryStore,
    memory_head: u64,
    observed_bodies: &BTreeSet<MemoryBodyId>,
) -> Result<(), OwnerSessionError> {
    let cut = publication.completion.cut;
    if cut.history_generation != generation || cut.memory_version > memory_head {
        return Err(OwnerSessionError::Record(
            "PHY summary source cut is unavailable".into(),
        ));
    }
    if let Some(text) = &publication.replacement {
        for citation in text
            .paragraphs
            .iter()
            .flat_map(|paragraph| &paragraph.citations)
        {
            let id = MemoryId(citation.memory_id);
            let at_cut = memories
                .record_at_cut(id, cut.memory_version)
                .is_some_and(|record| record.revision == citation.revision);
            let at_publication = memories
                .record_at_cut(id, memory_head)
                .is_some_and(|record| record.revision == citation.revision);
            let exact = observed_bodies.contains(&MemoryBodyId(citation.body_id))
                && memories
                    .revision_projection(id, Some(citation.revision))
                    .is_ok_and(|(record, body)| {
                        body.is_some_and(|body| citation_excerpt(citation, record, &body).is_some())
                    });
            if !at_cut || !at_publication || !exact {
                return Err(OwnerSessionError::Record(
                    "PHY summary citation is unavailable at its source and publication cuts".into(),
                ));
            }
        }
    }
    Ok(())
}

macro_rules! anchor_writes {
    () => {
        pub fn create_ego_anchor(
            &mut self,
            id: EgoAnchorId,
            priority: EgoAnchorPriority,
            text: String,
        ) -> Result<EgoAnchor, OwnerSessionError> {
            self.write_ego(|ego, container| ego.create_anchor(container, id, priority, text))
        }

        pub fn update_ego_anchor(
            &mut self,
            id: EgoAnchorId,
            expected_revision: u64,
            priority: EgoAnchorPriority,
            text: String,
        ) -> Result<EgoAnchor, OwnerSessionError> {
            self.write_ego(|ego, container| {
                ego.update_anchor(container, id, expected_revision, priority, text)
            })
        }

        pub fn delete_ego_anchor(
            &mut self,
            id: EgoAnchorId,
            expected_revision: u64,
        ) -> Result<(), OwnerSessionError> {
            self.write_ego(|ego, container| ego.delete_anchor(container, id, expected_revision))
        }
    };
}

impl PhyWriter {
    anchor_writes!();

    pub fn create_ego_identity(
        &mut self,
        id: EgoIdentityId,
        name: String,
        text: String,
    ) -> Result<EgoIdentity, OwnerSessionError> {
        self.write_ego(|ego, container| ego.create_identity(container, id, name, text))
    }

    pub fn update_ego_identity(
        &mut self,
        id: EgoIdentityId,
        expected_revision: u64,
        name: String,
        text: String,
    ) -> Result<EgoIdentity, OwnerSessionError> {
        self.write_ego(|ego, container| {
            ego.update_identity(container, id, expected_revision, name, text)
        })
    }

    pub fn delete_ego_identity(
        &mut self,
        id: EgoIdentityId,
        expected_revision: u64,
    ) -> Result<(), OwnerSessionError> {
        self.write_ego(|ego, container| ego.delete_identity(container, id, expected_revision))
    }

    pub fn select_ego_identity(
        &mut self,
        expected_selection_revision: u64,
        target: Option<(EgoIdentityId, u64)>,
    ) -> Result<EgoIdentitySelection, OwnerSessionError> {
        self.write_ego(|ego, container| {
            ego.select_identity(container, expected_selection_revision, target)
        })
    }

    /// Source REL evidence and grants are independently checked at the
    /// publication boundary that binds both mounted owners.
    pub fn publish_ego_rel_summary(
        &mut self,
        publication: EgoRelPublication,
    ) -> Result<u64, OwnerSessionError> {
        self.write_ego(|ego, container| ego.publish_rel_summary(container, publication))
    }

    pub fn publish_ego_phy_summary(
        &mut self,
        publication: EgoPhyPublication,
    ) -> Result<u64, OwnerSessionError> {
        let cut = publication.completion.cut;
        if cut.owner_id != self.owner_id
            || cut.history_generation != self.history_generation
            || cut.memory_version > self.memory_version()
        {
            return Err(OwnerSessionError::ManagementStale);
        }
        if self.ego_completion(publication.completion.id).is_none()
            && let Some(text) = &publication.replacement
            && !text
                .paragraphs
                .iter()
                .flat_map(|paragraph| &paragraph.citations)
                .all(|citation| {
                    self.ego_citation_current(citation)
                        && self
                            .common
                            .memories
                            .record_at_cut(MemoryId(citation.memory_id), cut.memory_version)
                            .is_some_and(|record| record.revision == citation.revision)
                })
        {
            return Err(OwnerSessionError::ManagementEvidence);
        }
        self.write_ego(|ego, container| ego.publish_phy_summary(container, publication))
    }

    fn write_ego<T>(
        &mut self,
        write: impl FnOnce(&mut EgoStore, &mut Container) -> Result<T, EgoError>,
    ) -> Result<T, OwnerSessionError> {
        let lease = self.container.writer_lease_proof();
        if self.poisoned || lease.is_none() || lease != self.writer_lease_proof {
            return Err(OwnerSessionError::ManagementStale);
        }
        match write(&mut self.common.ego, &mut self.container) {
            Ok(value) => {
                self.writer_lease_proof = self.container.writer_lease_proof();
                Ok(value)
            }
            Err(error) => {
                if matches!(error, EgoError::Container(_))
                    || self.container.writer_lease_proof() != lease
                {
                    self.poisoned = true;
                }
                Err(error.into())
            }
        }
    }
}

impl RelWriter {
    anchor_writes!();

    fn write_ego<T>(
        &mut self,
        write: impl FnOnce(&mut EgoStore, &mut Container) -> Result<T, EgoError>,
    ) -> Result<T, OwnerSessionError> {
        self.ensure_write_ready()?;
        let lease = self.container.writer_lease_proof();
        if lease.is_none() || lease != self.session.writer_lease_proof {
            return Err(OwnerSessionError::ManagementStale);
        }
        match write(&mut self.session.common.ego, &mut self.container) {
            Ok(value) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(value)
            }
            Err(error) => {
                if matches!(error, EgoError::Container(_))
                    || self.container.writer_lease_proof() != lease
                {
                    self.session.write_poisoned = true;
                }
                Err(error.into())
            }
        }
    }
}
