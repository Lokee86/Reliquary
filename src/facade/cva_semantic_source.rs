//! Read-only accepted-turn source production. No scoring or receipt publication.
use crate::freshness::semantic_policy::{SemanticPolicy, cosine_similarity};
use crate::{Cva, CvaError, FragmentId, Node, VectorGeneration, VectorGenerationId};
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
    pub node: Node,
    pub activity_position: u64,
    pub generation: VectorGeneration,
    pub base_policy: crate::freshness::FreshnessPolicy,
    pub semantic_policy: SemanticPolicy,
    pub policy_digest: [u8; 32],
    pub source_digest: [u8; 32],
    pub vectors: Vec<SemanticSourceVector>,
    pub excluded_mixed_fragments: usize,
    pub invalid_vectors: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SemanticSourceReadiness {
    Pending,
    Ready(AcceptedSemanticSource),
}

impl Cva {
    /// Select retained genuine-source evidence after Archive/vector publication.
    /// Exact Node equality prevents a caller from substituting another source.
    /// This seam is intentionally unwired until the live durability/application gates.
    pub(crate) fn prepare_accepted_semantic_source(
        &mut self,
        expected: &Node,
        generation_id: Option<VectorGenerationId>,
    ) -> Result<SemanticSourceReadiness, CvaError> {
        let owner_uuid = self.owner_uuid().ok_or(CvaError::InvalidContainerIdentity(
            "semantic source requires REL UUID",
        ))?;
        let actual = self
            .archive
            .nodes
            .get(&expected.conversation_id, &expected.id)
            .ok_or_else(|| CvaError::Freshness("missing accepted semantic source".into()))?;
        if actual != expected {
            return Err(CvaError::Freshness(
                "conflicting accepted semantic source".into(),
            ));
        }
        let activity_position = self
            .activity_position_for_turn(&expected.conversation_id, &expected.id)
            .ok_or_else(|| {
                CvaError::Freshness("semantic source is not an activity-bearing turn".into())
            })?;
        let Some(generation_id) = generation_id else {
            return Ok(SemanticSourceReadiness::Pending);
        };
        let generation = self.vector_generation(generation_id)?;
        if self
            .archive
            .activity_cut_at_archive_version(generation.source_archive_version)?
            < activity_position
        {
            return Ok(SemanticSourceReadiness::Pending);
        }
        let profile = self
            .compatibility_profiles
            .get(generation.compatibility_profile_id)
            .ok_or_else(|| CvaError::Freshness("missing semantic source profile".into()))?
            .clone();
        let set = self.archive_vectors(generation.archive_vector_id)?;
        let packed = self.packed_vectors(set.packed_vector_id)?;
        if packed.schema().dimensions != profile.dimensions
            || packed.schema().scalar != crate::ScalarType::F32
        {
            return Err(CvaError::Freshness(
                "incompatible semantic source matrix".into(),
            ));
        }

        // Completeness is proved over all source-containing fragments at this
        // selected Archive cut, not inferred from the existence of one vector.
        let fragments = self.fragments();
        let mut relevant = Vec::new();
        for fragment in fragments {
            if fragment.conversation_id != expected.conversation_id
                || self
                    .archive
                    .fragments
                    .archive_version(fragment.id)
                    .is_none_or(|version| version > generation.source_archive_version)
            {
                continue;
            }
            let turns = self.fragment_turns(fragment.id)?;
            if turns.iter().any(|turn| turn.node_id == expected.id) {
                relevant.push((fragment, turns.len()));
            }
        }
        if relevant.is_empty() {
            return Ok(SemanticSourceReadiness::Pending);
        }
        relevant.sort_by_key(|(fragment, _)| fragment.id.0);
        let mut vectors = Vec::new();
        let mut excluded_mixed_fragments = 0;
        let mut invalid_vectors = 0;
        let mut digest = Sha256::new();
        digest.update(b"reliquary:semantic-source-evidence:v1\0");
        digest.update(owner_uuid);
        hash_text(&mut digest, &expected.conversation_id);
        hash_text(&mut digest, &expected.id);
        digest.update(expected.content_id.0);
        digest.update(generation.id.0);
        for (fragment, turn_count) in relevant {
            let Some(row) = set.fragment_ids.iter().position(|id| *id == fragment.id) else {
                return Ok(SemanticSourceReadiness::Pending);
            };
            digest.update(fragment.id.0);
            digest.update((row as u64).to_le_bytes());
            if turn_count != 1
                || fragment.start_node_id != expected.id
                || fragment.end_node_id != expected.id
            {
                excluded_mixed_fragments += 1;
                digest.update([0]);
                continue;
            }
            let bytes = packed
                .row(row)
                .ok_or_else(|| CvaError::Freshness("missing semantic source row".into()))?;
            if bytes.len() != profile.dimensions as usize * 4 {
                return Err(CvaError::Freshness("semantic source row width".into()));
            }
            let components: Vec<f32> = bytes
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("four bytes")))
                .collect();
            digest.update(bytes);
            if cosine_similarity(&components, &components).is_err() {
                invalid_vectors += 1;
                digest.update([1]);
                continue;
            }
            digest.update([2]);
            vectors.push(SemanticSourceVector {
                fragment_id: fragment.id,
                row: row as u64,
                components,
            });
        }
        let policy_digest = SemanticPolicy::default()
            .digest()
            .map_err(|error| CvaError::Freshness(error.to_string()))?;
        Ok(SemanticSourceReadiness::Ready(AcceptedSemanticSource {
            owner_uuid,
            node: expected.clone(),
            activity_position,
            generation,
            base_policy: self.freshness.policy(),
            semantic_policy: SemanticPolicy::default(),
            policy_digest,
            source_digest: digest.finalize().into(),
            vectors,
            excluded_mixed_fragments,
            invalid_vectors,
        }))
    }
}

