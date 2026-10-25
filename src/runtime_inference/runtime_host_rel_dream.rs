use super::{
    DREAM_RETRY_POLL, RelShared, ReliquaryRuntimeHostError, notify_work, operation, stopped,
    wait_for_work,
};
use crate::runtime_shared::unix_now_ns;
use crate::{
    CompatibilityProfileId, DreamCandidateConfig, DreamCandidateSet, DreamPairClassification,
    DreamPairVerification, DreamProcessError, DreamProcessor, DreamVerificationPolicy,
    GeneralEndpoint, GeneralEndpointError, MemoryId,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
struct SharedGeneralEndpoint(Arc<dyn GeneralEndpoint>);
impl GeneralEndpoint for SharedGeneralEndpoint {
    fn model(&self) -> &str {
        self.0.model()
    }
    fn complete_json(
        &self,
        system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.0
            .complete_json(system_prompt, user_payload, schema_name, schema)
    }
}
struct DreamSnapshot {
    candidates: DreamCandidateSet,
    graph_version: u64,
    dream_epoch: u64,
}

fn lease(shared: &RelShared) -> Result<Option<super::LeaseGuard>, ReliquaryRuntimeHostError> {
    shared.admit()
}
fn prepare(
    shared: &RelShared,
    profile: CompatibilityProfileId,
    cooldowns: &HashMap<MemoryId, Instant>,
) -> Result<Option<DreamSnapshot>, ReliquaryRuntimeHostError> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
    let now = Instant::now();
    for id in runtime.cva.runtime_memory_ids() {
        if cooldowns.get(&id).is_some_and(|until| *until > now) {
            continue;
        }
        let Some(epoch) = crate::dream_cooldown::owner_eligible_epoch(
            crate::memory_store::MemoryReadView::new(&runtime.cva.memories, &runtime.cva.container),
            Some(&runtime.cva.archive),
            &runtime.cva.processing_epochs,
            id,
            unix_now_ns(),
        )
        .map_err(operation)?
        else {
            continue;
        };
        let body = runtime.cva.runtime_memory_body_id(id).map_err(operation)?;
        if runtime
            .cva
            .runtime_memory_vector_location(profile, body)
            .is_none()
        {
            continue;
        }
        let candidates = runtime
            .cva
            .native_dream_candidates(profile, id, DreamCandidateConfig::default())
            .map_err(operation)?;
        return Ok(Some(DreamSnapshot {
            candidates,
            graph_version: runtime.cva.runtime_graph_version(),
            dream_epoch: epoch,
        }));
    }
    Ok(None)
}
fn current(
    runtime: &mut crate::RelExecution,
    set: &DreamCandidateSet,
) -> Result<bool, ReliquaryRuntimeHostError> {
    if runtime
        .cva
        .runtime_memory(set.source.memory.id)
        .map_err(operation)?
        .revision
        != set.source.memory.revision
    {
        return Ok(false);
    }
    for candidate in &set.candidates {
        if runtime
            .cva
            .runtime_memory(candidate.context.memory.id)
            .map_err(operation)?
            .revision
            != candidate.context.memory.revision
        {
            return Ok(false);
        }
    }
    Ok(true)
}
fn perception_ids(snapshot: &DreamSnapshot) -> Vec<MemoryId> {
    let mut ids = vec![snapshot.candidates.source.memory.id];
    ids.extend(
        snapshot
            .candidates
            .candidates
            .iter()
            .map(|item| item.context.memory.id),
    );
    ids.sort_by_key(|id| id.0);
    ids.dedup();
    ids
}
fn commit(
    shared: &RelShared,
    snapshot: &DreamSnapshot,
    evaluated: Vec<(DreamPairClassification, Option<DreamPairVerification>)>,
) -> Result<bool, ReliquaryRuntimeHostError> {
    let Some(_lease) = lease(shared)? else {
        return Ok(false);
    };
    shared.mutate(
        super::super::mutation_events::RelMutationFamily::Background,
        |runtime| {
            if runtime.cva.runtime_graph_version() != snapshot.graph_version
                || !current(runtime, &snapshot.candidates)?
            {
                return Ok((false, false, vec![]));
            }
            let policy = DreamVerificationPolicy::default();
            for (classification, verification) in evaluated {
                let graph = runtime.cva.runtime_graph_version();
                runtime
                    .cva
                    .native_publish_dream_pair(
                        &classification,
                        verification.as_ref(),
                        policy,
                        graph,
                    )
                    .map_err(operation)?;
            }
            let id = snapshot.candidates.source.memory.id;
            runtime
                .cva
                .native_reconcile_dream_lifecycle(id)
                .map_err(operation)?;
            runtime
                .cva
                .owner
                .processing_epochs
                .put_for_memory(
                    &mut runtime.cva.owner.container,
                    &runtime.cva.owner.memories,
                    id,
                    crate::processing_epoch_model::DREAM_PROCESSING_LANE,
                    crate::dream_cooldown::dream_processing_state(
                        snapshot.dream_epoch,
                        unix_now_ns(),
                    ),
                )
                .map_err(operation)?;
            let resolutions = crate::entity_resolution_owner::ResolutionReadContext {
                memories: &runtime.cva.memories,
                entity_resolutions: &runtime.cva.entity_resolutions,
            };
            let keys = perception_ids(snapshot)
                .into_iter()
                .flat_map(|id| resolutions.schedulable_entity_mentions_for_memory(id))
                .collect::<Vec<_>>();
            shared
                .perception_queue
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .push_all(keys);
            Ok((
                true,
                true,
                vec![super::super::events::RelInvalidation::Memory(id)],
            ))
        },
    )
}

pub(super) fn worker_loop(shared: Arc<RelShared>) -> Result<(), ReliquaryRuntimeHostError> {
    let mut seen = 0_u64;
    let mut cooldowns = HashMap::<MemoryId, Instant>::new();
    loop {
        if stopped(&shared.signal)? {
            return Ok(());
        }
        if !shared.background_open()? {
            seen = super::wait_for_work_timeout(&shared.signal, seen, super::IDLE_POLL)?;
            continue;
        }
        let endpoint = shared
            .routes
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .dream();
        let profile = *shared
            .memory_profile
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let (Some(endpoint), Some(profile)) = (endpoint, profile) else {
            seen = wait_for_work(&shared.signal, seen)?;
            continue;
        };
        let Some(snapshot) = prepare(&shared, profile, &cooldowns)? else {
            cooldowns.retain(|_, until| *until > Instant::now());
            seen = wait_for_work(&shared.signal, seen)?;
            continue;
        };
        let source = snapshot.candidates.source.memory.id;
        let general = SharedGeneralEndpoint(endpoint);
        let processor = DreamProcessor::new(general.clone(), general);
        match processor.evaluate_candidates(
            &snapshot.candidates,
            DreamVerificationPolicy::default(),
            1,
        ) {
            Ok(evaluated) => {
                if commit(&shared, &snapshot, evaluated)? {
                    cooldowns.remove(&source);
                    notify_work(&shared.signal)?;
                } else if shared.fence.snapshot()?.0 == super::super::OwnerAdmission::Closed {
                    return Ok(());
                }
            }
            Err(DreamProcessError::Classification(_) | DreamProcessError::Verification(_)) => {
                cooldowns.insert(source, Instant::now() + DREAM_RETRY_POLL);
            }
            Err(error) => return Err(operation(error)),
        }
        cooldowns.retain(|_, until| *until > Instant::now());
    }
}
