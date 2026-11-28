use super::*;

macro_rules! observation_reads {
    ($session:ty) => {
        impl $session {
            pub fn observation_claim_index_profiles(
                &self,
            ) -> Vec<reliquary_vector_store::CompatibilityProfile> {
                let active = || {
                    self.common
                        .profiles
                        .iter()
                        .filter(|profile| !self.common.profiles.is_retired(profile.id))
                };
                let memory_profiles: Vec<_> = active()
                    .filter(|profile| self.common.memory_vectors.binding_count(profile.id) != 0)
                    .cloned()
                    .collect();
                if memory_profiles.is_empty() {
                    active().cloned().collect()
                } else {
                    memory_profiles
                }
            }

            pub fn observation_claim_profiles(
                &self,
            ) -> impl Iterator<Item = CompatibilityProfileId> + '_ {
                self.observation_store()
                    .claim_vector_profiles()
                    .map(CompatibilityProfileId)
                    .filter(|id| self.active_compatibility_profile(*id).is_some())
            }

            pub fn observation_claim_vector(
                &self,
                profile: CompatibilityProfileId,
                revision: &ObservationRevision,
            ) -> Result<Option<Vec<f32>>, OwnerSessionError> {
                if self.active_compatibility_profile(profile).is_none() {
                    return Err(invalid("claim vector profile is unavailable"));
                }
                let citation = revision.full_citation();
                let Some(vector) = self.observation_store().claim_vector(profile.0, &citation)
                else {
                    return Ok(None);
                };
                Ok(self
                    .observation(revision.id)
                    .filter(|head| {
                        head.standing == ObservationStanding::Current
                            && head.revision.full_citation() == citation
                    })
                    .map(|_| vector.to_vec()))
            }

            pub fn observation_progress(&self) -> Option<&ObservationAuditProgress> {
                self.observation_store().progress()
            }

            pub fn observation_publication_version(&self) -> u64 {
                self.observation_store().frontier()
            }

            pub fn observation(&self, id: ObservationId) -> Option<ObservationInspection<'_>> {
                self.observation_page_for_ids([id]).pop()
            }

            pub fn observations_after(
                &self,
                after: Option<ObservationId>,
                limit: usize,
            ) -> Vec<ObservationInspection<'_>> {
                self.observation_page_for_ids(
                    self.observation_store()
                        .heads_after(after, limit.min(MAX_PAGE)),
                )
            }

            fn observation_page_for_ids(
                &self,
                ids: impl IntoIterator<Item = ObservationId>,
            ) -> Vec<ObservationInspection<'_>> {
                let mut checker =
                    StandingCheck::new(self.observation_prefix(), self.observation_store());
                ids.into_iter()
                    .filter_map(|id| {
                        let revision = self.observation_store().head(id)?;
                        Some(ObservationInspection {
                            revision,
                            standing: checker.standing(id),
                            triggering_evidence: self
                                .observation_store()
                                .pending(id)
                                .map(|(_, evidence)| evidence)
                                .unwrap_or_default(),
                        })
                    })
                    .collect()
            }

            pub fn observation_revision(
                &self,
                id: ObservationId,
                revision: u64,
            ) -> Option<ObservationInspection<'_>> {
                let row = self.observation_store().revision(id, revision)?;
                let head = self.observation_store().head(id)?;
                if head.revision == revision {
                    self.observation(id)
                } else {
                    Some(ObservationInspection {
                        revision: row,
                        standing: ObservationStanding::Superseded,
                        triggering_evidence: Vec::new(),
                    })
                }
            }

            pub fn observation_revisions_after(
                &self,
                id: ObservationId,
                after: u64,
                limit: usize,
            ) -> Vec<ObservationInspection<'_>> {
                self.observation_store()
                    .revision_page(id, after, limit.min(MAX_PAGE))
                    .filter_map(|row| self.observation_revision(id, row.revision))
                    .collect()
            }

            pub fn matching_observation(&self, draft: &ObservationDraft) -> Option<ObservationId> {
                self.observation_store().matching_claim(draft)
            }

            pub fn observation_comparison(
                &self,
                comparison: &ObservationComparison,
            ) -> Option<ObservationComparisonOutcome> {
                self.observation_store().comparison(comparison)
            }

            pub fn observation_comparison_at_policy(
                &self,
                comparison: &ObservationComparison,
                policy: u32,
            ) -> Option<ObservationComparisonOutcome> {
                self.observation_store()
                    .comparison_at_policy(comparison, policy)
            }

            pub fn observation_pending_after(
                &self,
                after: Option<ObservationId>,
                limit: usize,
            ) -> Vec<ObservationInspection<'_>> {
                self.observation_page_for_ids(
                    self.observation_store()
                        .explicitly_pending_after(after, limit.min(MAX_PAGE)),
                )
            }

            pub fn observation_dependents_after(
                &self,
                source: ObservationSource,
                after: Option<ObservationId>,
                limit: usize,
            ) -> Vec<ObservationId> {
                self.observation_store()
                    .source_dependents_after(source, after, limit.min(MAX_PAGE))
                    .collect()
            }

            pub fn observation_participant_dependents_after(
                &self,
                entity: EntityId,
                after: Option<ObservationId>,
                limit: usize,
            ) -> Vec<ObservationId> {
                self.observation_store()
                    .participant_dependents_after(entity, after, limit.min(MAX_PAGE))
                    .collect()
            }

            pub fn observation_dependents(&self, source: ObservationSource) -> Vec<ObservationId> {
                self.observation_store()
                    .source_dependents(source)
                    .take(MAX_PAGE)
                    .collect()
            }

            pub fn observation_participant_dependents(
                &self,
                entity: EntityId,
            ) -> Vec<ObservationId> {
                self.observation_store()
                    .participant_dependents(entity)
                    .take(MAX_PAGE)
                    .collect()
            }

            pub fn observation_citation_text(
                &self,
                cite: &ObservationCitation,
            ) -> Result<String, OwnerSessionError> {
                let prefix = self.observation_prefix();
                let mut checker = StandingCheck::new(prefix, self.observation_store());
                checker.current_lineage(cite)?;
                self.historical_observation_citation_text(cite)
            }

            pub fn historical_observation_citation_text(
                &self,
                cite: &ObservationCitation,
            ) -> Result<String, OwnerSessionError> {
                let prefix = self.observation_prefix();
                prefix.check_owner(cite)?;
                match cite.source {
                    ObservationSource::Memory(id) => prefix.memory_text(cite, id),
                    ObservationSource::Observation(id) => self
                        .observation_store()
                        .revision(id, cite.revision)
                        .filter(|row| self.observation_source_eligible(row))
                        .ok_or_else(|| invalid("unknown Observation revision"))
                        .and_then(|revision| observation_text(revision, cite))
                        .map(str::to_owned),
                }
            }

            /// Historical revisions retain their citation coordinates. An exact
            /// revision is eligible to disclose only while all its pinned source
            /// Memories (including upstream Observation lineage) remain eligible.
            pub fn observation_revision_source_eligible(
                &self,
                id: ObservationId,
                revision: u64,
            ) -> bool {
                self.observation_store()
                    .revision(id, revision)
                    .is_some_and(|row| self.observation_source_eligible(row))
            }

            fn observation_source_eligible(&self, row: &ObservationRevision) -> bool {
                let prefix = self.observation_prefix();
                if let ObservationAuthority::Rel(eligibility) = prefix.source_eligibility
                    && !eligibility.has_removals()
                {
                    return true;
                }
                if row.lineage.iter().any(|source| {
                    prefix
                        .memories
                        .revision_record(MemoryId(source.memory_id), Some(source.revision))
                        .map_or(true, |record| {
                            !prefix.allows_memory_source(record)
                                || (matches!(
                                    prefix.source_eligibility,
                                    ObservationAuthority::Phy(_)
                                ) && !prefix.memories.contains_body_payload(record.body_id))
                        })
                }) {
                    return false;
                }
                let mut remaining = vec![row];
                let mut visited = BTreeSet::new();
                while let Some(row) = remaining.pop() {
                    if !visited.insert((row.id, row.revision)) {
                        continue;
                    }
                    if visited.len() > MAX_STANDING_CHECK
                        || row.draft.profile.participants().iter().any(|participant| {
                            self.common
                                .perception
                                .entity_revision(participant.entity_id, participant.revision)
                                .is_none_or(|entity| !prefix.allows_entity_source(entity))
                        })
                    {
                        return false;
                    }
                    for cite in &row.draft.citations {
                        if let ObservationSource::Observation(id) = cite.source {
                            let Some(parent) = self.observation_store().revision(id, cite.revision)
                            else {
                                return false;
                            };
                            remaining.push(parent);
                        }
                    }
                }
                true
            }
        }
    };
}

observation_reads!(RelSession);
observation_reads!(PhySession);
observation_reads!(PhyWriter);
