use super::{
    ReliquaryRuntimeHost, ReliquaryRuntimeHostError, operation,
    semantic_access::RuntimeSemanticOwnerKind,
};
use crate::compatibility_profile_probe::profile_from_endpoint;
use crate::{
    EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, MemoryId, MemoryRetrievalConfig,
    MemoryRetrievalResult, VectorNormalization,
};
use std::collections::HashMap;
use std::sync::Arc;

pub use crate::runtime_shared::{
    DEFAULT_MEMORY_SEARCH_DEPTH, DEFAULT_MEMORY_SEARCH_RESULTS, MAX_MEMORY_SEARCH_DEPTH,
    MAX_MEMORY_SEARCH_QUERY_BYTES, MAX_MEMORY_SEARCH_RESULTS, MemorySearchItem, MemorySearchLane,
    MemorySearchResult, VisibleMemorySearchOwner, VisibleMemorySearchResult,
};

impl ReliquaryRuntimeHost {
    pub fn search_visible_memories(
        &self,
        query: &str,
        limit: usize,
        depth: usize,
    ) -> Result<VisibleMemorySearchResult, ReliquaryRuntimeHostError> {
        let query = query.trim();
        if query.is_empty() || query.len() > MAX_MEMORY_SEARCH_QUERY_BYTES {
            return Err(operation(format!(
                "memory search requires a query of at most {MAX_MEMORY_SEARCH_QUERY_BYTES} bytes"
            )));
        }
        let limit = if limit == 0 {
            DEFAULT_MEMORY_SEARCH_RESULTS
        } else {
            limit
        };
        if !(1..=MAX_MEMORY_SEARCH_RESULTS).contains(&limit) {
            return Err(operation(format!(
                "memory search limit must be between 1 and {MAX_MEMORY_SEARCH_RESULTS}"
            )));
        }
        if !(1..=MAX_MEMORY_SEARCH_DEPTH).contains(&depth) {
            return Err(operation(format!(
                "memory search depth must be between 1 and {MAX_MEMORY_SEARCH_DEPTH}"
            )));
        }
        let endpoint = self
            .embedding_route()?
            .ok_or_else(|| operation("memory search requires an embedding route"))?;
        let vectors = endpoint
            .embed(EmbeddingMode::Query, &[query.to_owned()])
            .map_err(operation)?;
        let query_vector = vectors
            .into_iter()
            .next()
            .ok_or_else(|| operation("embedding route returned no query vector"))?;
        let config = MemoryRetrievalConfig {
            max_depth: depth,
            ..MemoryRetrievalConfig::default()
        };
        let owners = self.visible_semantic_owners()?;

        std::thread::scope(|scope| {
            let workers = owners
                .into_iter()
                .map(|owner| {
                    let query_vector = &query_vector;
                    (
                        owner.clone(),
                        scope.spawn(move || match owner.kind {
                            RuntimeSemanticOwnerKind::Reliquary => self
                                .retrieve_reliquary_memories_for(
                                    &owner.owner_id,
                                    query_vector,
                                    config,
                                )
                                .map(Some),
                            RuntimeSemanticOwnerKind::Phylactery => {
                                self.retrieve_phylactery_memories(query_vector, config)
                            }
                        }),
                    )
                })
                .collect::<Vec<_>>();
            let mut results = Vec::with_capacity(workers.len());
            for (owner, worker) in workers {
                match worker.join() {
                    Ok(Ok(lane)) => results.push(VisibleMemorySearchOwner {
                        owner,
                        lane: lane.map(|mut lane| {
                            lane.items.truncate(limit);
                            lane
                        }),
                        error: None,
                    }),
                    Ok(Err(error)) => results.push(VisibleMemorySearchOwner {
                        owner,
                        lane: None,
                        error: Some(error.to_string()),
                    }),
                    Err(_) => results.push(VisibleMemorySearchOwner {
                        owner,
                        lane: None,
                        error: Some("memory search worker panicked".into()),
                    }),
                }
            }
            Ok(VisibleMemorySearchResult {
                owners: results,
                traversal_depth: depth,
            })
        })
    }

    pub fn retrieve_reliquary_memories(
        &self,
        query_vector: &[f32],
        config: MemoryRetrievalConfig,
    ) -> Result<MemorySearchLane, ReliquaryRuntimeHostError> {
        let owner_id = self.active_rel_id().ok_or_else(|| {
            ReliquaryRuntimeHostError::Operation("Reliquary runtime has no active REL".into())
        })?;
        self.retrieve_reliquary_memories_for(&owner_id, query_vector, config)
    }

    pub fn retrieve_reliquary_memories_for(
        &self,
        owner_id: &str,
        query_vector: &[f32],
        config: MemoryRetrievalConfig,
    ) -> Result<MemorySearchLane, ReliquaryRuntimeHostError> {
        let profile = self.ensure_reliquary_embedding_profile_for(owner_id)?;
        let runtime = self.runtime_for_owner(owner_id)?;
        let mut runtime = runtime
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let owner_id = runtime.cva.owner_id().ok_or_else(|| {
            ReliquaryRuntimeHostError::Operation(
                "Reliquary does not have durable owner identity".into(),
            )
        })?;
        let retrieval = runtime
            .cva
            .retrieve_memories(profile, query_vector, config)
            .map_err(operation)?;
        hydrate_reliquary(&mut runtime.cva, owner_id, retrieval)
    }

