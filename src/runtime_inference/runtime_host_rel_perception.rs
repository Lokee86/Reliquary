use super::{
    DREAM_RETRY_POLL, IDLE_POLL, RelShared, ReliquaryRuntimeHostError, notify_work, operation,
    stopped, wait_for_work_timeout,
};
use crate::entity_reconciliation_staged::evaluate_reconciliation;
use crate::entity_resolution_commit::{
    EntityResolutionCommitContext, plan_entity_resolution_commit,
};
use crate::entity_resolution_engine::EntityResolutionEngineCore;
use crate::entity_resolution_owner::ResolutionReadContext;
use crate::{
    EntityCandidateConfig, EntityReconciliationError, EntityResolutionEvaluation,
    EntityResolutionPreparation, EntityResolutionPrepared, EntityResolverError, GeneralEndpoint,
    GeneralEndpointError, MemoryEntityMentionKey,
};
use serde_json::Value;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Default)]
struct Queue {
    order: VecDeque<MemoryEntityMentionKey>,
    queued: HashSet<MemoryEntityMentionKey>,
    delayed: std::collections::HashMap<MemoryEntityMentionKey, Instant>,
    in_flight: usize,
}
#[derive(Default)]
pub(in super::super) struct RelPerceptionQueue(Queue);
impl RelPerceptionQueue {
    pub(super) fn push_all(&mut self, keys: impl IntoIterator<Item = MemoryEntityMentionKey>) {
        for key in keys {
            if self.0.queued.insert(key) {
                self.0.order.push_back(key);
            }
        }
    }
    pub(in super::super) fn counts(&self) -> usize {
        self.0.queued.len().saturating_add(self.0.in_flight)
    }
    fn pop(&mut self, now: Instant) -> Option<MemoryEntityMentionKey> {
        let n = self.0.order.len();
        for _ in 0..n {
            let key = self.0.order.pop_front()?;
            if self.0.delayed.get(&key).is_some_and(|until| *until > now) {
                self.0.order.push_back(key);
                continue;
            }
            self.0.queued.remove(&key);
            self.0.delayed.remove(&key);
            self.0.in_flight += 1;
            return Some(key);
        }
        None
    }
    fn finish(&mut self) {
        self.0.in_flight = self.0.in_flight.saturating_sub(1);
    }
    fn defer(&mut self, key: MemoryEntityMentionKey) {
        if self.0.queued.insert(key) {
            self.0.order.push_back(key);
        }
        self.0
            .delayed
            .insert(key, Instant::now() + DREAM_RETRY_POLL);
    }
}
#[derive(Clone)]
struct SharedEndpoint(Arc<dyn GeneralEndpoint>);
impl GeneralEndpoint for SharedEndpoint {
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
fn lease(shared: &RelShared) -> Result<Option<super::LeaseGuard>, ReliquaryRuntimeHostError> {
    shared.admit()
}
fn now_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}
fn startup(shared: &RelShared) -> Result<(), ReliquaryRuntimeHostError> {
    let keys = {
        let mut runtime = shared
            .runtime
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let ids = runtime.cva.memories.current_ids();
        let mut keys = Vec::new();
        for id in ids {
            if !runtime
                .cva
                .processing_epochs
                .state(id, crate::dream_cooldown::DREAM_PROCESSING_LANE)
                .is_some_and(|state| state.checkpoint_at_ns.is_some())
            {
                continue;
            }
            let m = runtime
                .cva
                .memories
                .memory(&runtime.cva.container, id)
                .map_err(operation)?;
            if !m.archived && m.superseded_by.is_none() {
                keys.extend(
                    ResolutionReadContext {
                        memories: &runtime.cva.memories,
                        entity_resolutions: &runtime.cva.entity_resolutions,
                    }
                    .schedulable_entity_mentions_for_memory(id),
                );
            }
        }
        keys
    };
    shared
        .perception_queue
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
        .push_all(keys);
    Ok(())
}
fn prepare(
    shared: &RelShared,
    key: MemoryEntityMentionKey,
) -> Result<Option<EntityResolutionPreparation>, ReliquaryRuntimeHostError> {
    let Some(_lease) = lease(shared)? else {
        return Ok(None);
    };
    let complete = shared.mutate(
        super::super::mutation_events::RelMutationFamily::Background,
        |runtime| {
            let now_ns = now_ns();
            let outcome = {
                let cva = &mut runtime.cva;
                let plan = crate::perception_publication_resolution::plan_rel_principal_resolution(
                    &cva.container,
                    &cva.memories,
                    &cva.entities,
                    &cva.entity_resolutions,
                    &cva.graph,
                    &cva.archive,
                    key,
                    now_ns,
                )
                .map_err(operation)?;
                plan.map(|plan| {
                    crate::perception_publication_resolution::apply_principal_resolution_plan(
                        &mut cva.container,
                        &cva.memories,
                        &mut cva.entities,
                        &mut cva.entity_resolutions,
                        &mut cva.graph,
                        plan,
                        now_ns,
                    )
                    .map_err(operation)
                })
                .transpose()?
            };
            let changed = outcome.is_some();
            Ok((
                outcome.map(EntityResolutionPreparation::Complete),
                changed,
                vec![super::super::events::RelInvalidation::Memory(key.memory_id)],
            ))
        },
    )?;
    if complete.is_some() {
        return Ok(complete);
    }
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
    let cva = &mut runtime.cva;
    crate::perception_publication_resolution::prepare_entity_resolution(
        &cva.container,
        &cva.memories,
        &mut cva.lexical_index,
        &cva.entities,
        &cva.graph,
        &cva.entity_resolutions,
        key,
        EntityCandidateConfig::default(),
        cva.container.owner_id(),
    )
    .map(Some)
    .map_err(operation)
}
fn commit(
    shared: &RelShared,
    prepared: EntityResolutionPrepared,
    evaluation: EntityResolutionEvaluation,
) -> Result<Option<Vec<MemoryEntityMentionKey>>, ReliquaryRuntimeHostError> {
    let Some(_lease) = lease(shared)? else {
        return Ok(None);
    };
    let surface = prepared.candidates.mention.text.clone();
    shared.mutate(
        super::super::mutation_events::RelMutationFamily::Background,
        |runtime| {
            let now_ns = now_ns();
            let cva = &mut runtime.cva;
            let context = EntityResolutionCommitContext {
                owner_id: cva.container.owner_id(),
                memories: &cva.memories,
                entities: &cva.entities,
                graph: &cva.graph,
                resolutions: &cva.entity_resolutions,
            };
            let Some(plan) = plan_entity_resolution_commit(&context, prepared, evaluation, now_ns)
                .map_err(operation)?
            else {
                return Ok((None, false, vec![]));
            };
            let outcome =
                crate::perception_publication_resolution::apply_entity_resolution_commit_plan(
                    &mut cva.container,
                    &cva.memories,
                    &mut cva.entities,
                    &mut cva.entity_resolutions,
                    &mut cva.graph,
                    plan,
                    false,
                )
                .map_err(operation)?;
            let mut wakes = ResolutionReadContext {
                memories: &runtime.cva.memories,
                entity_resolutions: &runtime.cva.entity_resolutions,
            }
            .pending_entity_mentions_for_surface(&surface)
            .into_iter()
            .filter(|k| *k != outcome.key)
            .collect::<Vec<_>>();
            if outcome.entity_created || outcome.association_changed {
                if let Some(id) = outcome.entity_id {
                    wakes.extend(
                        ResolutionReadContext {
                            memories: &runtime.cva.memories,
                            entity_resolutions: &runtime.cva.entity_resolutions,
                        }
                        .pending_entity_mentions_for_candidate(id),
                    );
                }
            }
            wakes.retain(|k| *k != outcome.key);
            wakes.sort_by_key(|k| (k.memory_id.0, k.field.tag(), k.start_byte, k.end_byte));
            wakes.dedup();
            Ok((
                Some(wakes),
                true,
                vec![super::super::events::RelInvalidation::Memory(
                    outcome.key.memory_id,
                )],
            ))
        },
    )
}
fn versions(shared: &RelShared) -> Result<(u64, u64, u64), ReliquaryRuntimeHostError> {
    let r = shared
        .runtime
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
    Ok((
        r.cva.entities.entity_version(),
        r.cva.graph.graph_version(),
        r.cva.memories.memory_version(),
    ))
}
fn reconcile(
    shared: &RelShared,
    endpoint: Arc<dyn GeneralEndpoint>,
    last: &mut Option<(u64, u64, u64)>,
) -> Result<bool, ReliquaryRuntimeHostError> {
    let before = versions(shared)?;
    if *last == Some(before) {
        return Ok(false);
    }
    if shared
        .perception_queue
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
        .counts()
        != 0
    {
        return Ok(false);
    }
    let prepared = {
        let mut r = shared
            .runtime
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        crate::perception_publication_reconciliation::prepare_entity_reconciliation(
            &r.cva.container,
            &r.cva.memories,
            &r.cva.entities,
            &r.cva.graph,
            &r.cva.entity_resolutions,
        )
        .map_err(operation)?
    };
    let endpoint = SharedEndpoint(endpoint);
    let evaluation = match evaluate_reconciliation(&endpoint, &prepared) {
        Ok(v) => v,
        Err(EntityReconciliationError::Endpoint(_)) => return Ok(false),
        Err(e) => return Err(operation(e)),
    };
    let Some(_lease) = lease(shared)? else {
        return Ok(false);
    };
    let result = shared.mutate(
        super::super::mutation_events::RelMutationFamily::Background,
        |r| {
            let result =
                crate::perception_publication_reconciliation::commit_entity_reconciliation::<
                    crate::CvaError,
                >(
                    &mut r.cva.container,
                    &r.cva.memories,
                    &mut r.cva.entities,
                    &mut r.cva.entity_resolutions,
                    &mut r.cva.graph,
                    &mut r.cva.relationships,
                    prepared,
                    evaluation,
                    now_ns(),
                )
                .map_err(operation)?;
            let changed = result.as_ref().is_some_and(|v| v.report.changed());
            Ok((result, changed, vec![]))
        },
    )?;
    let Some(result) = result else {
        return Ok(false);
    };
    if result.report.changed() {
        startup(shared)?;
        notify_work(&shared.signal)?;
        *last = None;
        return Ok(true);
    }
    *last = Some(versions(shared)?);
    Ok(false)
}

