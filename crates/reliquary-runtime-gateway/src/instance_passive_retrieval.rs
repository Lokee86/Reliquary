//! Authorized passive retrieval over the same owner cut used by agentic selection.
use super::*;
use crate::instance_observation_reads::{
    ObservationReadCut, observation_retrieval_candidates, observations_are_current,
};
use crate::instance_retrieval::preview::{MAX_RETRIEVAL_RESULTS, project_memories};
use crate::{RetrievalCut, RetrievalMemory};
use reliquary_application::{
    MemoryRetrievalConfig, MemoryRetrievalMode, MemoryRetrievalResult, OwnerMemorySelection,
    RetrievalConfig, TemporalEvidence, TemporalFilter, agentic_memories,
    observation_matches_temporal, rank_retrieval_candidates, route_retrieval_candidates,
};
use reliquary_owner_session::model::{CompatibilityProfile, CompatibilityProfileId};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    #[default]
    Lexical,
    Hybrid,
    Semantic,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SearchResult {
    pub cut: RetrievalCut,
    pub query: String,
    pub mode: SearchMode,
    pub semantic_routing: Option<&'static str>,
    pub fallback_reason: Option<&'static str>,
    pub memories_without_vectors: Option<usize>,
    pub truncated: bool,
    pub memories: Vec<RetrievalMemory>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub observations: Vec<RetrievalObservation>,
}

/// Evidence for a returned Memory revision, keyed by its result identifier.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemoryTemporalMatch {
    pub memory_id: String,
    pub evidence: Vec<TemporalEvidence>,
}

impl Instance {
    /// Reports whether this authorized owner has vector records under an
    /// active profile. The summary exposes no profile or vector data.
    pub fn has_active_vectors(&self, owner: OwnerId) -> Result<bool, GatewayError> {
        if self.rel_grants.contains(&owner) {
            let has_vectors = |rel: &RelSession| {
                let memory = rel.memory_vector_infos().into_iter().any(|info| {
                    rel.active_compatibility_profile(info.compatibility_profile_id)
                        .is_some()
                });
                memory
                    || rel.observation_claim_profiles().next().is_some()
                    || rel.has_active_archive_vectors()
            };
            match self.reliquary(owner) {
                Ok(rel) => Ok(has_vectors(&rel)),
                Err(GatewayError::WriterUnavailable) => {
                    self.with_writable_reliquary(owner, |rel| has_vectors(rel))
                }
                Err(error) => Err(error),
            }
        } else if self.phy_grants.contains(&owner) {
            self.phy_retrieval_embedding_profiles(owner)
                .map(|profiles| !profiles.is_empty())
        } else {
            self.reliquary(owner).map(|_| false)
        }
    }

    pub(crate) fn retrieval_snapshot(
        &self,
        owner: OwnerId,
        profile: Option<CompatibilityProfileId>,
    ) -> Result<reliquary_owner_session::RelRetrievalSnapshot, GatewayError> {
        self.retrieval_cut_snapshot(owner, profile, true)
    }

    pub(crate) fn retrieval_cut_snapshot(
        &self,
        owner: OwnerId,
        profile: Option<CompatibilityProfileId>,
        include_vectors: bool,
    ) -> Result<reliquary_owner_session::RelRetrievalSnapshot, GatewayError> {
        self.with_reliquary(owner, |rel| {
            capture_memory_snapshot(rel, profile, include_vectors)
        })?
    }

    /// Active, owner-qualified profiles with an existing Memory vector set.
    pub fn retrieval_embedding_profiles(
        &self,
        owner: OwnerId,
    ) -> Result<Vec<CompatibilityProfile>, GatewayError> {
        self.with_reliquary(owner, |rel| {
            let mut profiles = rel
                .memory_vector_infos()
                .into_iter()
                .map(|info| info.compatibility_profile_id)
                .filter(|id| rel.active_compatibility_profile(*id).is_some())
                .collect::<BTreeSet<_>>();
            if profiles.is_empty() {
                profiles.extend(rel.observation_claim_profiles());
            }
            profiles
                .into_iter()
                .filter_map(|id| rel.active_compatibility_profile(id).cloned())
                .collect()
        })
    }

    pub fn search_rel_memories(
        &self,
        owner: OwnerId,
        query: &str,
        mode: SearchMode,
        limit: Option<usize>,
        embedding: Option<(CompatibilityProfileId, Vec<f32>)>,
    ) -> Result<SearchResult, GatewayError> {
        self.search_rel_memories_with_policy(
            owner,
            query,
            mode,
            limit,
            embedding,
            RetrievalConfig::default(),
        )
    }

