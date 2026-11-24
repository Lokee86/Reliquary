//! Authorized passive retrieval over the same owner cut used by agentic selection.
use super::*;
use crate::instance_retrieval::preview::{MAX_RETRIEVAL_RESULTS, project_memories};
use crate::{RetrievalCut, RetrievalMemory};
use reliquary_application::{
    MemoryRetrievalConfig, MemoryRetrievalMode, MemoryRetrievalResult, OwnerMemorySelection,
    RetrievalConfig, TemporalEvidence, TemporalFilter,
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
                memory || rel.has_active_archive_vectors()
            };
            match self.reliquary(owner) {
                Ok(rel) => Ok(has_vectors(&rel)),
                Err(GatewayError::WriterUnavailable) => {
                    self.with_writable_reliquary(owner, |rel| has_vectors(rel))
                }
                Err(error) => Err(error),
            }
        } else if self.phy_grants.contains(&owner) {
            let has_vectors = |phy: &PhySession| {
                phy.memory_vector_infos().into_iter().any(|info| {
                    phy.active_compatibility_profile(info.compatibility_profile_id)
                        .is_some()
                })
            };
            match self.phylactery(owner) {
                Ok(phy) => Ok(has_vectors(&phy)),
                Err(GatewayError::WriterUnavailable) => {
                    self.with_entity_phy_writer(owner, |writer| {
                        writer.memory_vector_infos().into_iter().any(|info| {
                            writer
                                .active_compatibility_profile(info.compatibility_profile_id)
                                .is_some()
                        })
                    })
                }
                Err(error) => Err(error),
            }
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
            if profile.is_some_and(|id| rel.active_compatibility_profile(id).is_none()) {
                return Err(GatewayError::RetrievalUnavailable(
                    "embedding profile is unavailable",
                ));
            }
            Ok(rel.retrieval_snapshot(if include_vectors { profile } else { None })?)
        })?
    }

    /// Active, owner-qualified profiles with an existing Memory vector set.
    pub fn retrieval_embedding_profiles(
        &self,
        owner: OwnerId,
    ) -> Result<Vec<CompatibilityProfile>, GatewayError> {
        self.with_reliquary(owner, |rel| {
            rel.memory_vector_infos()
                .into_iter()
                .map(|info| info.compatibility_profile_id)
                .collect::<BTreeSet<_>>()
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
        let mut snapshot = self.retrieval_snapshot(owner, profile)?;
        if filter.is_none() && vector.is_some() && snapshot.vectors.is_empty() {
            return Err(GatewayError::RetrievalUnavailable(
                "no current Memory vectors for the embedding profile",
            ));
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
        let (memories, more) = if mode == SearchMode::Semantic && snapshot.vectors.is_empty() {
            semantic_routing = Some("global_exact");
            (Vec::new(), false)
        } else if mode == SearchMode::Semantic {
            let mut config = MemoryRetrievalConfig::default();
            let limit = limit.unwrap_or(config.seed_limit);
            config.seed_limit = limit + 1;
            let result =
                reliquary_application::route_owner_snapshot(&snapshot, vector.unwrap(), config)
                    .map_err(GatewayError::Retrieval)?;
            semantic_routing = Some(if result.mode_used == MemoryRetrievalMode::GlobalExact {
                "global_exact"
            } else {
                "community_routed"
            });
            if result.used_global_fallback {
                fallback_reason =
                    Some("community routing unavailable for this graph and vector cut");
            }
            let mut memories =
                project_memories(&snapshot, result.memories.iter().copied().take(limit))?;
            for (memory, hit) in memories.iter_mut().zip(&result.seeds) {
                memory.semantic_score = Some(hit.score);
            }
            (memories, result.memories.len() > limit)
        } else {
            let limit = limit.unwrap_or(policy.result_limit.min(MAX_RETRIEVAL_RESULTS));
            policy.result_limit = limit + 1;
            policy.candidate_limit = policy.candidate_limit.max(policy.result_limit);
            let ranked =
                reliquary_application::rank_owner_snapshot(&snapshot, query, vector, policy)
                    .map_err(GatewayError::Retrieval)?;
            let mut memories = project_memories(
                &snapshot,
                ranked.selected.iter().take(limit).map(|hit| hit.memory_id),
            )?;
            for (memory, hit) in memories.iter_mut().zip(&ranked.selected) {
                memory.lexical_score = hit.lexical_score;
                memory.semantic_score = hit.semantic_score;
                memory.combined_score = Some(hit.combined_score);
            }
            (memories, ranked.selected.len() > limit)
        };
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
                cut: RetrievalCut::capture(&snapshot, profile),
                query: query.to_owned(),
                mode,
                semantic_routing,
                fallback_reason,
                memories_without_vectors: missing,
                truncated,
                memories,
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
