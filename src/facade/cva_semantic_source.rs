//! Shared fragment-local semantic search and Freshness acceptance.
use crate::freshness::semantic_policy::{SemanticPolicy, cosine_similarity};
use crate::{Cva, CvaError, FragmentId, Node, VectorGeneration};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SemanticSourceVector {
    pub fragment_id: FragmentId,
    pub row: u64,
    pub components: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AcceptedSemanticSource {
    pub owner_uuid: [u8; 16],
    pub fragment: Option<crate::freshness_storage::SemanticFragmentEvidence>,
    pub node: Node,
    pub activity_position: u64,
    pub generation: VectorGeneration,
    pub base_policy: crate::freshness::FreshnessPolicy,
    pub semantic_policy: SemanticPolicy,
    pub policy_digest: [u8; 32],
    pub source_digest: [u8; 32],
    pub vectors: Vec<SemanticSourceVector>,
    pub invalid_vectors: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SemanticSourceReadiness {
    Pending,
    Ready(AcceptedSemanticSource),
}

impl Cva {
    pub(crate) fn freshness_replay_semantic_event(
        &mut self,
        event_id: String,
        evidence: crate::freshness_storage::SemanticEventEvidence,
        effects: std::collections::BTreeMap<crate::MemoryId, i16>,
    ) -> Result<(), CvaError> {
        let owner = self.owner_uuid().ok_or(CvaError::InvalidContainerIdentity(
            "semantic replay requires owner",
        ))?;
        if evidence.owner_uuid != owner
            || self.semantic_evidence_position(&evidence)? != evidence.accepted_turn
            || event_id != evidence.event_id()
            || evidence.base_policy_digest != freshness_policy_digest(self.freshness.policy())
            || evidence.semantic_policy_version != SemanticPolicy::default().version
            || evidence.semantic_policy_digest
                != SemanticPolicy::default()
                    .digest()
                    .map_err(|e| CvaError::Freshness(e.to_string()))?
        {
            return Err(CvaError::Freshness(
                "unsupported or conflicting semantic replay evidence".into(),
            ));
        }
        if self
            .freshness
            .contains_event(&event_id)
            .map_err(CvaError::Freshness)?
        {
            return if self
                .freshness
                .semantic_event_matches(&event_id, &evidence, &effects)
                .map_err(CvaError::Freshness)?
            {
                Ok(())
            } else {
                Err(CvaError::Freshness(
                    "semantic replay identity conflicts".into(),
                ))
            };
        }
        let payload = self
            .freshness
            .encode_semantic_event(owner, evidence.clone(), effects.clone())
            .map_err(CvaError::Freshness)?;
        self.freshness
            .validate_semantic_preappend(owner, &payload)
            .map_err(CvaError::Freshness)?;
        self.freshness
            .persist_policy_if_needed(&mut self.container, owner)
            .map_err(CvaError::Freshness)?;
        self.container.append(&payload)?;
        self.container.sync()?;
        self.freshness
            .ingest_appended(&payload)
            .map_err(CvaError::Freshness)?;
        self.freshness_refresh_due(effects.keys().copied(), evidence.accepted_turn)?;
        Ok(())
    }

    /// Rediscover retained fragments containing this accepted turn; each fragment
    /// is independently accepted once at its availability. No new fragments are created.
    pub(crate) fn apply_accepted_semantic_reinforcement(
        &mut self,
        node: &crate::Node,
        source_memory_cohort: &std::collections::BTreeSet<crate::MemoryId>,
    ) -> Result<usize, CvaError> {
        if self.archive.nodes.get(&node.conversation_id, &node.id) != Some(node) {
            return Err(CvaError::Freshness(
                "conflicting accepted semantic source".into(),
            ));
        }
        let Some(candidate) = self
            .vector_generations
            .generations()
            .iter()
            .max_by_key(|g| g.vector_version)
            .copied()
        else {
            return Ok(0);
        };
        // Historical/offline fill may retain a partial generation so retries
        // can reuse rows. Such evidence cannot drive live scoring before it is
        // exact, complete current coverage for its profile.
        let Some(generation) = self.current_vector_generation(candidate.compatibility_profile_id)
        else {
            return Ok(0);
        };
        let mut applied = 0;
        for fragment in self.fragments() {
            let keys = self.semantic_fragment_sources(fragment.id)?;
            if !keys.contains(&(node.conversation_id.clone(), node.id.clone())) {
                continue;
            }
            let id = crate::freshness_storage::semantic_fragment_event_id(
                self.owner_uuid()
                    .ok_or(CvaError::InvalidContainerIdentity("semantic owner"))?,
                fragment.id,
            );
            if self
                .freshness
                .contains_event(&id)
                .map_err(CvaError::Freshness)?
            {
                continue;
            }
            if let SemanticSourceReadiness::Ready(source) =
                self.prepare_semantic_fragment(fragment.id, generation.id)?
            {
                applied +=
                    self.apply_prepared_semantic_reinforcement(source, source_memory_cohort)?;
            }
        }
        Ok(applied)
    }

    pub(crate) fn apply_prepared_semantic_reinforcement(
        &mut self,
        source: AcceptedSemanticSource,
        source_memory_cohort: &std::collections::BTreeSet<crate::MemoryId>,
    ) -> Result<usize, CvaError> {
        if source.owner_uuid
            != self
                .owner_uuid()
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
        if self.semantic_fragment_sources(fragment.fragment_id)? != fragment.sources
            || self.semantic_fragment_position(fragment.fragment_id)?
                != Some(source.activity_position)
        {
            return Err(CvaError::Freshness(
                "conflicting semantic fragment availability".into(),
            ));
        }
        {
            let id = crate::freshness_storage::semantic_fragment_event_id(
                source.owner_uuid,
                fragment.fragment_id,
            );
            if self
                .freshness
                .contains_event(&id)
                .map_err(CvaError::Freshness)?
            {
                return Ok(0);
            }
        }
        let search = self.search_eligible_semantic_targets(&source, source_memory_cohort)?;
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
            base_policy_digest: freshness_policy_digest(source.base_policy),
            semantic_policy_version: source.semantic_policy.version,
            semantic_policy_digest: source.policy_digest,
            compatibility_profile_id: source.generation.compatibility_profile_id.0,
            source_evidence_digest: source.source_digest,
            target_evidence_digest: search.target_evidence_digest,
        };
        let payload = self
            .freshness
            .encode_semantic_event(source.owner_uuid, evidence, effects.clone())
            .map_err(CvaError::Freshness)?;
        // Validate canonical projection and identity before durable append.
        // A rejected late event must not poison the accepted journal prefix.
        self.freshness
            .validate_semantic_preappend(source.owner_uuid, &payload)
            .map_err(CvaError::Freshness)?;
        self.freshness
            .persist_policy_if_needed(&mut self.container, source.owner_uuid)
            .map_err(CvaError::Freshness)?;
        self.container
            .append(&payload)
            .map_err(|error| CvaError::Freshness(error.to_string()))?;
        self.container
            .sync()
            .map_err(|error| CvaError::Freshness(error.to_string()))?;
        self.freshness
            .ingest_appended(&payload)
            .map_err(CvaError::Freshness)?;
        if !effects.is_empty() {
            self.freshness_refresh_due(
                effects.keys().copied().collect::<Vec<_>>(),
                source.activity_position,
            )?;
        }
        Ok(effects.len())
    }
}

fn freshness_policy_digest(policy: crate::freshness::FreshnessPolicy) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"reliquary:freshness-policy:v1\0");
    digest.update(policy.decay_turns_per_point.to_le_bytes());
    digest.update(policy.access_principal.to_le_bytes());
    digest.update(policy.linkage_principal.to_le_bytes());
    digest.update(policy.local_hop_cost.to_le_bytes());
    digest.update(policy.outside_hop_cost.to_le_bytes());
    digest.finalize().into()
}

