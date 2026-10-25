use super::{WorkerShared, operation};
use crate::runtime_shared::unix_now_ns;
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
        let Some(epoch) = crate::dream_cooldown::owner_eligible_epoch(
            crate::memory_store::MemoryReadView::new(&phy.owner.memories, &phy.owner.container),
            None,
            &phy.owner.processing_epochs,
            id,
            unix_now_ns(),
        )
        .map_err(operation)?
        else {
            continue;
        };
        let body = phy.memory_body_id(id).map_err(operation)?;
        if phy.memory_vector_location(profile, body).is_none() {
            continue;
        }
        let work = crate::dream_owner_candidates::prepare_dream_candidates(
            &crate::memory_store::MemoryReadView::new(&phy.owner.memories, &phy.owner.container),
            &phy.owner.graph,
            &phy.owner.dream_pairs,
            &crate::memory_vector_reader::MemoryVectorReader::new(
                phy.owner.memory_vector_read_view(),
            ),
            profile,
            id,
            DreamCandidateConfig::default(),
        )
        .map_err(operation)?;
        let source_times = work
            .source_time_memories()
            .map(|memory| {
                (
                    memory.id,
                    crate::memory_source_time::memory_source_timestamp_ns(memory),
                )
            })
            .collect();
        let candidates =
            crate::dream_owner_candidates::finish_dream_candidates(work, &source_times);
        return Ok(Some(Snapshot {
            candidates,
            graph_version: phy.graph_version(),
            dream_epoch: epoch,
        }));
    }
    Ok(None)
}

fn current(
    phy: &mut crate::phy_execution_owner::PhyExecutionOwner,
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

fn publish_pair(
    phy: &mut crate::phy_execution_owner::PhyExecutionOwner,
    classification: &DreamPairClassification,
    verification: Option<&DreamPairVerification>,
    policy: DreamVerificationPolicy,
    graph_version: u64,
) -> Result<crate::DreamPublicationOutcome, crate::DreamPublicationError> {
    crate::dream_publication_execution::publish_dream_pair(
        crate::owner_lifecycle::DreamGraphPublication::new(
            &mut phy.owner.container,
            &phy.owner.memories,
            &mut phy.owner.graph,
        ),
        &mut phy.duplicate_index,
        classification,
        verification,
        policy,
        graph_version,
        crate::dream_publication_execution::DreamSourceTime::Phylactery,
    )
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
        publish_pair(
            &mut phy,
            &classification,
            verification.as_ref(),
            policy,
            version,
        )
        .map_err(operation)?;
    }
    let source = snapshot.candidates.source.memory.id;
    {
        let state = &mut phy.owner;
        let graph = &state.graph;
        let mut memories =
            crate::memory_store::MemoryOwnerWriter::new(&mut state.memories, &mut state.container);
        crate::dream_owner_publisher::reconcile_phy_lifecycle(&mut memories, graph, source)
            .map_err(operation)?;
    }
    {
        let phy = &mut *phy;
        phy.owner
            .processing_epochs
            .put_for_memory(
                &mut phy.owner.container,
                &phy.owner.memories,
                source,
                crate::processing_epoch_model::DREAM_PROCESSING_LANE,
                crate::dream_cooldown::dream_processing_state(snapshot.dream_epoch, unix_now_ns()),
            )
            .map_err(operation)?;
    }
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
    let resolutions = crate::entity_resolution_owner::ResolutionReadContext {
        memories: &phy.owner.memories,
        entity_resolutions: &phy.owner.entity_resolutions,
    };
    let keys = ids
        .into_iter()
        .flat_map(|id| resolutions.schedulable_entity_mentions_for_memory(id))
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
        if !shared.background_open()? {
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