    pub fn search_rel_memories_with_policy(
        &self,
        owner: OwnerId,
        query: &str,
        mode: SearchMode,
        limit: Option<usize>,
        embedding: Option<(CompatibilityProfileId, Vec<f32>)>,
        policy: RetrievalConfig,
    ) -> Result<SearchResult, GatewayError> {
        self.search_rel_memories_with_filter(owner, query, mode, limit, embedding, policy, None)
            .map(|(result, _)| result)
    }

    pub fn search_rel_memories_with_filter(
        &self,
        owner: OwnerId,
        query: &str,
        mode: SearchMode,
        limit: Option<usize>,
        embedding: Option<(CompatibilityProfileId, Vec<f32>)>,
        mut policy: RetrievalConfig,
        filter: Option<TemporalFilter>,
    ) -> Result<(SearchResult, Vec<MemoryTemporalMatch>), GatewayError> {
        if query.trim().is_empty()
            || query.len() > 512
            || limit.is_some_and(|n| n == 0 || n > MAX_RETRIEVAL_RESULTS)
            || filter.is_some_and(|f| f.start_ns >= f.end_exclusive_ns)
        {
            return Err(GatewayError::InvalidPreparedRetrieval);
        }
        if (mode == SearchMode::Lexical) != embedding.is_none() {
            return Err(GatewayError::Retrieval(
                "compatible query embedding is required for this search mode".into(),
            ));
        }
        let profile = embedding.as_ref().map(|(id, _)| *id);
        let vector = embedding.as_ref().map(|(_, row)| row.as_slice());
        let (mut snapshot, removal_cut, observation_cut, mut observation_candidates) = self
            .with_reliquary(owner, |rel| -> Result<_, GatewayError> {
                Ok((
                    capture_memory_snapshot(rel, profile, true)?,
                    rel.archive_source_removal_version(),
                    ObservationReadCut::capture(rel),
                    observation_retrieval_candidates(rel, profile, &BTreeSet::new())?,
                ))
            })??;
        let has_vectors = !snapshot.vectors.is_empty()
            || observation_candidates
                .values()
                .any(|row| row.input.candidate.vector.is_some());
        if filter.is_none() && vector.is_some() && !has_vectors {
            return Err(GatewayError::RetrievalUnavailable(
                "no current Memory vectors for the embedding profile",
            ));
        }
        if let Some(filter) = &filter {
            observation_candidates.retain(|_, row| {
                row.input
                    .validity
                    .is_some_and(|validity| observation_matches_temporal(validity, filter))
            });
        }
        let mut matches: BTreeMap<_, _> = filter
            .map(|filter| reliquary_application::retain_temporal_memories(&mut snapshot, &filter))
            .into_iter()
            .flatten()
            .map(|(id, evidence)| (hex(&id.0), evidence))
            .collect();
        let missing = vector.map(|_| {
            snapshot
                .memories
                .iter()
                .filter(|memory| {
                    !snapshot
                        .vectors
                        .contains_key(&memory.revision.record.body_id)
                })
                .count()
        });
        let mut semantic_routing = None;
        let mut fallback_reason = None;
        let candidates = agentic_memories(&snapshot)
            .into_iter()
            .chain(
                observation_candidates
                    .values()
                    .map(|row| row.input.candidate.clone()),
            )
            .collect::<Vec<_>>();
        let mut observations = Vec::new();
        let mut memories = Vec::new();
        let more = if mode == SearchMode::Semantic {
            semantic_routing = Some("global_exact");
            let mut config = MemoryRetrievalConfig::default();
            let limit = limit.unwrap_or(config.seed_limit);
            config.seed_limit = limit + 1;
            if candidates.iter().any(|row| row.vector.is_some()) {
                let result =
                    route_retrieval_candidates(&snapshot, candidates, vector.unwrap(), config)
                        .map_err(GatewayError::Retrieval)?;
                if result.mode_used == MemoryRetrievalMode::CommunityRouted {
                    semantic_routing = Some("community_routed");
                }
                if result.used_global_fallback {
                    fallback_reason =
                        Some("community routing unavailable for this graph and vector cut");
                }
                let mut memory_hits = Vec::new();
                for hit in result.seeds.iter().take(limit) {
                    if let Some(row) = observation_candidates.remove(&hit.memory_id) {
                        let mut observation = row.result;
                        observation.semantic_score = Some(hit.score);
                        observations.push(observation);
                    } else {
                        memory_hits.push(hit);
                    }
                }
                memories =
                    project_memories(&snapshot, memory_hits.iter().map(|hit| hit.memory_id))?;
                for (memory, hit) in memories.iter_mut().zip(memory_hits) {
                    memory.semantic_score = Some(hit.score);
                }
                result.seeds.len() > limit
            } else {
                false
            }
        } else {
            let limit = limit.unwrap_or(policy.result_limit.min(MAX_RETRIEVAL_RESULTS));
            policy.result_limit = limit + 1;
            policy.candidate_limit = policy.candidate_limit.max(policy.result_limit);
            let ranked =
                rank_retrieval_candidates(snapshot.owner_id, candidates, query, vector, policy)
                    .map_err(GatewayError::Retrieval)?;
            let mut memory_hits = Vec::new();
            for hit in ranked.selected.iter().take(limit) {
                if let Some(row) = observation_candidates.remove(&hit.memory_id) {
                    let mut observation = row.result;
                    observation.lexical_score = hit.lexical_score.unwrap_or_default();
                    observation.semantic_score = hit.semantic_score;
                    observation.combined_score = Some(hit.combined_score);
                    observations.push(observation);
                } else {
                    memory_hits.push(hit);
                }
            }
            memories = project_memories(&snapshot, memory_hits.iter().map(|hit| hit.memory_id))?;
            for (memory, hit) in memories.iter_mut().zip(memory_hits) {
                memory.lexical_score = hit.lexical_score;
                memory.semantic_score = hit.semantic_score;
                memory.combined_score = Some(hit.combined_score);
            }
            ranked.selected.len() > limit
        };
        let current = self.with_reliquary(owner, |rel| {
            rel.archive_source_removal_version() == removal_cut
                && (observations.is_empty()
                    || (ObservationReadCut::capture(rel) == observation_cut
                        && observations_are_current(rel, &observations)))
        })?;
        if !current {
            return Err(GatewayError::StaleRetrieval);
        }
        self.ensure_credential_active()?;
        let truncated = more || memories.iter().any(|memory| memory.truncated);
        let evidence = memories
            .iter()
            .filter_map(|memory| {
                Some(MemoryTemporalMatch {
                    evidence: matches.remove(&memory.memory_id)?,
                    memory_id: memory.memory_id.clone(),
                })
            })
            .collect();
        Ok((
            SearchResult {
                cut: RetrievalCut::capture(&snapshot, profile, observation_cut.publication),
                query: query.to_owned(),
                mode,
                semantic_routing,
                fallback_reason,
                memories_without_vectors: missing,
                truncated,
                memories,
                observations,
            },
            evidence,
        ))
    }

