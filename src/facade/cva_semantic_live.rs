//! Intrinsic live semantic Freshness and bounded rediscovery of accepted semantic work.
//!
//! F8 is part of normal REL Freshness semantics. It has no enable/disable control.
//! Historical checkpoints and legacy pre-release activation records contribute only
//! a lower-bound watermark so already-accounted fragments are never credited again.
use crate::freshness_storage::SemanticActivation;
use crate::{Cva, CvaError, VectorGenerationId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SemanticFreshnessReport {
    pub completed_fragments: usize,
    pub pending_fragments: usize,
    pub reinforced_memories: usize,
}

impl Cva {
    fn semantic_freshness_floor(&self) -> Result<([u8; 16], u64), CvaError> {
        let owner = self.owner_uuid().ok_or(CvaError::InvalidContainerIdentity(
            "semantic Freshness requires REL UUID",
        ))?;
        let mut floor = self
            .freshness
            .latest_import_settlement()
            .map_or(0, |settlement| settlement.settlement_cut);
        if let Some(activation) = self.freshness.semantic_activation() {
            self.validate_semantic_activation(activation)?;
            floor = floor.max(activation.accepted_turn);
        }
        Ok((owner, floor))
    }

    /// Number of retained eligible fragments after the accepted semantic floor
    /// without a durable completion.
    pub fn semantic_freshness_pending_fragments(&self) -> Result<usize, CvaError> {
        Ok(self.pending_semantic_fragments()?.len())
    }

    fn pending_semantic_fragments(&self) -> Result<Vec<(u64, crate::FragmentId)>, CvaError> {
        crate::semantic_archive_batch::pending_fragments(
            &crate::owner_lifecycle::semantic_archive_read::SemanticArchiveRead::new(
                crate::owner_lifecycle::semantic_source::SemanticSourceReadParts {
                    container: &self.container,
                    archive: &self.archive,
                    freshness: &self.freshness,
                    generations: &self.vector_generations,
                    compatibility_profiles: &self.compatibility_profiles,
                    archive_vectors: &self.archive_vectors,
                    packed_vectors: &self.packed_vectors,
                },
            ),
        )
    }

    /// Process at most `limit` uncompleted retained fragments against one generation.
    /// F8 is intrinsic; this is a scheduling surface, not an activation control.
    pub fn process_semantic_freshness(
        &mut self,
        generation: VectorGenerationId,
        limit: usize,
    ) -> Result<SemanticFreshnessReport, CvaError> {
        let mut report = SemanticFreshnessReport::default();
        let selected = self.vector_generation(generation)?;
        if self.current_vector_generation(selected.compatibility_profile_id) != Some(selected) {
            return Err(CvaError::Freshness(
                "semantic Freshness requires complete current Archive coverage".into(),
            ));
        }
        for (_, fragment) in self.pending_semantic_fragments()?.into_iter().take(limit) {
            let source = match self.prepare_semantic_fragment(fragment, generation)? {
                crate::cva_semantic_source::SemanticSourceReadiness::Pending => {
                    report.pending_fragments += 1;
                    continue;
                }
                crate::cva_semantic_source::SemanticSourceReadiness::Ready(source) => source,
            };
            report.reinforced_memories +=
                self.apply_prepared_semantic_reinforcement(source, &Default::default())?;
            report.completed_fragments += 1;
        }
        Ok(report)
    }

    /// Validate a legacy pre-release semantic activation record. Current code no
    /// longer creates or changes these records; they survive only as a prospective
    /// watermark on old development files.
    pub(crate) fn validate_semantic_activation(
        &self,
        activation: &SemanticActivation,
    ) -> Result<(), CvaError> {
        let at = match &activation.boundary {
            None => Some(0),
            Some((conversation, node)) => self.activity_position_for_turn(conversation, node),
        };
        if self.owner_uuid() != Some(activation.owner_uuid) || at != Some(activation.accepted_turn)
        {
            return Err(CvaError::Freshness(
                "semantic activation boundary is unavailable or conflicting".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
impl Cva {
    pub(crate) fn prepare_semantic_archive_batch(
        &mut self,
        profile_id: crate::CompatibilityProfileId,
    ) -> Result<Option<crate::semantic_archive_batch::SemanticArchiveBatch>, CvaError> {
        if let Some(generation) = self.current_vector_generation(profile_id) {
            self.process_semantic_freshness(generation.id, 64)?;
        }
        let read = crate::owner_lifecycle::semantic_archive_read::SemanticArchiveRead::new(
            crate::owner_lifecycle::semantic_source::SemanticSourceReadParts {
                container: &self.container,
                archive: &self.archive,
                freshness: &self.freshness,
                generations: &self.vector_generations,
                compatibility_profiles: &self.compatibility_profiles,
                archive_vectors: &self.archive_vectors,
                packed_vectors: &self.packed_vectors,
            },
        );
        crate::semantic_archive_batch::prepare_batch(
            &read,
            profile_id,
            self.current_vector_generation(profile_id),
        )
    }
    pub(crate) fn commit_semantic_archive_batch(
        &mut self,
        batch: crate::semantic_archive_batch::SemanticArchiveBatch,
        vectors: Vec<Vec<f32>>,
    ) -> Result<(), CvaError> {
        let current = self.current_vector_generation(batch.profile.id);
        if let Some(plan) = crate::semantic_archive_batch::pack_batch(batch, vectors, current)? {
            let generation = crate::owner_lifecycle::vector_publication::publish_semantic_archive(
                &mut self.owner.container,
                &self.owner.archive,
                &self.owner.compatibility_profiles,
                &mut self.owner.packed_vectors,
                &mut self.owner.archive_vectors,
                &mut self.owner.vector_generations,
                plan,
            )?;
            if self.owner_uuid().is_some() {
                self.process_semantic_freshness(generation.id, 64)
                    .map_err(|e| crate::VectorGenerationError::Freshness(e.to_string()))?;
            }
        }
        Ok(())
    }
}