#[cfg(test)]
fn semantic_target_digest(effects: &std::collections::BTreeMap<crate::MemoryId, i16>) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"reliquary:semantic-target-evidence:v1\0");
    for (memory_id, bonus) in effects {
        digest.update(memory_id.0);
        digest.update(bonus.to_le_bytes());
    }
    digest.finalize().into()
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SemanticCandidateSearch {
    pub effects: Vec<crate::freshness::semantic_policy::SemanticEffect>,
    pub scanned_memories: usize,
    pub eligible_memories: usize,
    pub excluded_memories: usize,
    pub target_evidence_digest: [u8; 32],
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SemanticTargetSnapshot {
    pub memory: crate::Memory,
    pub body_id: crate::MemoryBodyId,
    pub admitted_at_turn: Option<u64>,
    pub birth_turn: Option<u64>,
    pub vector: Vec<f32>,
}

impl Cva {
    /// Search only this REL's current Memory bindings at the accepted source cut.
    ///
    /// The complete newly-created cohort must be supplied by the publication
    /// owner. Filtering precedes the semantic usable limit; the policy planner
    /// then performs strongest-per-target reduction and deterministic ranking.
    pub(crate) fn search_eligible_semantic_targets(
        &mut self,
        source: &AcceptedSemanticSource,
        source_memory_cohort: &std::collections::BTreeSet<crate::MemoryId>,
    ) -> Result<SemanticCandidateSearch, CvaError> {
        let mut memory_ids = self.memories.current_ids();
        // Group existing owner IDs by immutable vector set. Retain only one
        // packed matrix at a time; never reload/hash an entire matrix per row.
        memory_ids.sort_by_cached_key(|id| {
            self.memories
                .current_body_id(*id)
                .ok()
                .and_then(|body| {
                    self.memory_vector_location(source.generation.compatibility_profile_id, body)
                })
                .map(|location| location.set_id.0)
                .unwrap_or([255; 32])
        });
        // A grounding/content source on one member excludes the whole generating
        // Episode cohort, even when that source lies outside the Episode path.
        let mut source_memory_cohort = source_memory_cohort.clone();
        let mut source_episodes = std::collections::HashSet::new();
        for id in &memory_ids {
            let memory = self
                .memories
                .memory(&mut self.container, *id)
                .map_err(|e| CvaError::Freshness(e.to_string()))?;
            if source_memory_cohort.contains(id) || source_has_memory_provenance(source, &memory) {
                source_memory_cohort.insert(*id);
                if let Some(episode) = memory.source_episode_id {
                    source_episodes.insert(episode);
                }
            }
        }
        let mut cached: Option<(
            crate::MemoryVectorId,
            crate::MemoryVectorSet,
            crate::PackedVectors,
        )> = None;
        let mut selected = Vec::new();
        let mut eligible_memories = 0;
        let mut excluded_memories = 0;
        for id in &memory_ids {
            let memory = self
                .memories
                .memory(&mut self.container, *id)
                .map_err(|e| CvaError::Freshness(e.to_string()))?;
            if memory
                .source_episode_id
                .is_some_and(|id| source_episodes.contains(&id))
                || self.semantic_source_excludes_memory(source, &memory)?
            {
                source_memory_cohort.insert(*id);
            }
            let body_id = self
                .memories
                .current_body_id(*id)
                .map_err(|e| CvaError::Freshness(e.to_string()))?;
            let Some(location) =
                self.memory_vector_location(source.generation.compatibility_profile_id, body_id)
            else {
                excluded_memories += 1;
                continue;
            };
            if cached
                .as_ref()
                .is_none_or(|(id, _, _)| *id != location.set_id)
            {
                let set = self.memory_vectors(location.set_id)?;
                let packed = self.packed_vectors(set.packed_vector_id)?;
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
            binding.update(body_id.0);
            binding.update(location.set_id.0);
            binding.update(location.row.to_le_bytes());
            binding.update(set.packed_vector_id.0);
            binding.update(bytes);
            let binding: [u8; 32] = binding.finalize().into();
            let cosine = source
                .vectors
                .iter()
                .filter_map(|v| cosine_similarity(&v.components, &vector).ok())
                .max_by(f64::total_cmp)
                .unwrap_or(-1.0);
            let target = SemanticTargetSnapshot {
                memory,
                body_id,
                admitted_at_turn: self
                    .freshness
                    .records()
                    .get(id)
                    .and_then(|r| r.admitted_at_turn),
                birth_turn: self.freshness.birth_turn(*id),
                vector,
            };
            let (effects, eligible, excluded) =
                plan_eligible_semantic_targets(source, &source_memory_cohort, [target])?;
            eligible_memories += eligible;
            excluded_memories += excluded;
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
            scanned_memories: memory_ids.len(),
            eligible_memories,
            excluded_memories,
            target_evidence_digest: digest.finalize().into(),
        })
    }
}

fn source_has_memory_provenance(source: &AcceptedSemanticSource, memory: &crate::Memory) -> bool {
    let matches =
        |c: Option<&str>, n: Option<&str>| c.zip(n).is_some_and(|(c, n)| source.represents(c, n));
    matches(
        memory.content_source_conversation_id.as_deref(),
        memory.content_source_node_id.as_deref(),
    ) || matches(
        memory.grounding_source_conversation_id.as_deref(),
        memory.grounding_source_node_id.as_deref(),
    ) || memory.source_ref.as_ref().is_some_and(|r| {
        matches(
            r.content_source_conversation_id.as_deref(),
            r.content_source_node_id.as_deref(),
        ) || matches(
            r.grounding_source_conversation_id.as_deref(),
            r.grounding_source_node_id.as_deref(),
        )
    })
}

fn plan_eligible_semantic_targets(
    source: &AcceptedSemanticSource,
    source_memory_cohort: &std::collections::BTreeSet<crate::MemoryId>,
    targets: impl IntoIterator<Item = SemanticTargetSnapshot>,
) -> Result<
    (
        Vec<crate::freshness::semantic_policy::SemanticEffect>,
        usize,
        usize,
    ),
    CvaError,
> {
    use crate::freshness::semantic_policy::{
        SemanticCandidate, cosine_similarity, plan_local_effects,
    };
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
        if cosine_similarity(&target.vector, &target.vector).is_err()
            || source
                .vectors
                .iter()
                .any(|v| v.components.len() != target.vector.len())
        {
            filtered += 1;
            continue;
        }
        eligible_memories += 1;
        for source_vector in &source.vectors {
            let Ok(cosine) = cosine_similarity(&source_vector.components, &target.vector) else {
                continue;
            };
            candidates.push(SemanticCandidate {
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
    let effects = plan_local_effects(source.semantic_policy, candidates)
        .map_err(|error| CvaError::Freshness(error.to_string()))?;
    Ok((effects, eligible_memories, filtered))
}

#[cfg(test)]
#[path = "cva_semantic_source_tests.rs"]
mod tests;