    pub fn retrieve_phylactery_memories(
        &self,
        query_vector: &[f32],
        config: MemoryRetrievalConfig,
    ) -> Result<Option<MemorySearchLane>, ReliquaryRuntimeHostError> {
        if !self.has_phylactery()? {
            return Ok(None);
        }
        let profile = self.ensure_phylactery_memory_profile()?;
        let phylactery = Arc::clone(&self.active_execution()?.phylactery);
        let mut phylactery = phylactery
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let phy = phylactery.as_mut().ok_or_else(|| {
            ReliquaryRuntimeHostError::Operation("Phylactery is unavailable".into())
        })?;
        let owner_id = phy.owner_id().ok_or_else(|| {
            ReliquaryRuntimeHostError::Operation(
                "Phylactery does not have durable owner identity".into(),
            )
        })?;
        let retrieval = phy
            .retrieve_memories(profile, query_vector, config)
            .map_err(operation)?;
        hydrate_phylactery(phy, owner_id, retrieval).map(Some)
    }

    pub(crate) fn ensure_reliquary_embedding_profile(
        &self,
    ) -> Result<crate::CompatibilityProfileId, ReliquaryRuntimeHostError> {
        let owner_id = self.active_rel_id().ok_or_else(|| {
            ReliquaryRuntimeHostError::Operation("Reliquary runtime has no active REL".into())
        })?;
        self.ensure_reliquary_embedding_profile_for(&owner_id)
    }

    pub(super) fn ensure_reliquary_embedding_profile_for(
        &self,
        owner_id: &str,
    ) -> Result<crate::CompatibilityProfileId, ReliquaryRuntimeHostError> {
        if let Some(profile) = self
            .execution_for(owner_id)?
            .memory_profiles
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .project
        {
            return Ok(profile);
        }
        let candidate = self.runtime_profile_candidate()?;
        let runtime = self.runtime_for_owner(owner_id)?;
        let profile = runtime
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .cva
            .accept_runtime_compatibility_profile(candidate)
            .map_err(operation)?;
        self.execution_for(owner_id)?
            .memory_profiles
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .project = Some(profile.id);
        Ok(profile.id)
    }

    fn ensure_phylactery_memory_profile(
        &self,
    ) -> Result<crate::CompatibilityProfileId, ReliquaryRuntimeHostError> {
        if let Some(profile) = self
            .active_execution()?
            .memory_profiles
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .user
        {
            return Ok(profile);
        }
        let candidate = self.runtime_profile_candidate()?;
        let mut phylactery = self
            .active_execution()?
            .phylactery
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let profile = phylactery
            .as_mut()
            .ok_or_else(|| {
                ReliquaryRuntimeHostError::Operation("Phylactery is unavailable".into())
            })?
            .accept_runtime_compatibility_profile(candidate)
            .map_err(operation)?;
        self.active_execution()?
            .memory_profiles
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .user = Some(profile.id);
        Ok(profile.id)
    }

    fn runtime_profile_candidate(
        &self,
    ) -> Result<crate::CompatibilityProfile, ReliquaryRuntimeHostError> {
        let endpoint = self.embedding_route()?.ok_or_else(|| {
            ReliquaryRuntimeHostError::Operation(
                "memory retrieval requires an embedding route".into(),
            )
        })?;
        profile_from_endpoint(&SharedEmbeddingEndpoint(endpoint)).map_err(operation)
    }

    pub fn retrieve_memory_pair(
        &self,
        query_vector: &[f32],
        config: MemoryRetrievalConfig,
    ) -> Result<MemorySearchResult, ReliquaryRuntimeHostError> {
        std::thread::scope(|scope| {
            let rel = scope.spawn(|| self.retrieve_reliquary_memories(query_vector, config));
            let phy = scope.spawn(|| self.retrieve_phylactery_memories(query_vector, config));
            Ok(MemorySearchResult {
                reliquary: rel
                    .join()
                    .map_err(|_| ReliquaryRuntimeHostError::ThreadPanicked)??,
                phylactery: phy
                    .join()
                    .map_err(|_| ReliquaryRuntimeHostError::ThreadPanicked)??,
            })
        })
    }
}

#[derive(Clone)]
struct SharedEmbeddingEndpoint(Arc<dyn EmbeddingEndpoint + Send + Sync>);

impl EmbeddingEndpoint for SharedEmbeddingEndpoint {
    fn dimensions(&self) -> u32 {
        self.0.dimensions()
    }

    fn normalization(&self) -> VectorNormalization {
        self.0.normalization()
    }

    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.0.embed(mode, inputs)
    }
}

fn hydrate_reliquary(
    cva: &mut crate::Cva,
    owner_id: String,
    retrieval: MemoryRetrievalResult,
) -> Result<MemorySearchLane, ReliquaryRuntimeHostError> {
    let scores = seed_scores(&retrieval);
    let items = retrieval
        .memories
        .iter()
        .map(|id| {
            cva.memory(*id)
                .map(|memory| MemorySearchItem {
                    memory,
                    seed_score: scores.get(id).copied(),
                })
                .map_err(operation)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MemorySearchLane {
        owner_id,
        retrieval,
        items,
    })
}

fn hydrate_phylactery(
    phylactery: &mut crate::Phylactery,
    owner_id: String,
    retrieval: MemoryRetrievalResult,
) -> Result<MemorySearchLane, ReliquaryRuntimeHostError> {
    let scores = seed_scores(&retrieval);
    let items = retrieval
        .memories
        .iter()
        .map(|id| {
            phylactery
                .memory(*id)
                .map(|memory| MemorySearchItem {
                    memory,
                    seed_score: scores.get(id).copied(),
                })
                .map_err(operation)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MemorySearchLane {
        owner_id,
        retrieval,
        items,
    })
}

fn seed_scores(retrieval: &MemoryRetrievalResult) -> HashMap<MemoryId, f64> {
    retrieval
        .seeds
        .iter()
        .map(|hit| (hit.memory_id, hit.score))
        .collect()
}
