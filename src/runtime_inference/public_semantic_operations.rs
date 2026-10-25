//! Canonical Gateway execution for live semantic Freshness.
//!
//! Source/store facts and durable effects are obtained through owner lifecycle
//! contracts; scoring and deterministic target ranking stay in Freshness policy.

use crate::freshness::semantic_policy::SemanticEffect;
use crate::owner_lifecycle::semantic_source::{AcceptedSemanticSource, SemanticSourceReadiness};
use crate::{CvaError, FragmentId, Memory, MemoryId, Node, VectorGenerationId};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SemanticFreshnessReport {
    pub completed_fragments: usize,
    pub pending_fragments: usize,
    pub reinforced_memories: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SemanticCandidateSearch {
    pub effects: Vec<SemanticEffect>,
    pub scanned_memories: usize,
    pub eligible_memories: usize,
    pub excluded_memories: usize,
    pub target_evidence_digest: [u8; 32],
}

#[derive(Clone, Debug, PartialEq)]
#[cfg(test)]
pub(crate) struct SemanticTargetSnapshot {
    pub memory: Memory,
    pub body_id: crate::MemoryBodyId,
    pub admitted_at_turn: Option<u64>,
    pub birth_turn: Option<u64>,
    pub vector: Vec<f32>,
}

#[cfg(test)]
pub(crate) fn freshness_policy_digest(policy: crate::freshness::FreshnessPolicy) -> [u8; 32] {
    crate::freshness::semantic_policy::freshness_policy_digest(policy)
}

impl crate::RelExecutionOwner {
    pub(crate) fn runtime_semantic_fragment_sources(
        &self,
        id: FragmentId,
    ) -> Result<Vec<(String, String)>, CvaError> {
        crate::owner_lifecycle::semantic_freshness_operations::fragment_sources(&self.owner, id)
            .map_err(Into::into)
    }

    pub(crate) fn runtime_semantic_fragment_position(
        &self,
        id: FragmentId,
    ) -> Result<Option<u64>, CvaError> {
        crate::owner_lifecycle::semantic_freshness_operations::fragment_position(&self.owner, id)
            .map_err(Into::into)
    }

    pub(crate) fn runtime_semantic_source_excludes_memory(
        &self,
        source: &AcceptedSemanticSource,
        memory: &Memory,
    ) -> Result<bool, CvaError> {
        crate::owner_lifecycle::semantic_freshness_operations::source_excludes_memory(
            &self.owner,
            source,
            memory,
        )
        .map_err(Into::into)
    }

    pub(crate) fn runtime_prepare_semantic_fragment(
        &self,
        id: FragmentId,
        generation_id: VectorGenerationId,
    ) -> Result<SemanticSourceReadiness, CvaError> {
        crate::owner_lifecycle::semantic_freshness_operations::prepare_fragment(
            &self.owner,
            id,
            generation_id,
        )
        .map_err(Into::into)
    }

    pub(crate) fn runtime_semantic_freshness_pending_fragments(&self) -> Result<usize, CvaError> {
        Ok(self.pending_semantic_fragments()?.len())
    }

    fn pending_semantic_fragments(&self) -> Result<Vec<(u64, FragmentId)>, CvaError> {
        let read = crate::owner_lifecycle::semantic_freshness_operations::archive_read(&self.owner);
        crate::semantic_archive_batch::pending_fragments(&read)
    }

    pub(crate) fn runtime_process_semantic_freshness(
        &mut self,
        generation: VectorGenerationId,
        limit: usize,
    ) -> Result<SemanticFreshnessReport, CvaError> {
        let mut report = SemanticFreshnessReport::default();
        let selected = crate::owner_lifecycle::semantic_freshness_operations::vector_generation(
            &self.owner,
            generation,
        )
        .ok_or(crate::VectorGenerationError::MissingGeneration)?;
        if self.runtime_current_vector_generation(selected.compatibility_profile_id)
            != Some(selected)
        {
            return Err(CvaError::Freshness(
                "semantic Freshness requires complete current Archive coverage".into(),
            ));
        }
        for (_, fragment) in self.pending_semantic_fragments()?.into_iter().take(limit) {
            let source = match self.runtime_prepare_semantic_fragment(fragment, generation)? {
                SemanticSourceReadiness::Pending => {
                    report.pending_fragments += 1;
                    continue;
                }
                SemanticSourceReadiness::Ready(source) => source,
            };
            report.reinforced_memories +=
                self.runtime_apply_prepared_semantic_reinforcement(source, &Default::default())?;
            report.completed_fragments += 1;
        }
        Ok(report)
    }

    #[cfg(test)]
    pub(crate) fn runtime_apply_accepted_semantic_reinforcement(
        &mut self,
        node: &Node,
        source_memory_cohort: &std::collections::BTreeSet<MemoryId>,
    ) -> Result<usize, CvaError> {
        if !crate::owner_lifecycle::semantic_freshness_operations::accepted_node(&self.owner, node)
        {
            return Err(CvaError::Freshness(
                "conflicting accepted semantic source".into(),
            ));
        }
        let Some(candidate) =
            crate::owner_lifecycle::semantic_freshness_operations::latest_generation(&self.owner)
        else {
            return Ok(0);
        };
        let Some(generation) =
            self.runtime_current_vector_generation(candidate.compatibility_profile_id)
        else {
            return Ok(0);
        };
        let mut applied = 0;
        let fragments =
            crate::owner_lifecycle::semantic_freshness_operations::fragments(&self.owner);
        for fragment in fragments {
            let keys = self.runtime_semantic_fragment_sources(fragment.id)?;
            if !keys.contains(&(node.conversation_id.clone(), node.id.clone())) {
                continue;
            }
            let id = crate::freshness::historical_semantic::live_semantic_event_id(
                self.runtime_owner_uuid()
                    .ok_or(CvaError::InvalidContainerIdentity("semantic owner"))?,
                fragment.id,
            );
            if crate::owner_lifecycle::semantic_freshness_operations::event_seen(&self.owner, &id)
                .map_err(CvaError::Freshness)?
            {
                continue;
            }
            if let SemanticSourceReadiness::Ready(source) =
                self.runtime_prepare_semantic_fragment(fragment.id, generation.id)?
            {
                applied += self
                    .runtime_apply_prepared_semantic_reinforcement(source, source_memory_cohort)?;
            }
        }
        Ok(applied)
    }

    pub(crate) fn runtime_apply_prepared_semantic_reinforcement(
        &mut self,
        source: AcceptedSemanticSource,
        source_memory_cohort: &std::collections::BTreeSet<MemoryId>,
    ) -> Result<usize, CvaError> {
        if source.owner_uuid
            != self
                .runtime_owner_uuid()
                .ok_or(CvaError::InvalidContainerIdentity("semantic owner"))?
        {
            return Err(CvaError::Freshness(
                "semantic fragment belongs to another owner".into(),
            ));
        }
        let fragment = source
            .fragment
            .as_ref()
            .ok_or_else(|| CvaError::Freshness("live semantic source must be a fragment".into()))?;
        if self.runtime_semantic_fragment_sources(fragment.fragment_id)? != fragment.sources
            || self.runtime_semantic_fragment_position(fragment.fragment_id)?
                != Some(source.activity_position)
        {
            return Err(CvaError::Freshness(
                "conflicting semantic fragment availability".into(),
            ));
        }
        let identity = crate::freshness::historical_semantic::live_semantic_event_identity(
            source.owner_uuid,
            fragment.fragment_id,
            source.base_policy,
        );
        if crate::owner_lifecycle::semantic_freshness_operations::event_seen(
            &self.owner,
            &identity.event_id,
        )
        .map_err(CvaError::Freshness)?
        {
            return Ok(0);
        }
        let search =
            self.runtime_search_eligible_semantic_targets(&source, source_memory_cohort)?;
        let effects = search
            .effects
            .iter()
            .map(|effect| (effect.memory_id, effect.bonus))
            .collect::<std::collections::BTreeMap<_, _>>();
        let evidence = crate::freshness_storage::SemanticEventEvidence {
            fragment: source.fragment.clone(),
            owner_uuid: source.owner_uuid,
            conversation_id: source.node.conversation_id.clone(),
            node_id: source.node.id.clone(),
            accepted_turn: source.activity_position,
            base_policy_digest: identity.base_policy_digest,
            semantic_policy_version: source.semantic_policy.version,
            semantic_policy_digest: source.policy_digest,
            compatibility_profile_id: source.generation.compatibility_profile_id.0,
            source_evidence_digest: source.source_digest,
            target_evidence_digest: search.target_evidence_digest,
        };
        let payload = crate::owner_lifecycle::semantic_freshness_operations::encode_semantic_event(
            &self.owner,
            source.owner_uuid,
            evidence,
            effects.clone(),
        )
        .map_err(CvaError::Freshness)?;
        crate::owner_lifecycle::semantic_freshness_operations::validate_semantic_preappend(
            &self.owner,
            source.owner_uuid,
            &payload,
        )
        .map_err(CvaError::Freshness)?;
        crate::owner_lifecycle::semantic_freshness_operations::append_semantic_event(
            &mut self.owner,
            source.owner_uuid,
            &payload,
        )?;
        crate::owner_lifecycle::semantic_freshness_operations::ingest_appended_semantic_event(
            &mut self.owner,
            &payload,
        )
        .map_err(CvaError::Freshness)?;
        if !effects.is_empty() {
            self.runtime_freshness_refresh_due(
                effects.keys().copied().collect::<Vec<_>>(),
                source.activity_position,
            )?;
        }
        Ok(effects.len())
    }

    pub(crate) fn runtime_search_eligible_semantic_targets(
        &mut self,
        source: &AcceptedSemanticSource,
        source_memory_cohort: &std::collections::BTreeSet<MemoryId>,
    ) -> Result<SemanticCandidateSearch, CvaError> {
        let memory_targets =
            crate::owner_lifecycle::semantic_freshness_operations::semantic_memory_targets(
                &self.owner,
                source.generation.compatibility_profile_id,
            )
            .map_err(CvaError::from)?;
        let scanned_memories = memory_targets.len();
        let mut source_memory_cohort = source_memory_cohort.clone();
        let mut source_episodes = std::collections::HashSet::new();
        // Pass one hydrates one Memory at a time to identify the complete source Episode cohort.
        for target in &memory_targets {
            let memory = crate::owner_lifecycle::semantic_freshness_operations::memory(
                &self.owner,
                target.id,
            )
            .map_err(CvaError::from)?;
            if source_memory_cohort.contains(&target.id)
                || source_has_memory_provenance(source, &memory)
            {
                source_memory_cohort.insert(target.id);
                if let Some(episode) = memory.source_episode_id {
                    source_episodes.insert(episode);
                }
            }
        }
        let mut cached: Option<(
            crate::MemoryVectorId,
            crate::MemoryVectorSet,
            lodestone_packed::PackedVectors,
        )> = None;
        let mut selected = Vec::new();
        let mut eligible_memories = 0;
        let mut excluded_memories = 0;
        // Pass two again hydrates one Memory at a time while applying exact eligibility and ranking.
        for target in memory_targets {
            let id = target.id;
            let memory =
                crate::owner_lifecycle::semantic_freshness_operations::memory(&self.owner, id)
                    .map_err(CvaError::from)?;
            if memory
                .source_episode_id
                .is_some_and(|episode| source_episodes.contains(&episode))
                || self.runtime_semantic_source_excludes_memory(source, &memory)?
            {
                source_memory_cohort.insert(id);
            }
            let Some(location) =
                crate::owner_lifecycle::semantic_freshness_operations::vector_location(
                    &self.owner,
                    source.generation.compatibility_profile_id,
                    target.body_id,
                )
            else {
                excluded_memories += 1;
                continue;
            };
            if cached
                .as_ref()
                .is_none_or(|(set_id, _, _)| *set_id != location.set_id)
            {
                let set = crate::owner_lifecycle::semantic_freshness_operations::memory_vector_set(
                    &self.owner,
                    location.set_id,
                )
                .map_err(CvaError::from)?;
                let packed = crate::owner_lifecycle::semantic_freshness_operations::packed_vectors(
                    &self.owner,
                    set.packed_vector_id,
                )
                .map_err(CvaError::from)?;
                cached = Some((location.set_id, set, packed));
            }
            let (_, set, packed) = cached.as_ref().expect("loaded target set");
            let bytes = packed
                .row(location.row as usize)
                .ok_or_else(|| CvaError::Freshness("missing semantic target row".into()))?;
            let vector: Vec<f32> = bytes
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("four bytes")))
                .collect();
            let mut binding = Sha256::new();
            binding.update(target.body_id.0);
            binding.update(location.set_id.0);
            binding.update(location.row.to_le_bytes());
            binding.update(set.packed_vector_id.0);
            binding.update(bytes);
            let binding: [u8; 32] = binding.finalize().into();
            let (admitted_at_turn, birth_turn) =
                crate::owner_lifecycle::semantic_freshness_operations::target_timing(
                    &self.owner,
                    id,
                );
            let source_vectors = source
                .vectors
                .iter()
                .map(
                    |source| crate::freshness::historical_semantic::LiveSemanticSourceVector {
                        fragment_id: &source.fragment_id.0,
                        components: &source.components,
                    },
                )
                .collect::<Vec<_>>();
            let source_excluded =
                source_memory_cohort.contains(&id) || source_has_memory_provenance(source, &memory);
            let admitted_before_source = admitted_at_turn
                .is_some_and(|turn| turn <= source.activity_position)
                && birth_turn.is_some_and(|turn| turn < source.activity_position);
            let lifecycle_eligible = !memory.archived
                && memory.superseded_by.is_none()
                && matches!(memory.lifecycle_state.as_str(), "knowledge" | "canonical");
            if source_excluded || !admitted_before_source || !lifecycle_eligible {
                excluded_memories += 1;
                continue;
            }
            let (eligible, cosine, effects) =
                crate::freshness::historical_semantic::evaluate_live_semantic_target(
                    source.semantic_policy,
                    &source_vectors,
                    id,
                    binding,
                    source.generation.id.0 != [0; 32],
                    &vector,
                )
                .map_err(|error| CvaError::Freshness(error.to_string()))?;
            if eligible {
                eligible_memories += 1;
            } else {
                excluded_memories += 1;
            }
            if let Some(effect) = effects.into_iter().next() {
                selected.push((effect, binding, cosine));
                selected.sort_by(|(a, _, ac), (b, _, bc)| {
                    bc.total_cmp(ac).then(a.memory_id.0.cmp(&b.memory_id.0))
                });
                selected.truncate(source.semantic_policy.candidate_limit);
            }
        }
        selected.sort_by_key(|(effect, _, _)| effect.memory_id.0);
        let mut digest = Sha256::new();
        digest.update(b"reliquary:semantic-target-bindings:v1\0");
        for (effect, binding, _) in &selected {
            digest.update(effect.memory_id.0);
            digest.update(binding);
            digest.update(effect.bonus.to_le_bytes());
        }
        Ok(SemanticCandidateSearch {
            effects: selected.into_iter().map(|(effect, _, _)| effect).collect(),
            scanned_memories,
            eligible_memories,
            excluded_memories,
            target_evidence_digest: digest.finalize().into(),
        })
    }
}

