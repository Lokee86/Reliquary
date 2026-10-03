use super::perception_queue::PerceptionOwner;
use super::{ReliquaryRuntimeHostError, Shared, operation};
use crate::dream_cooldown::unix_now_ns;
use crate::{
    CompatibilityProfileId, DreamCandidateConfig, DreamCandidateSet, DreamPairClassification,
    DreamPairVerification, DreamVerificationPolicy, MemoryId,
};
use std::collections::HashMap;
use std::time::Instant;

pub(super) struct DreamSnapshot {
    pub(super) candidates: DreamCandidateSet,
    graph_version: u64,
    dream_epoch: u64,
}

pub(super) fn prepare_project(
    shared: &Shared,
    profile_id: CompatibilityProfileId,
    cooldowns: &HashMap<MemoryId, Instant>,
) -> Result<Option<DreamSnapshot>, ReliquaryRuntimeHostError> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
    let now = Instant::now();
    let now_ns = unix_now_ns();
    for id in runtime.cva.memory_ids() {
        if cooldowns.get(&id).is_some_and(|until| *until > now) {
            continue;
        }
        let Some(dream_epoch) = runtime
            .cva
            .dream_eligible_epoch(id, now_ns)
            .map_err(operation)?
        else {
            continue;
        };
        let body_id = runtime.cva.memory_body_id(id).map_err(operation)?;
        if runtime
            .cva
            .memory_vector_location(profile_id, body_id)
            .is_none()
        {
            continue;
        }
        let candidates = runtime
            .cva
            .dream_candidates(profile_id, id, DreamCandidateConfig::default())
            .map_err(operation)?;
        return Ok(Some(DreamSnapshot {
            candidates,
            graph_version: runtime.cva.graph_version(),
            dream_epoch,
        }));
    }
    Ok(None)
}

pub(super) fn prepare_user(
    shared: &Shared,
    profile_id: CompatibilityProfileId,
    cooldowns: &HashMap<MemoryId, Instant>,
) -> Result<Option<DreamSnapshot>, ReliquaryRuntimeHostError> {
    if !shared.phylactery_active() {
        return Ok(None);
    }
    let mut phylactery = shared
        .phylactery
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
    let Some(phy) = phylactery.as_mut() else {
        return Ok(None);
    };
    let now = Instant::now();
    let now_ns = unix_now_ns();
    for id in phy.memory_ids() {
        if cooldowns.get(&id).is_some_and(|until| *until > now) {
            continue;
        }
        let Some(dream_epoch) = phy.dream_eligible_epoch(id, now_ns).map_err(operation)? else {
            continue;
        };
        let body_id = phy.memory_body_id(id).map_err(operation)?;
        if phy.memory_vector_location(profile_id, body_id).is_none() {
            continue;
        }
        let candidates = phy
            .dream_candidates(profile_id, id, DreamCandidateConfig::default())
            .map_err(operation)?;
        return Ok(Some(DreamSnapshot {
            candidates,
            graph_version: phy.graph_version(),
            dream_epoch,
        }));
    }
    Ok(None)
}

