//! Prospective opt-in and bounded rediscovery of accepted semantic work.
use crate::freshness_storage::SemanticActivation;
use crate::{Cva, CvaError, VectorGenerationId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SemanticFreshnessReport {
    pub completed_fragments: usize,
    pub pending_fragments: usize,
    pub reinforced_memories: usize,
}

impl Cva {
    /// Enable v1 prospectively at today's exact accepted Archive cut.
    /// Repeated enabling is a no-op. Cancellation is permitted only before completion.
    pub fn set_semantic_freshness_enabled(&mut self, enabled: bool) -> Result<bool, CvaError> {
        if enabled == self.freshness.semantic_activation().is_some() {
            return Ok(false);
        }
        let owner = self.owner_uuid().ok_or(CvaError::InvalidContainerIdentity(
            "semantic activation requires REL UUID",
        ))?;
        let activation = if enabled {
            let turn = self.rel_turn_count();
            let boundary = self
                .archive
                .nodes
                .iter()
                .find(|node| {
                    self.activity_position_for_turn(&node.conversation_id, &node.id) == Some(turn)
                })
                .map(|node| (node.conversation_id.clone(), node.id.clone()));
            let policy = crate::freshness::semantic_policy::SemanticPolicy::default();
            Some(SemanticActivation {
                owner_uuid: owner,
                boundary,
                accepted_turn: turn,
                policy_version: policy.version,
                policy_digest: policy
                    .digest()
                    .map_err(|e| CvaError::Freshness(e.to_string()))?,
            })
        } else {
            None
        };
        self.freshness
            .persist_semantic_activation(&mut self.container, owner, activation)
            .map_err(CvaError::Freshness)?;
        Ok(true)
    }

    pub fn semantic_freshness_enabled(&self) -> bool {
        self.freshness.semantic_activation().is_some()
    }

    /// Number of retained eligible fragments without a durable completion.
    pub fn semantic_freshness_pending_fragments(&self) -> Result<usize, CvaError> {
        let Some(a) = self.freshness.semantic_activation() else {
            return Ok(0);
        };
        Ok(self.pending_semantic_fragments(a)?.len())
    }

    fn pending_semantic_fragments(
        &self,
        a: &SemanticActivation,
    ) -> Result<Vec<(u64, crate::FragmentId)>, CvaError> {
        let mut pending = Vec::new();
        for f in self.fragments() {
            let Some(at) = self.semantic_fragment_position(f.id)? else {
                continue;
            };
            if at > a.accepted_turn
                && !self
                    .freshness
                    .contains_event(&crate::freshness_storage::semantic_fragment_event_id(
                        a.owner_uuid,
                        f.id,
                    ))
                    .map_err(CvaError::Freshness)?
            {
                pending.push((at, f.id));
            }
        }
        pending.sort_by_key(|(at, id)| {
            (
                *at,
                crate::freshness_storage::semantic_fragment_event_id(a.owner_uuid, *id),
            )
        });
        Ok(pending)
    }

    /// Process at most limit uncompleted retained fragments against one generation.
    pub fn process_semantic_freshness(
        &mut self,
        generation: VectorGenerationId,
        limit: usize,
    ) -> Result<SemanticFreshnessReport, CvaError> {
        let mut report = SemanticFreshnessReport::default();
        let Some(activation) = self.freshness.semantic_activation().cloned() else {
            return Ok(report);
        };
        self.validate_semantic_activation(&activation)?;
        self.vector_generation(generation)?;
        for (_, fragment) in self
            .pending_semantic_fragments(&activation)?
            .into_iter()
            .take(limit)
        {
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

    pub(crate) fn validate_semantic_activation(
        &self,
        a: &SemanticActivation,
    ) -> Result<(), CvaError> {
        let at = match &a.boundary {
            None => Some(0),
            Some((conversation, node)) => self.activity_position_for_turn(conversation, node),
        };
        if self.owner_uuid() != Some(a.owner_uuid) || at != Some(a.accepted_turn) {
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
        if !self.semantic_freshness_enabled() {
            return Ok(None);
        }
        if let Some(generation) = self.current_vector_generation(profile_id) {
            self.process_semantic_freshness(generation.id, 64)?;
        }
        if self.semantic_freshness_pending_fragments()? == 0 {
            return Ok(None);
        }
        let archive_version = self.archive_version();
        if let Some(generation) = self.current_vector_generation(profile_id) {
            let set = self.archive_vectors(generation.archive_vector_id)?;
            let activation = self
                .freshness
                .semantic_activation()
                .expect("enabled")
                .clone();
            // A low-level generation may cover the Archive cut but omit a row.
            // Retry that missing evidence; a cut watermark alone is insufficient.
            if self
                .pending_semantic_fragments(&activation)?
                .iter()
                .all(|(_, id)| {
                    self.archive
                        .fragments
                        .archive_version(*id)
                        .is_some_and(|v| v <= generation.source_archive_version)
                        && set.fragment_ids.contains(id)
                })
            {
                return Ok(None);
            }
        }
        let profile = self
            .compatibility_profiles
            .get(profile_id)
            .cloned()
            .ok_or_else(|| CvaError::Freshness("missing semantic embedding profile".into()))?;
        let fragment_ids: Vec<_> = self.fragments().into_iter().map(|f| f.id).collect();
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
            .is_some_and(|g| g.source_archive_version > batch.archive_version)
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
