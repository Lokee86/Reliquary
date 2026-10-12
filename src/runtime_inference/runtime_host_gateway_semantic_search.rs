//! Instance-authorized semantic search across the selected REL's granted closure and bound PHY.
use super::*;
use crate::{
    EmbeddingMode, MemoryId, MemoryRetrievalConfig, MemoryRetrievalResult, RuntimeSemanticOwner,
    RuntimeSemanticOwnerKind,
};
use crate::{
    MemorySearchItem, MemorySearchLane, VisibleMemorySearchOwner, VisibleMemorySearchResult,
};
use std::collections::HashMap;

/// Archive hits visible through one instance's authorized REL set.
#[derive(Clone, Debug, PartialEq)]
pub struct VisibleArchiveSearchOwner {
    pub owner: RuntimeSemanticOwner,
    pub hits: Vec<crate::ConversationSearchHit>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VisibleArchiveSearchResult {
    pub owners: Vec<VisibleArchiveSearchOwner>,
}

struct AuthorizedLane {
    owner: RuntimeSemanticOwner,
    token: OwnerAccessToken,
    profile: Option<crate::CompatibilityProfileId>,
}

/// Uses the existing owner-local profiles only. This path never creates or mutates profiles.
impl GatewayRuntime {
    /// Search the instance-selected REL's independently granted dependency closure
    /// and, when authorized, only that instance's exact bound PHY.
    pub fn search_visible_memories(
        &self,
        instance: &InstanceRuntime,
        query: &str,
        limit: usize,
        depth: usize,
    ) -> Result<VisibleMemorySearchResult, ReliquaryRuntimeHostError> {
        let query = query.trim();
        if query.is_empty() || query.len() > crate::MAX_MEMORY_SEARCH_QUERY_BYTES {
            return Err(operation(format!(
                "memory search requires a query of at most {} bytes",
                crate::MAX_MEMORY_SEARCH_QUERY_BYTES
            )));
        }
        let limit = if limit == 0 {
            crate::DEFAULT_MEMORY_SEARCH_RESULTS
        } else {
            limit
        };
        if !(1..=crate::MAX_MEMORY_SEARCH_RESULTS).contains(&limit) {
            return Err(operation(format!(
                "memory search limit must be between 1 and {}",
                crate::MAX_MEMORY_SEARCH_RESULTS
            )));
        }
        if !(1..=crate::MAX_MEMORY_SEARCH_DEPTH).contains(&depth) {
            return Err(operation(format!(
                "memory search depth must be between 1 and {}",
                crate::MAX_MEMORY_SEARCH_DEPTH
            )));
        }

        // Freeze the attachment and owner authority before external embedding I/O.
        let authorization = instance.authorization().map_err(operation)?;
        let grants = instance.grants_snapshot().map_err(operation)?;
        let mut lanes = Vec::new();
        if let Some(root) = self.active_rel_id(instance)? {
            for owner_id in self.dependency_closure_for(instance, &root)? {
                let token = self.authorize(instance, &owner_id, OwnerGrant::ViewRel)?;
                let profile = {
                    let rels = self
                        .rel_executions
                        .read()
                        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                    let execution = rels
                        .get(&owner_id)
                        .ok_or_else(|| operation("granted REL owner is not mounted"))?;
                    let runtime = Arc::clone(&execution.owner);
                    drop(rels);
                    let runtime = runtime
                        .lock()
                        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                    let profile = *runtime
                        .memory_profile
                        .lock()
                        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                    profile
                };
                lanes.push(AuthorizedLane {
                    owner: RuntimeSemanticOwner {
                        owner_id,
                        kind: RuntimeSemanticOwnerKind::Reliquary,
                    },
                    token,
                    profile,
                });
            }
        }
        if let Some(owner_id) = instance.attached_phy_owner_id().filter(|owner_id| {
            grants
                .get(*owner_id)
                .is_some_and(|caps| caps.contains(&OwnerGrant::UseOwnPhy))
        }) {
            let token = self.authorize(instance, owner_id, OwnerGrant::UseOwnPhy)?;
            let profile = {
                let phys = self
                    .phy_executions
                    .read()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                let execution = phys
                    .get(owner_id)
                    .ok_or_else(|| operation("bound PHY owner is not mounted"))?;
                let runtime = Arc::clone(&execution.runtime);
                drop(phys);
                let profile = runtime
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .memory_profile()?;
                profile
            };
            lanes.push(AuthorizedLane {
                owner: RuntimeSemanticOwner {
                    owner_id: owner_id.to_owned(),
                    kind: RuntimeSemanticOwnerKind::Phylactery,
                },
                token,
                profile,
            });
        }

        let endpoint = self
            .routes
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .embedding()
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

        let results = std::thread::scope(|scope| {
            let workers = lanes
                .iter()
                .map(|lane| {
                    let vector = &query_vector;
                    scope.spawn(move || {
                        let Some(profile) = lane.profile else {
                            return Err(operation(
                                "owner has no established memory compatibility profile",
                            ));
                        };
                        let result = match lane.token.kind {
                            OwnerKind::Rel => self.with_rel(&lane.token, |runtime| {
                                let owner_id = runtime.cva.owner_id().ok_or_else(|| {
                                    operation("Reliquary does not have durable owner identity")
                                })?;
                                let retrieval = runtime
                                    .cva
                                    .retrieve_memories(profile, vector, config)
                                    .map_err(operation)?;
                                hydrate_lane_reliquary(&mut runtime.cva, owner_id, retrieval)
                            }),
                            OwnerKind::Phy => self.with_phy(&lane.token, |owner| {
                                let owner_id = owner.owner_id().ok_or_else(|| {
                                    operation("Phylactery does not have durable owner identity")
                                })?;
                                let retrieval = owner
                                    .retrieve_memories(profile, vector, config)
                                    .map_err(operation)?;
                                hydrate_lane_phylactery(owner, owner_id, retrieval)
                            }),
                        }?;
                        Ok(result)
                    })
                })
                .collect::<Vec<_>>();
            workers
                .into_iter()
                .map(|worker| match worker.join() {
                    Ok(result) => result,
                    Err(_) => Err(operation("semantic search worker panicked")),
                })
                .collect::<Vec<_>>()
        });

        // Revalidate every original capability after all I/O and before exposing any
        // lane. Holding the attachment lease closes the final revoke/detach race.
        let attachment = self.attachment_entry(instance)?;
        let _delivery = attachment.leases.acquire_current()?;
        if instance.authorization().map_err(operation)? != authorization {
            return Err(operation(
                "instance authorization changed before semantic search delivery",
            ));
        }
        for lane in &lanes {
            match lane.token.kind {
                OwnerKind::Rel => self.with_rel(&lane.token, |_| Ok(()))?,
                OwnerKind::Phy => self.with_phy(&lane.token, |_| Ok(()))?,
            }
        }
        if instance.authorization().map_err(operation)? != authorization {
            return Err(operation(
                "instance authorization changed before semantic search delivery",
            ));
        }

        let owners = lanes
            .into_iter()
            .zip(results)
            .map(|(lane, result)| match result {
                Ok(mut lane_result) => {
                    lane_result.items.truncate(limit);
                    VisibleMemorySearchOwner {
                        owner: lane.owner,
                        lane: Some(lane_result),
                        error: None,
                    }
                }
                Err(error) => VisibleMemorySearchOwner {
                    owner: lane.owner,
                    lane: None,
                    error: Some(error.to_string()),
                },
            })
            .collect();
        Ok(VisibleMemorySearchResult {
            owners,
            traversal_depth: depth,
        })
    }
}

fn hydrate_lane_reliquary(
    cva: &mut crate::Cva,
    owner_id: String,
    retrieval: MemoryRetrievalResult,
) -> Result<MemorySearchLane, ReliquaryRuntimeHostError> {
    let scores = retrieval
        .seeds
        .iter()
        .map(|hit| (hit.memory_id, hit.score))
        .collect::<HashMap<MemoryId, f64>>();
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

fn hydrate_lane_phylactery(
    phy: &mut crate::Phylactery,
    owner_id: String,
    retrieval: MemoryRetrievalResult,
) -> Result<MemorySearchLane, ReliquaryRuntimeHostError> {
    let scores = retrieval
        .seeds
        .iter()
        .map(|hit| (hit.memory_id, hit.score))
        .collect::<HashMap<MemoryId, f64>>();
    let items = retrieval
        .memories
        .iter()
        .map(|id| {
            phy.memory(*id)
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