impl Cva {
    /// Apply one accepted-turn semantic event using the newest retained vector
    /// generation. Missing or incomplete vector evidence is a no-op; the
    /// accepted Archive turn remains authoritative and can be retried later.
    pub(crate) fn apply_accepted_semantic_reinforcement(
        &mut self,
        node: &crate::Node,
        source_memory_cohort: &std::collections::BTreeSet<crate::MemoryId>,
    ) -> Result<usize, CvaError> {
        let Some(generation) = self
            .vector_generations
            .generations()
            .iter()
            .max_by_key(|generation| generation.vector_version)
            .copied()
        else {
            return Ok(0);
        };
        let source = match self.prepare_accepted_semantic_source(node, Some(generation.id))? {
            SemanticSourceReadiness::Pending => return Ok(0),
            SemanticSourceReadiness::Ready(source) => source,
        };
        let search = self.search_eligible_semantic_targets(&source, source_memory_cohort)?;
        let effects = search
            .effects
            .iter()
            .map(|effect| (effect.memory_id, effect.bonus))
            .collect::<std::collections::BTreeMap<_, _>>();
        let evidence = crate::freshness_storage::SemanticEventEvidence {
            owner_uuid: source.owner_uuid,
            conversation_id: source.node.conversation_id.clone(),
            node_id: source.node.id.clone(),
            accepted_turn: source.activity_position,
            base_policy_digest: freshness_policy_digest(source.base_policy),
            semantic_policy_version: source.semantic_policy.version,
            semantic_policy_digest: source.policy_digest,
            compatibility_profile_id: source.generation.compatibility_profile_id.0,
            source_evidence_digest: source.source_digest,
            target_evidence_digest: semantic_target_digest(&effects),
        };
        let payload = self
            .freshness
            .encode_semantic_event(source.owner_uuid, evidence, effects.clone())
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
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SemanticTargetSnapshot {
    pub memory: crate::Memory,
    pub body_id: crate::MemoryBodyId,
    pub admitted_at_turn: Option<u64>,
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
        let memory_ids = self.memories.current_ids();
        let vectors = crate::memory_retrieval_vectors::load_memory_vectors(
            &mut self.container,
            &self.memories,
            &self.memory_vectors,
            &self.packed_vectors,
            source.generation.compatibility_profile_id,
        )
        .map_err(|error| CvaError::Freshness(error.to_string()))?;
        let mut targets = Vec::new();
        let mut excluded_memories = 0;
        for id in &memory_ids {
            let memory = self
                .memories
                .memory(&mut self.container, *id)
                .map_err(|error| CvaError::Freshness(error.to_string()))?;
            let body_id = self
                .memories
                .current_body_id(*id)
                .map_err(|error| CvaError::Freshness(error.to_string()))?;
            let admitted_at_turn = self
                .freshness
                .records()
                .get(id)
                .and_then(|record| record.admitted_at_turn);
            let Some(vector) = vectors.get(&body_id).cloned() else {
                excluded_memories += 1;
                continue;
            };
            targets.push(SemanticTargetSnapshot {
                memory,
                body_id,
                admitted_at_turn,
                vector,
            });
        }
        let (effects, eligible_memories, filtered) =
            plan_eligible_semantic_targets(source, source_memory_cohort, targets)?;
        excluded_memories += filtered;
        Ok(SemanticCandidateSearch {
            effects,
            scanned_memories: memory_ids.len(),
            eligible_memories,
            excluded_memories,
        })
    }
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
            || target
                .memory
                .content_source_conversation_id
                .as_deref()
                .zip(target.memory.content_source_node_id.as_deref())
                .is_some_and(|(conversation, node)| {
                    conversation == source.node.conversation_id && node == source.node.id
                })
            || target
                .memory
                .grounding_source_conversation_id
                .as_deref()
                .zip(target.memory.grounding_source_node_id.as_deref())
                .is_some_and(|(conversation, node)| {
                    conversation == source.node.conversation_id && node == source.node.id
                });
        let admitted_before_source = target
            .admitted_at_turn
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
        eligible_memories += 1;
        for source_vector in &source.vectors {
            let cosine = cosine_similarity(&source_vector.components, &target.vector)
                .map_err(|error| CvaError::Freshness(error.to_string()))?;
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

fn hash_text(digest: &mut Sha256, text: &str) {
    digest.update((text.len() as u64).to_le_bytes());
    digest.update(text.as_bytes());
}

#[cfg(test)]
#[path = "cva_semantic_source_tests.rs"]
mod tests;
