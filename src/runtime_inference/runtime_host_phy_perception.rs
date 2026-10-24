use super::{WorkerShared, operation};
use crate::entity_reconciliation_staged::evaluate_reconciliation;
use crate::entity_resolution_commit::{
    EntityResolutionCommitContext, plan_entity_resolution_commit,
};
use crate::entity_resolution_engine::EntityResolutionEngineCore;
use crate::entity_resolution_owner::ResolutionReadContext;
use crate::{
    EntityCandidateConfig, EntityReconciliationError, EntityResolutionPreparation,
    EntityResolutionPrepared, EntityResolverError, GeneralEndpoint, GeneralEndpointError,
    MemoryEntityMentionKey,
};
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone)]
struct SharedEndpoint(Arc<dyn GeneralEndpoint>);
impl GeneralEndpoint for SharedEndpoint {
    fn model(&self) -> &str {
        self.0.model()
    }
    fn complete_json(
        &self,
        system: &str,
        payload: &str,
        name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.0.complete_json(system, payload, name, schema)
    }
}

pub(super) fn seed(shared: &WorkerShared) -> Result<(), super::ReliquaryRuntimeHostError> {
    let mut phy = shared
        .owner
        .lock()
        .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
    let mut keys = Vec::new();
    for id in phy.memories.current_ids() {
        if !phy
            .processing_epochs
            .state(id, crate::dream_cooldown::DREAM_PROCESSING_LANE)
            .is_some_and(|state| state.checkpoint_at_ns.is_some())
        {
            continue;
        }
        let memory = phy.memories.memory(&phy.container, id).map_err(operation)?;
        if !memory.archived && memory.superseded_by.is_none() {
            keys.extend(
                ResolutionReadContext {
                    memories: &phy.memories,
                    entity_resolutions: &phy.entity_resolutions,
                }
                .schedulable_entity_mentions_for_memory(id),
            );
        }
    }
    drop(phy);
    shared.enqueue(keys)
}

fn now_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

enum PreparedWork {
    Complete,
    Ready(EntityResolutionPrepared),
    Deferred,
}

fn prepare(
    shared: &WorkerShared,
    key: MemoryEntityMentionKey,
) -> Result<PreparedWork, super::ReliquaryRuntimeHostError> {
    let Some(_lease) = shared.lease()? else {
        return Ok(PreparedWork::Deferred);
    };
    if shared.stopped()? {
        return Ok(PreparedWork::Deferred);
    }
    let mut guard = shared
        .owner
        .lock()
        .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
    let now_ns = now_ns();
    let plan = {
        let phy = &mut *guard;
        crate::perception_publication_resolution::plan_phy_principal_resolution(
            crate::memory_store::MemoryReadView::new(&phy.memories, &phy.container),
            &phy.entities,
            &phy.entity_resolutions,
            &phy.graph,
            key,
            now_ns,
        )
        .map_err(operation)?
    };
    if let Some(plan) = plan {
        let phy = &mut *guard;
        crate::owner_lifecycle::perception_publication_resolution::apply_principal_resolution_plan(
            &mut phy.container,
            &phy.memories,
            &mut phy.entities,
            &mut phy.entity_resolutions,
            &mut phy.graph,
            plan,
            now_ns,
        )
        .map_err(operation)?;
        phy.container.sync().map_err(operation)?;
        return Ok(PreparedWork::Complete);
    }
    let prepared = {
        let phy = &mut *guard;
        let owner_id = phy.container.owner_id();
        crate::perception_publication_resolution::prepare_entity_resolution(
            crate::memory_store::MemoryReadView::new(&phy.memories, &phy.container),
            &mut phy.lexical_index,
            &phy.entities,
            &phy.graph,
            &phy.entity_resolutions,
            key,
            EntityCandidateConfig::default(),
            owner_id,
        )
        .map_err(operation)?
    };
    match prepared {
        EntityResolutionPreparation::Complete(_) => Ok(PreparedWork::Complete),
        EntityResolutionPreparation::Ready(p) => Ok(PreparedWork::Ready(p)),
    }
}

fn commit(
    shared: &WorkerShared,
    prepared: EntityResolutionPrepared,
    evaluation: crate::EntityResolutionEvaluation,
) -> Result<Option<Vec<MemoryEntityMentionKey>>, super::ReliquaryRuntimeHostError> {
    let Some(_lease) = shared.lease()? else {
        return Ok(None);
    };
    if shared.stopped()? {
        return Ok(None);
    }
    let surface = prepared.candidates.mention.text.clone();
    let mut guard = shared
        .owner
        .lock()
        .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
    let now_ns = now_ns();
    let outcome = {
        let phy = &mut *guard;
        let context = EntityResolutionCommitContext {
            owner_id: phy.container.owner_id(),
            memories: &phy.memories,
            entities: &phy.entities,
            graph: &phy.graph,
            resolutions: &phy.entity_resolutions,
        };
        let Some(plan) = plan_entity_resolution_commit(&context, prepared, evaluation, now_ns)
            .map_err(operation)?
        else {
            return Ok(None);
        };
        crate::owner_lifecycle::perception_publication_resolution::apply_entity_resolution_commit_plan(
            &mut phy.container,
            &phy.memories,
            &mut phy.entities,
            &mut phy.entity_resolutions,
            &mut phy.graph,
            plan,
            true,
        )
        .map_err(operation)?
    };
    let phy = &mut *guard;
    phy.container.sync().map_err(operation)?;
    let mut wakes = ResolutionReadContext {
        memories: &phy.memories,
        entity_resolutions: &phy.entity_resolutions,
    }
    .pending_entity_mentions_for_surface(&surface)
    .into_iter()
    .filter(|k| *k != outcome.key)
    .collect::<Vec<_>>();
    if outcome.entity_created || outcome.association_changed {
        if let Some(id) = outcome.entity_id {
            wakes.extend(
                ResolutionReadContext {
                    memories: &phy.memories,
                    entity_resolutions: &phy.entity_resolutions,
                }
                .pending_entity_mentions_for_candidate(id),
            );
        }
    }
    wakes.retain(|key| *key != outcome.key);
    wakes.sort_by_key(sort_key);
    wakes.dedup();
    Ok(Some(wakes))
}

