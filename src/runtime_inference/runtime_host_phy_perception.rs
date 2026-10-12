use super::{WorkerShared, operation};
use crate::entity_reconciliation_staged::evaluate_reconciliation;
use crate::{
    EntityCandidateConfig, EntityReconciliationError, EntityResolutionEngine,
    EntityResolutionPreparation, EntityResolutionPrepared, EntityResolverError, GeneralEndpoint,
    GeneralEndpointError, MemoryEntityMentionKey,
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
    for id in phy.memory_ids() {
        if !phy.dream_was_processed(id) {
            continue;
        }
        let memory = phy.memory(id).map_err(operation)?;
        if !memory.archived && memory.superseded_by.is_none() {
            keys.extend(phy.schedulable_entity_mentions_for_memory(id));
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
    let mut phy = shared
        .owner
        .lock()
        .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
    if phy
        .resolve_principal_entity_mention(key, now_ns())
        .map_err(operation)?
        .is_some()
    {
        phy.sync().map_err(operation)?;
        return Ok(PreparedWork::Complete);
    }
    match phy
        .prepare_entity_resolution(key, EntityCandidateConfig::default())
        .map_err(operation)?
    {
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
    let mut phy = shared
        .owner
        .lock()
        .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
    let Some(outcome) = phy
        .commit_entity_resolution(prepared, evaluation, now_ns())
        .map_err(operation)?
    else {
        return Ok(None);
    };
    phy.sync().map_err(operation)?;
    let mut wakes = phy
        .pending_entity_mentions_for_surface(&surface)
        .into_iter()
        .filter(|k| *k != outcome.key)
        .collect::<Vec<_>>();
    if outcome.entity_created || outcome.association_changed {
        if let Some(id) = outcome.entity_id {
            wakes.extend(phy.pending_entity_mentions_for_candidate(id));
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
            phy.entity_version(),
            phy.graph_version(),
            phy.memory_version(),
        );
        if *last == Some(versions) {
            return Ok(false);
        }
        (
            versions,
            phy.prepare_entity_reconciliation().map_err(operation)?,
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
    let mut phy = shared
        .owner
        .lock()
        .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
    let Some(commit) = phy
        .commit_entity_reconciliation(prepared.1, evaluated, now_ns())
        .map_err(operation)?
    else {
        return Ok(false);
    };
    phy.sync().map_err(operation)?;
    if commit.report.changed() {
        drop(phy);
        seed(shared)?;
        shared.wake()?;
        return Ok(true);
    }
    *last = Some((
        phy.entity_version(),
        phy.graph_version(),
        phy.memory_version(),
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
        let engine = EntityResolutionEngine::new(SharedEndpoint(endpoint))
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