    pub fn rank_rel_memories(
        &self,
        owner: OwnerId,
        query: &str,
        profile: Option<CompatibilityProfileId>,
        vector: Option<&[f32]>,
        config: RetrievalConfig,
    ) -> Result<OwnerMemorySelection, GatewayError> {
        let snapshot = self.retrieval_snapshot(owner, profile)?;
        let result = reliquary_application::rank_owner_snapshot(&snapshot, query, vector, config)
            .map_err(GatewayError::Retrieval)?;
        self.ensure_credential_active()?;
        Ok(result)
    }

    pub fn route_rel_memories(
        &self,
        owner: OwnerId,
        profile: CompatibilityProfileId,
        vector: &[f32],
        config: MemoryRetrievalConfig,
    ) -> Result<MemoryRetrievalResult, GatewayError> {
        let snapshot = self.retrieval_snapshot(owner, Some(profile))?;
        let result = reliquary_application::route_owner_snapshot(&snapshot, vector, config)
            .map_err(GatewayError::Retrieval)?;
        self.ensure_credential_active()?;
        Ok(result)
    }
}

pub(crate) fn capture_memory_snapshot(
    rel: &RelSession,
    profile: Option<CompatibilityProfileId>,
    include_vectors: bool,
) -> Result<reliquary_owner_session::RelRetrievalSnapshot, GatewayError> {
    if profile.is_some_and(|id| rel.active_compatibility_profile(id).is_none()) {
        return Err(GatewayError::RetrievalUnavailable(
            "embedding profile is unavailable",
        ));
    }
    Ok(rel.retrieval_snapshot(if include_vectors { profile } else { None })?)
}