pub(super) fn worker_loop(shared: Arc<RelShared>) -> Result<(), ReliquaryRuntimeHostError> {
    startup(&shared)?;
    let mut seen = 0_u64;
    let mut last_versions = None;
    loop {
        if stopped(&shared.signal)? {
            return Ok(());
        }
        if !shared.background_open()? {
            seen = wait_for_work_timeout(&shared.signal, seen, IDLE_POLL)?;
            continue;
        }
        let (endpoint, decision) = {
            let routes = shared
                .routes
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            (
                routes.entity_resolution(),
                routes.entity_resolution_decision(),
            )
        };
        let Some(endpoint) = endpoint else {
            seen = wait_for_work_timeout(&shared.signal, seen, IDLE_POLL)?;
            continue;
        };
        if reconcile(&shared, Arc::clone(&endpoint), &mut last_versions)? {
            continue;
        }
        let key = shared
            .perception_queue
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .pop(Instant::now());
        let Some(key) = key else {
            seen = wait_for_work_timeout(&shared.signal, seen, IDLE_POLL)?;
            continue;
        };
        let preparation = match prepare(&shared, key) {
            Ok(Some(v)) => v,
            Ok(None) => {
                shared
                    .perception_queue
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .finish();
                continue;
            }
            Err(e) => {
                shared
                    .perception_queue
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .finish();
                return Err(e);
            }
        };
        let prepared = match preparation {
            EntityResolutionPreparation::Complete(_) => {
                shared
                    .perception_queue
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .finish();
                continue;
            }
            EntityResolutionPreparation::Ready(v) => v,
        };
        let engine = EntityResolutionEngineCore::new(SharedEndpoint(endpoint))
            .with_decision_endpoint(decision);
        let evaluation = match engine.evaluate_prepared(&prepared) {
            Ok(v) => v,
            Err(EntityResolverError::Endpoint(_)) => {
                let mut q = shared
                    .perception_queue
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                q.finish();
                q.defer(key);
                continue;
            }
            Err(e) => {
                shared
                    .perception_queue
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .finish();
                return Err(operation(e));
            }
        };
        let committed = commit(&shared, prepared, evaluation)?;
        let did_commit = committed.is_some();
        {
            let mut q = shared
                .perception_queue
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            q.finish();
            if let Some(wakes) = committed {
                q.push_all(wakes);
            } else {
                // Preserve in-flight REL work across fencing and worker restart.
                q.push_all([key]);
            }
        }
        if shared.fence.snapshot()?.0 == super::super::OwnerAdmission::Closed {
            return Ok(());
        }
        if did_commit {
            notify_work(&shared.signal)?
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RelPerceptionQueue;
    use crate::{MemoryEntityMentionKey, MemoryId, MemoryTextField};
    use std::time::Instant;

    fn key(byte: u32) -> MemoryEntityMentionKey {
        MemoryEntityMentionKey {
            memory_id: MemoryId([7; 32]),
            field: MemoryTextField::Content,
            start_byte: byte,
            end_byte: byte + 1,
        }
    }

    #[test]
    fn rel_queue_deduplicates_and_tracks_in_flight() {
        let mut queue = RelPerceptionQueue::default();
        queue.push_all([key(0), key(0), key(2)]);
        assert_eq!(queue.counts(), 2);
        assert_eq!(queue.pop(Instant::now()), Some(key(0)));
        assert_eq!(queue.counts(), 2);
        queue.finish();
        assert_eq!(queue.counts(), 1);
    }

    #[test]
    fn rel_queue_deferred_key_becomes_ready_after_deadline() {
        let mut queue = RelPerceptionQueue::default();
        queue.defer(key(4));
        assert_eq!(queue.pop(Instant::now()), None);
        queue
            .0
            .delayed
            .insert(key(4), Instant::now() - std::time::Duration::from_secs(1));
        assert_eq!(queue.pop(Instant::now()), Some(key(4)));
    }
}
