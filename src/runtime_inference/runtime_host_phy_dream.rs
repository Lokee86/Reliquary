use super::{WorkerShared, operation};
use crate::dream_cooldown::unix_now_ns;
use crate::{
    DreamCandidateConfig, DreamPairClassification, DreamPairVerification, DreamProcessError,
    DreamProcessor, DreamVerificationPolicy, GeneralEndpoint, GeneralEndpointError, MemoryId,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct SharedGeneral(Arc<dyn GeneralEndpoint>);
impl GeneralEndpoint for SharedGeneral {
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
struct Snapshot {
    candidates: crate::DreamCandidateSet,
    graph_version: u64,
    dream_epoch: u64,
}

fn prepare(
    shared: &WorkerShared,
    profile: crate::CompatibilityProfileId,
    cooldowns: &HashMap<MemoryId, Instant>,
) -> Result<Option<Snapshot>, super::ReliquaryRuntimeHostError> {
    let mut phy = shared
        .owner
        .lock()
        .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
    let now = Instant::now();
    for id in phy.memory_ids() {
        if cooldowns.get(&id).is_some_and(|until| *until > now) {
            continue;
        }
        let Some(epoch) = phy
            .dream_eligible_epoch(id, unix_now_ns())
            .map_err(operation)?
        else {
            continue;
        };
        let body = phy.memory_body_id(id).map_err(operation)?;
        if phy.memory_vector_location(profile, body).is_none() {
            continue;
        }
        let candidates = phy
            .dream_candidates(profile, id, DreamCandidateConfig::default())
            .map_err(operation)?;
        return Ok(Some(Snapshot {
            candidates,
            graph_version: phy.graph_version(),
            dream_epoch: epoch,
        }));
    }
    Ok(None)
}

fn current(
    phy: &mut crate::Phylactery,
    set: &crate::DreamCandidateSet,
) -> Result<bool, super::ReliquaryRuntimeHostError> {
    if phy
        .memory(set.source.memory.id)
        .map_err(operation)?
        .revision
        != set.source.memory.revision
    {
        return Ok(false);
    }
    for candidate in &set.candidates {
        if phy
            .memory(candidate.context.memory.id)
            .map_err(operation)?
            .revision
            != candidate.context.memory.revision
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn commit(
    shared: &WorkerShared,
    snapshot: &Snapshot,
    evaluated: Vec<(DreamPairClassification, Option<DreamPairVerification>)>,
) -> Result<bool, super::ReliquaryRuntimeHostError> {
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
    if phy.graph_version() != snapshot.graph_version || !current(&mut phy, &snapshot.candidates)? {
        return Ok(false);
    }
    let policy = DreamVerificationPolicy::default();
    for (classification, verification) in evaluated {
        let version = phy.graph_version();
        phy.publish_dream_pair(&classification, verification.as_ref(), policy, version)
            .map_err(operation)?;
    }
    let source = snapshot.candidates.source.memory.id;
    phy.reconcile_dream_lifecycle(source).map_err(operation)?;
    phy.mark_dream_processed(source, snapshot.dream_epoch, unix_now_ns())
        .map_err(operation)?;
    phy.sync().map_err(operation)?;
    let mut ids = vec![source];
    ids.extend(
        snapshot
            .candidates
            .candidates
            .iter()
            .map(|c| c.context.memory.id),
    );
    ids.sort_by_key(|id| id.0);
    ids.dedup();
    let keys = ids
        .into_iter()
        .flat_map(|id| phy.schedulable_entity_mentions_for_memory(id))
        .collect::<Vec<_>>();
    drop(phy);
    shared.enqueue(keys)?;
    Ok(true)
}

pub(super) fn worker_loop(
    shared: Arc<WorkerShared>,
) -> Result<(), super::ReliquaryRuntimeHostError> {
    let mut seen = 0;
    let mut cooldowns = HashMap::<MemoryId, Instant>::new();
    loop {
        if shared.stopped()? {
            return Ok(());
        }
        if !shared.admission_open()? {
            shared.wait(&mut seen, super::IDLE_POLL)?;
            continue;
        }
        let endpoint = shared
            .routes
            .read()
            .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?
            .dream();
        let profile = shared
            .profile
            .lock()
            .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?
            .as_ref()
            .map(|p| p.id);
        let (Some(endpoint), Some(profile)) = (endpoint, profile) else {
            shared.wait(&mut seen, super::IDLE_POLL)?;
            continue;
        };
        let Some(snapshot) = prepare(&shared, profile, &cooldowns)? else {
            cooldowns.retain(|_, until| *until > Instant::now());
            shared.wait(&mut seen, super::IDLE_POLL)?;
            continue;
        };
        let source = snapshot.candidates.source.memory.id;
        let general = SharedGeneral(endpoint);
        let processor = DreamProcessor::new(general.clone(), general);
        match processor.evaluate_candidates(
            &snapshot.candidates,
            DreamVerificationPolicy::default(),
            1,
        ) {
            Ok(evaluated) => {
                if commit(&shared, &snapshot, evaluated)? {
                    cooldowns.remove(&source);
                    shared.wake()?;
                } else if shared.stopped()? {
                    return Ok(());
                }
            }
            Err(DreamProcessError::Classification(_) | DreamProcessError::Verification(_)) => {
                cooldowns.insert(source, Instant::now() + Duration::from_secs(60));
            }
            Err(error) => return Err(operation(error)),
        }
        if shared.stopped()? {
            return Ok(());
        }
    }
}