#[cfg(test)]
pub(crate) fn plan_eligible_semantic_targets(
    source: &AcceptedSemanticSource,
    source_memory_cohort: &std::collections::BTreeSet<MemoryId>,
    targets: impl IntoIterator<Item = SemanticTargetSnapshot>,
) -> Result<(Vec<SemanticEffect>, usize, usize), CvaError> {
    let mut candidates = Vec::new();
    let mut eligible_memories = 0;
    let mut filtered = 0;
    for target in targets {
        let id = target.memory.id;
        let source_excluded = source_memory_cohort.contains(&id)
            || source_has_memory_provenance(source, &target.memory);
        let admitted_before_source = target
            .admitted_at_turn
            .is_some_and(|turn| turn <= source.activity_position)
            && target
                .birth_turn
                .is_some_and(|turn| turn < source.activity_position);
        let lifecycle_eligible = !target.memory.archived
            && target.memory.superseded_by.is_none()
            && matches!(
                target.memory.lifecycle_state.as_str(),
                "knowledge" | "canonical"
            );
        if source_excluded || !admitted_before_source || !lifecycle_eligible {
            filtered += 1;
            continue;
        }
        if crate::freshness::semantic_policy::cosine_similarity(&target.vector, &target.vector)
            .is_err()
            || source
                .vectors
                .iter()
                .any(|vector| vector.components.len() != target.vector.len())
        {
            filtered += 1;
            continue;
        }
        eligible_memories += 1;
        for source_vector in &source.vectors {
            let Ok(cosine) = crate::freshness::semantic_policy::cosine_similarity(
                &source_vector.components,
                &target.vector,
            ) else {
                continue;
            };
            candidates.push(crate::freshness::semantic_policy::SemanticCandidate {
                memory_id: id,
                cosine,
                fragment_id: source_vector.fragment_id.0.to_vec(),
                vector_binding_id: target.body_id.0,
                profile_compatible: true,
                generation_valid: source.generation.id.0 != [0; 32],
                source_excluded: false,
            });
        }
    }
    let effects =
        crate::freshness::semantic_policy::plan_local_effects(source.semantic_policy, candidates)
            .map_err(|error| CvaError::Freshness(error.to_string()))?;
    Ok((effects, eligible_memories, filtered))
}

fn source_has_memory_provenance(source: &AcceptedSemanticSource, memory: &Memory) -> bool {
    let matches = |conversation: Option<&str>, node: Option<&str>| {
        conversation
            .zip(node)
            .is_some_and(|(c, n)| source.represents(c, n))
    };
    matches(
        memory.content_source_conversation_id.as_deref(),
        memory.content_source_node_id.as_deref(),
    ) || matches(
        memory.grounding_source_conversation_id.as_deref(),
        memory.grounding_source_node_id.as_deref(),
    ) || memory.source_ref.as_ref().is_some_and(|reference| {
        matches(
            reference.content_source_conversation_id.as_deref(),
            reference.content_source_node_id.as_deref(),
        ) || matches(
            reference.grounding_source_conversation_id.as_deref(),
            reference.grounding_source_node_id.as_deref(),
        )
    })
}