pub(super) fn commit_project(
    shared: &Shared,
    snapshot: &DreamSnapshot,
    evaluated: Vec<(DreamPairClassification, Option<DreamPairVerification>)>,
) -> Result<bool, ReliquaryRuntimeHostError> {
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
    if runtime.cva.graph_version() != snapshot.graph_version
        || !project_snapshot_is_current(&mut runtime.cva, &snapshot.candidates)?
    {
        return Ok(false);
    }
    let policy = DreamVerificationPolicy::default();
    let source_id = snapshot.candidates.source.memory.id;
    crate::dream_freshness::begin_initial_pass(
        &mut runtime.cva,
        source_id,
        snapshot
            .candidates
            .candidates
            .iter()
            .map(|candidate| candidate.context.memory.id),
    )
    .map_err(operation)?;
    for (classification, verification) in evaluated {
        let graph_version = runtime.cva.graph_version();
        let publication = runtime
            .cva
            .publish_dream_pair(
                &classification,
                verification.as_ref(),
                policy,
                graph_version,
            )
            .map_err(operation)?;
        crate::dream_freshness::record_publication_roots(&mut runtime.cva, source_id, &publication)
            .map_err(operation)?;
    }
    if let Some(pass) = runtime.cva.pending_initial_dream_pass(source_id) {
        let settlement_turn = runtime.cva.rel_turn_count();
        runtime
            .cva
            .accept_initial_dream_settlement(&pass.pass_id, settlement_turn)
            .map_err(operation)?;
    }
    let lifecycle = runtime
        .cva
        .reconcile_dream_lifecycle(source_id)
        .map_err(operation)?;
    let active_admission = !lifecycle.source.archived
        && matches!(
            lifecycle.source.lifecycle_state.as_str(),
            "knowledge" | "canonical"
        );
    let freshness_work = crate::dream_freshness::prepare_initial_settlement(
        &mut runtime.cva,
        source_id,
        active_admission,
    )
    .map_err(operation)?;
    drop(runtime);
    let freshness_result = freshness_work
        .map(|work| work.evaluate(crate::freshness::default_worker_count()))
        .transpose()
        .map_err(operation)?;
    let mut runtime = shared
        .runtime
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
    if let Some(result) = freshness_result {
        result.commit(&mut runtime.cva).map_err(operation)?;
    }
    runtime
        .cva
        .mark_dream_processed(source_id, snapshot.dream_epoch, unix_now_ns())
        .map_err(operation)?;
    runtime.cva.sync().map_err(operation)?;
    let keys = perception_keys_for_project(&runtime.cva, snapshot);
    shared
        .perception_queue
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
        .push_all(PerceptionOwner::Project, keys);
    Ok(true)
}

pub(super) fn commit_user(
    shared: &Shared,
    snapshot: &DreamSnapshot,
    evaluated: Vec<(DreamPairClassification, Option<DreamPairVerification>)>,
) -> Result<bool, ReliquaryRuntimeHostError> {
    if !shared.phylactery_active() {
        return Ok(false);
    }
    let mut phylactery = shared
        .phylactery
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
    let Some(phy) = phylactery.as_mut() else {
        return Ok(false);
    };
    if phy.graph_version() != snapshot.graph_version
        || !user_snapshot_is_current(phy, &snapshot.candidates)?
    {
        return Ok(false);
    }
    let policy = DreamVerificationPolicy::default();
    for (classification, verification) in evaluated {
        let graph_version = phy.graph_version();
        phy.publish_dream_pair(
            &classification,
            verification.as_ref(),
            policy,
            graph_version,
        )
        .map_err(operation)?;
    }
    let source_id = snapshot.candidates.source.memory.id;
    phy.reconcile_dream_lifecycle(source_id)
        .map_err(operation)?;
    phy.mark_dream_processed(source_id, snapshot.dream_epoch, unix_now_ns())
        .map_err(operation)?;
    phy.sync().map_err(operation)?;
    let keys = perception_keys_for_user(phy, snapshot);
    shared
        .perception_queue
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
        .push_all(PerceptionOwner::User, keys);
    Ok(true)
}

fn perception_memory_ids(snapshot: &DreamSnapshot) -> Vec<MemoryId> {
    let mut ids = vec![snapshot.candidates.source.memory.id];
    ids.extend(
        snapshot
            .candidates
            .candidates
            .iter()
            .map(|candidate| candidate.context.memory.id),
    );
    ids.sort_by_key(|id| id.0);
    ids.dedup();
    ids
}

fn perception_keys_for_project(
    cva: &crate::Cva,
    snapshot: &DreamSnapshot,
) -> Vec<crate::MemoryEntityMentionKey> {
    perception_memory_ids(snapshot)
        .into_iter()
        .flat_map(|id| cva.schedulable_entity_mentions_for_memory(id))
        .collect()
}

fn perception_keys_for_user(
    phy: &crate::Phylactery,
    snapshot: &DreamSnapshot,
) -> Vec<crate::MemoryEntityMentionKey> {
    perception_memory_ids(snapshot)
        .into_iter()
        .flat_map(|id| phy.schedulable_entity_mentions_for_memory(id))
        .collect()
}

fn project_snapshot_is_current(
    cva: &mut crate::Cva,
    set: &DreamCandidateSet,
) -> Result<bool, ReliquaryRuntimeHostError> {
    if cva
        .memory(set.source.memory.id)
        .map_err(operation)?
        .revision
        != set.source.memory.revision
    {
        return Ok(false);
    }
    for candidate in &set.candidates {
        if cva
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

fn user_snapshot_is_current(
    phy: &mut crate::Phylactery,
    set: &DreamCandidateSet,
) -> Result<bool, ReliquaryRuntimeHostError> {
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
