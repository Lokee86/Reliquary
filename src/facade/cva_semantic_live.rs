//! Prospective opt-in and bounded rediscovery of accepted semantic work.
use crate::freshness_storage::SemanticActivation;
use crate::{Cva, CvaError, VectorGenerationId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SemanticFreshnessReport {
    pub completed_turns: usize,
    pub pending_turns: usize,
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

    /// Number of eligible Archive turns without a durable semantic completion.
    pub fn semantic_freshness_pending_turns(&self) -> Result<usize, CvaError> {
        let Some(a) = self.freshness.semantic_activation() else {
            return Ok(0);
        };
        let mut pending = 0;
        for node in self.archive.nodes.iter() {
            if self
                .activity_position_for_turn(&node.conversation_id, &node.id)
                .is_some_and(|at| at > a.accepted_turn)
                && !self
                    .freshness
                    .contains_event(&crate::freshness_storage::semantic_live_event_id(
                        a.owner_uuid,
                        &node.conversation_id,
                        &node.id,
                    ))
                    .map_err(CvaError::Freshness)?
            {
                pending += 1;
            }
        }
        Ok(pending)
    }

    /// Process at most limit uncompleted eligible turns against one retained generation.
    /// Pending work remains discoverable from Archive after restart; there is no queue.
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
        let owner = activation.owner_uuid;
        let mut turns: Vec<_> = self
            .archive
            .nodes
            .iter()
            .filter_map(|node| {
                let at = self.activity_position_for_turn(&node.conversation_id, &node.id)?;
                (at > activation.accepted_turn).then(|| (at, node.clone()))
            })
            .collect();
        turns.sort_by_key(|(at, _)| *at);
        let mut attempted = 0;
        for (_, node) in turns {
            let id = crate::freshness_storage::semantic_live_event_id(
                owner,
                &node.conversation_id,
                &node.id,
            );
            if self
                .freshness
                .contains_event(&id)
                .map_err(CvaError::Freshness)?
            {
                continue;
            }
            if attempted == limit {
                break;
            }
            attempted += 1;
            let source = match self.prepare_accepted_semantic_source(&node, Some(generation))? {
                crate::cva_semantic_source::SemanticSourceReadiness::Pending => {
                    report.pending_turns += 1;
                    continue;
                }
                crate::cva_semantic_source::SemanticSourceReadiness::Ready(source) => source,
            };
            // Births at or after the accepted cut are excluded independently of
            // asynchronous cohort publication; provenance adds a second guard.
            report.reinforced_memories +=
                self.apply_prepared_semantic_reinforcement(source, &Default::default())?;
            report.completed_turns += 1;
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

    pub(crate) fn materialize_semantic_turn_fragments(
        &mut self,
    ) -> Result<(), crate::ArchiveError> {
        let Some(a) = self.freshness.semantic_activation() else {
            return Ok(());
        };
        let nodes: Vec<_> = self
            .archive
            .nodes
            .iter()
            .filter(|node| {
                self.activity_position_for_turn(&node.conversation_id, &node.id)
                    .is_some_and(|at| at > a.accepted_turn)
            })
            .cloned()
            .collect();
        for node in nodes {
            // Materialize this exact node, not every ancestor on its path.
            // Archive validates and owns the immutable fragment publication.
            self.archive.put_fragment(
                &mut self.container,
                crate::Fragment {
                    id: crate::fragment_store::fragment_id(
                        &node.conversation_id,
                        &node.id,
                        &node.id,
                    ),
                    conversation_id: node.conversation_id,
                    start_node_id: node.id.clone(),
                    end_node_id: node.id,
                },
            )?;
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
        let a = self
            .freshness
            .semantic_activation()
            .expect("enabled")
            .clone();
        let mut pending = false;
        for node in self.archive.nodes.iter() {
            if self
                .activity_position_for_turn(&node.conversation_id, &node.id)
                .is_some_and(|at| at > a.accepted_turn)
                && !self
                    .freshness
                    .contains_event(&crate::freshness_storage::semantic_live_event_id(
                        a.owner_uuid,
                        &node.conversation_id,
                        &node.id,
                    ))
                    .map_err(CvaError::Freshness)?
            {
                pending = true;
                break;
            }
        }
        if !pending {
            return Ok(None);
        }
        self.materialize_semantic_turn_fragments()?;
        let archive_version = self.archive_version();
        if self
            .current_vector_generation(profile_id)
            .is_some_and(|g| g.source_archive_version >= archive_version)
        {
            return Ok(None);
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
