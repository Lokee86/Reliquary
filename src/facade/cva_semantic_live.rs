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
        let (owner, floor) = self.semantic_freshness_floor()?;
        let mut pending = Vec::new();
        for fragment in self.fragments() {
            let Some(at) = self.semantic_fragment_position(fragment.id)? else {
                continue;
            };
            if at > floor
                && !self
                    .freshness
                    .contains_event(&crate::freshness_storage::semantic_fragment_event_id(
                        owner,
                        fragment.id,
                    ))
                    .map_err(CvaError::Freshness)?
            {
                pending.push((at, fragment.id));
            }
        }
        pending.sort_by_key(|(at, id)| {
            (
                *at,
                crate::freshness_storage::semantic_fragment_event_id(owner, *id),
            )
        });
        Ok(pending)
    }

    /// Process at most `limit` uncompleted retained fragments against one generation.
    /// F8 is intrinsic; this is a scheduling surface, not an activation control.
    pub fn process_semantic_freshness(
        &mut self,
        generation: VectorGenerationId,
        limit: usize,
    ) -> Result<SemanticFreshnessReport, CvaError> {
        let mut report = SemanticFreshnessReport::default();
        self.vector_generation(generation)?;
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

pub(crate) struct SemanticArchiveBatch {
    profile: crate::CompatibilityProfile,
    archive_version: u64,
    fragment_ids: Vec<crate::FragmentId>,
    pub texts: Vec<String>,
}

impl Cva {
    pub(crate) fn prepare_semantic_archive_batch(
        &mut self,
        profile_id: crate::CompatibilityProfileId,
    ) -> Result<Option<SemanticArchiveBatch>, CvaError> {
        if let Some(generation) = self.current_vector_generation(profile_id) {
            self.process_semantic_freshness(generation.id, 64)?;
        }
        let pending = self.pending_semantic_fragments()?;
        if pending.is_empty() {
            return Ok(None);
        }
        let archive_version = self.archive_version();
        if let Some(generation) = self.current_vector_generation(profile_id) {
            let set = self.archive_vectors(generation.archive_vector_id)?;
            // A low-level generation may cover the Archive cut but omit a row.
            // Retry that missing evidence; a cut watermark alone is insufficient.
            if pending.iter().all(|(_, id)| {
                self.archive
                    .fragments
                    .archive_version(*id)
                    .is_some_and(|version| version <= generation.source_archive_version)
                    && set.fragment_ids.contains(id)
            }) {
                return Ok(None);
            }
        }
        let profile = self
            .compatibility_profiles
            .get(profile_id)
            .cloned()
            .ok_or_else(|| CvaError::Freshness("missing semantic embedding profile".into()))?;
        let fragment_ids: Vec<_> = self
            .fragments()
            .into_iter()
            .map(|fragment| fragment.id)
            .collect();
        let mut texts = Vec::with_capacity(fragment_ids.len());
        for id in &fragment_ids {
            texts.push(self.fragment_text(*id)?);
        }
        Ok(Some(SemanticArchiveBatch {
            profile,
            archive_version,
            fragment_ids,
            texts,
        }))
    }

    pub(crate) fn commit_semantic_archive_batch(
        &mut self,
        batch: SemanticArchiveBatch,
        vectors: Vec<Vec<f32>>,
    ) -> Result<(), CvaError> {
        crate::compatibility_profile_probe::validate_embedding_batch(
            &batch.profile,
            &vectors,
            batch.texts.len(),
        )
        .map_err(|e| CvaError::Freshness(e.to_string()))?;
        // A newer retained generation can win while provider I/O is in flight.
        if self
            .current_vector_generation(batch.profile.id)
            .is_some_and(|generation| generation.source_archive_version > batch.archive_version)
        {
            return Ok(());
        }
        let schema = crate::VectorSchema::new(batch.profile.dimensions, crate::ScalarType::F32)
            .map_err(|e| CvaError::Freshness(e.to_string()))?;
        let bytes = vectors
            .into_iter()
            .flatten()
            .flat_map(f32::to_le_bytes)
            .collect();
        let packed = crate::PackedVectors::from_bytes(schema, bytes)
            .map_err(|e| CvaError::Freshness(e.to_string()))?;
        let packed_id = self.put_packed_vectors(packed)?;
        let archive_id = self.put_archive_vectors(packed_id, batch.fragment_ids)?;
        self.publish_vector_generation(batch.profile.id, archive_id, batch.archive_version)?;
        Ok(())
    }
}