fn sort_key(key: &MemoryEntityMentionKey) -> ([u8; 32], u8, u32, u32) {
    (
        key.memory_id.0,
        key.field.tag(),
        key.start_byte,
        key.end_byte,
    )
}

fn reconcile(
    shared: &WorkerShared,
    endpoint: Arc<dyn GeneralEndpoint>,
    last: &mut Option<(u64, u64, u64)>,
) -> Result<bool, super::ReliquaryRuntimeHostError> {
    if shared
        .perception
        .lock()
        .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?
        .len()
        != 0
    {
        return Ok(false);
    }
    let prepared = {
        let mut phy = shared
            .owner
            .lock()
            .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
        let versions = (
            phy.entities.entity_version(),
            phy.graph.graph_version(),
            phy.memories.memory_version(),
        );
        if *last == Some(versions) {
            return Ok(false);
        }
        (
            versions,
            crate::perception_publication_reconciliation::prepare_entity_reconciliation(
                &phy.container,
                &phy.memories,
                &phy.entities,
                &phy.graph,
                &phy.entity_resolutions,
            )
            .map_err(operation)?,
        )
    };
    let evaluated = match evaluate_reconciliation(&SharedEndpoint(endpoint), &prepared.1) {
        Ok(v) => v,
        Err(EntityReconciliationError::Endpoint(_)) => return Ok(false),
        Err(e) => return Err(operation(e)),
    };
    let Some(_lease) = shared.lease()? else {
        return Ok(false);
    };
    if shared.stopped()? {
        return Ok(false);
    }
    let mut guard = shared
        .owner
        .lock()
        .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
    let commit = {
        let phy = &mut *guard;
        crate::perception_publication_reconciliation::commit_entity_reconciliation::<
            crate::PhylacteryError,
        >(
            &mut phy.container,
            &phy.memories,
            &mut phy.entities,
            &mut phy.entity_resolutions,
            &mut phy.graph,
            &mut phy.relationships,
            prepared.1,
            evaluated,
            now_ns(),
        )
        .map_err(operation)?
    };
    let Some(commit) = commit else {
        return Ok(false);
    };
    guard.container.sync().map_err(operation)?;
    if commit.report.changed() {
        drop(guard);
        seed(shared)?;
        shared.wake()?;
        return Ok(true);
    }
    let phy = &*guard;
    *last = Some((
        phy.entities.entity_version(),
        phy.graph.graph_version(),
        phy.memories.memory_version(),
    ));
    Ok(false)
}

pub(super) fn worker_loop(
    shared: Arc<WorkerShared>,
) -> Result<(), super::ReliquaryRuntimeHostError> {
    seed(&shared)?;
    let mut seen = 0;
    let mut last_versions = None;

    loop {
        if shared.stopped()? {
            return Ok(());
        }
        if !shared.background_open()? {
            shared.wait(&mut seen, super::IDLE_POLL)?;
            continue;
        }
        let (endpoint, decision_endpoint) = {
            let routes = shared
                .routes
                .read()
                .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
            (
                routes.entity_resolution(),
                routes.entity_resolution_decision(),
            )
        };
        let Some(endpoint) = endpoint else {
            shared.wait(&mut seen, super::IDLE_POLL)?;
            continue;
        };
        if reconcile(&shared, Arc::clone(&endpoint), &mut last_versions)? {
            continue;
        }
        let key = {
            let mut queue = shared
                .perception
                .lock()
                .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
            queue.pop_ready(Instant::now())
        };
        let Some(key) = key else {
            shared.wait(&mut seen, super::IDLE_POLL)?;
            continue;
        };
        let prepared = match prepare(&shared, key)? {
            PreparedWork::Complete => {
                shared
                    .perception
                    .lock()
                    .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?
                    .finish();
                continue;
            }
            PreparedWork::Deferred => {
                let mut queue = shared
                    .perception
                    .lock()
                    .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
                queue.finish();
                queue.push(key);
                drop(queue);
                if shared.stopped()? {
                    return Ok(());
                }
                shared.wait(&mut seen, super::IDLE_POLL)?;
                continue;
            }
            PreparedWork::Ready(prepared) => prepared,
        };
        let engine = EntityResolutionEngineCore::new(SharedEndpoint(endpoint))
            .with_decision_endpoint(decision_endpoint);
        let evaluation = match engine.evaluate_prepared(&prepared) {
            Ok(value) => value,
            Err(EntityResolverError::Endpoint(_)) => {
                let mut queue = shared
                    .perception
                    .lock()
                    .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
                queue.finish();
                queue.defer(key);
                drop(queue);
                shared.wait(&mut seen, Duration::from_secs(1))?;
                continue;
            }
            Err(error) => {
                shared
                    .perception
                    .lock()
                    .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?
                    .finish();
                return Err(operation(error));
            }
        };
        match commit(&shared, prepared, evaluation)? {
            Some(wakes) => {
                let mut queue = shared
                    .perception
                    .lock()
                    .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
                queue.finish();
                for wake in wakes {
                    queue.push(wake);
                }
                drop(queue);
                shared.wake()?;
            }
            None => {
                let mut queue = shared
                    .perception
                    .lock()
                    .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
                queue.finish();
                queue.push(key);
                drop(queue);
                if shared.stopped()? {
                    return Ok(());
                }
            }
        }
    }
}
