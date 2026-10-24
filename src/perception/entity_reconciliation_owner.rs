use crate::entity_reconciliation_support::exact_entity_for_surface;
use crate::entity_resolution_owner::ResolutionWriteContext;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::{Container, EntityReconciliationError};

pub(crate) struct RejectionReconsiderationContext<'a> {
    pub(crate) container: &'a mut Container,
    pub(crate) memories: &'a MemoryStore,
    pub(crate) entities: &'a mut EntityStore,
    pub(crate) graph: &'a mut GraphStore,
    pub(crate) resolutions: &'a mut EntityResolutionStore,
    pub(crate) lexical_index: &'a mut crate::lexical_index::LexicalIndex,
    pub(crate) archive: Option<&'a crate::Archive>,
}

pub(crate) fn reconsider_late_exact_rejections<E: crate::GeneralEndpoint>(
    context: &mut RejectionReconsiderationContext<'_>,
    endpoint: &E,
    timestamp: &mut i64,
) -> Result<usize, EntityReconciliationError> {
    let states = context.resolutions.records();
    let engine = crate::EntityResolutionEngine::new(endpoint);
    let mut changed = 0usize;
    let entities = context.entities.current();
    for state in states {
        let crate::MemoryEntityResolutionStatus::Rejected { reason } = state.status else {
            continue;
        };
        let Some(metadata) = context.memories.routing_metadata(state.key.memory_id) else {
            continue;
        };
        let Some(mention) = metadata.entity_mentions.iter().find(|mention| {
            mention.field == state.key.field
                && mention.start_byte == state.key.start_byte
                && mention.end_byte == state.key.end_byte
        }) else {
            continue;
        };
        let Some(entity) = exact_entity_for_surface(&entities, &mention.text) else {
            continue;
        };
        if entity.created_at_ns <= state.updated_at_ns {
            continue;
        }

        *timestamp = timestamp.saturating_add(1);
        {
            let mut writer = ResolutionWriteContext {
                container: context.container,
                memories: context.memories,
                entities: context.entities,
                entity_resolutions: context.resolutions,
            };
            writer
                .reset_entity_resolution(state.key, *timestamp)
                .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
        }
        *timestamp = timestamp.saturating_add(1);
        let archive = context.archive;
        let result = (|| -> Result<crate::EntityResolutionOutcome, crate::EntityResolverError> {
            let principal_outcome = {
                let mut principal = crate::entity_principal::PrincipalContext {
                    container: context.container,
                    memories: context.memories,
                    entities: context.entities,
                    entity_resolutions: context.resolutions,
                    graph: context.graph,
                    archive,
                };
                if archive.is_some() {
                    principal.resolve_principal_entity_mention(state.key, *timestamp)?
                } else {
                    principal.reject_phy_principal_entity_mention(state.key, *timestamp)?
                }
            };
            if let Some(outcome) = principal_outcome {
                return Ok(outcome);
            }

            let owner_id = context.container.owner_id();
            let prepared = {
                let mut preparation = crate::entity_resolution_prepare::EntityResolutionContext {
                    candidates: crate::entity_candidates::EntityCandidateContext {
                        memories: context.memories,
                        lexical_index: context.lexical_index,
                        container: context.container,
                        entities: context.entities,
                        graph: context.graph,
                    },
                    resolutions: context.resolutions,
                };
                crate::entity_resolution_prepare::prepare_entity_resolution(
                    &mut preparation,
                    state.key,
                    crate::EntityCandidateConfig::default(),
                    owner_id,
                )?
            };
            match prepared {
                crate::EntityResolutionPreparation::Complete(outcome) => Ok(outcome),
                crate::EntityResolutionPreparation::Ready(prepared) => {
                    let evaluation = engine.evaluate_prepared(&prepared)?;
                    let mut commit =
                        crate::entity_resolution_commit::EntityResolutionCommitContext {
                            container: context.container,
                            memories: context.memories,
                            entities: context.entities,
                            graph: context.graph,
                            resolutions: context.resolutions,
                        };
                    crate::entity_resolution_commit::commit_entity_resolution(
                        &mut commit,
                        prepared,
                        evaluation,
                        *timestamp,
                    )?
                    .ok_or_else(crate::entity_resolution_processor::stale_snapshot)
                }
            }
        })();
        match result {
            Ok(_) => changed += 1,
            Err(error) => {
                *timestamp = timestamp.saturating_add(1);
                let mut writer = ResolutionWriteContext {
                    container: context.container,
                    memories: context.memories,
                    entities: context.entities,
                    entity_resolutions: context.resolutions,
                };
                writer
                    .put_entity_rejected(state.key, 0, reason, *timestamp)
                    .map_err(|restore| {
                        EntityReconciliationError::Operation(format!(
                            "resolver failed ({error}); failed to restore rejection: {restore}"
                        ))
                    })?;
                return Err(error.into());
            }
        }
    }
    Ok(changed)
}
