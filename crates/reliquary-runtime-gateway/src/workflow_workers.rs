//! Concrete specialty workers over authenticated Application workflows.

use super::*;
use crate::owner_workers::{OwnerWorkerDriver, OwnerWorkerPoll};
use reliquary_application::{
    DreamOrganizationDraft, EntityMemoryMention, FreshnessTransition, SwitchboardEmbeddingEndpoint,
};
use reliquary_owner_session::model::{CompatibilityProfileId, MemoryId};
use reliquary_switchboard::{EmbeddingEndpoint, GeneralEndpoint};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub struct OwnerWorkflowSet {
    drivers: Vec<Arc<dyn OwnerWorkerDriver>>,
}
impl OwnerWorkflowSet {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn dream(mut self, worker: DreamWorker) -> Self {
        self.drivers.push(Arc::new(worker));
        self
    }
    pub fn entity(mut self, worker: EntityWorker) -> Self {
        self.drivers.push(Arc::new(worker));
        self
    }
    pub fn vector(mut self, worker: VectorWorker) -> Self {
        self.drivers.push(Arc::new(worker));
        self
    }
    pub fn freshness(mut self, worker: Arc<FreshnessWorker>) -> Self {
        self.drivers.push(worker);
        self
    }
}

impl Gateway {
    pub fn start_owner_workflows(
        &self,
        instance: Arc<Instance>,
        owner: OwnerId,
        workflows: OwnerWorkflowSet,
        config: OwnerWorkerConfig,
    ) -> Result<OwnerWorkerCoordinator, GatewayError> {
        self.start_owner_workers(instance, owner, workflows.drivers, config)
    }
}

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
        schema: &serde_json::Value,
    ) -> Result<serde_json::Value, reliquary_switchboard::GeneralEndpointError> {
        self.0.complete_json(system, payload, name, schema)
    }
}

pub struct DreamWorker {
    instance: Arc<Instance>,
    owner: OwnerId,
    publication_id: [u8; 32],
    draft: Arc<DreamOrganizationDraft>,
    updated_at_ns: i64,
    complete: AtomicBool,
}
impl DreamWorker {
    pub fn new(
        instance: Arc<Instance>,
        owner: OwnerId,
        publication_id: [u8; 32],
        draft: DreamOrganizationDraft,
        updated_at_ns: i64,
    ) -> Self {
        Self {
            instance,
            owner,
            publication_id,
            draft: Arc::new(draft),
            updated_at_ns,
            complete: AtomicBool::new(false),
        }
    }
}
impl OwnerWorkerDriver for DreamWorker {
    fn kind(&self) -> OwnerWorkerKind {
        OwnerWorkerKind::Dream
    }
    fn poll(&self) -> Result<OwnerWorkerPoll, String> {
        if self.complete.load(Ordering::Acquire) {
            return Ok(OwnerWorkerPoll::Idle);
        }
        self.instance
            .publish_dream_batch(
                self.owner,
                self.publication_id,
                &self.draft,
                self.updated_at_ns,
            )
            .map_err(|error| error.to_string())?;
        self.complete.store(true, Ordering::Release);
        Ok(OwnerWorkerPoll::Progress)
    }
}

pub struct EntityWorker {
    instance: Arc<Instance>,
    input: EntityMemoryMention,
    endpoint: Arc<dyn GeneralEndpoint>,
    updated_at_ns: i64,
    complete: AtomicBool,
}
impl EntityWorker {
    pub fn new(
        instance: Arc<Instance>,
        input: EntityMemoryMention,
        endpoint: Arc<dyn GeneralEndpoint>,
        updated_at_ns: i64,
    ) -> Self {
        Self {
            instance,
            input,
            endpoint,
            updated_at_ns,
            complete: AtomicBool::new(false),
        }
    }
}
impl OwnerWorkerDriver for EntityWorker {
    fn kind(&self) -> OwnerWorkerKind {
        OwnerWorkerKind::Entity
    }
    fn poll(&self) -> Result<OwnerWorkerPoll, String> {
        if self.complete.load(Ordering::Acquire) {
            return Ok(OwnerWorkerPoll::Idle);
        }
        self.instance
            .resolve_and_publish_entity_mention(
                self.input.clone(),
                SharedGeneral(Arc::clone(&self.endpoint)),
                self.updated_at_ns,
            )
            .map_err(|error| error.to_string())?;
        self.complete.store(true, Ordering::Release);
        Ok(OwnerWorkerPoll::Progress)
    }
}

pub struct VectorWorker {
    instance: Arc<Instance>,
    owner: OwnerId,
    profile: CompatibilityProfileId,
    memories: Vec<MemoryId>,
    endpoint: Arc<dyn EmbeddingEndpoint>,
    complete: AtomicBool,
}
impl VectorWorker {
    pub fn new(
        instance: Arc<Instance>,
        owner: OwnerId,
        profile: CompatibilityProfileId,
        memories: Vec<MemoryId>,
        endpoint: Arc<dyn EmbeddingEndpoint>,
    ) -> Self {
        Self {
            instance,
            owner,
            profile,
            memories,
            endpoint,
            complete: AtomicBool::new(false),
        }
    }
}
impl OwnerWorkerDriver for VectorWorker {
    fn kind(&self) -> OwnerWorkerKind {
        OwnerWorkerKind::Vector
    }
    fn poll(&self) -> Result<OwnerWorkerPoll, String> {
        if self.complete.load(Ordering::Acquire) {
            return Ok(OwnerWorkerPoll::Idle);
        }
        let endpoint = SwitchboardEmbeddingEndpoint::new(Arc::clone(&self.endpoint));
        let prepared = self
            .instance
            .prepare_owner_memory_vectors(self.owner, self.profile, &self.memories, &endpoint)
            .map_err(|error| error.to_string())?;
        self.instance
            .publish_owner_memory_vectors(&prepared)
            .map_err(|error| error.to_string())?;
        self.complete.store(true, Ordering::Release);
        Ok(OwnerWorkerPoll::Progress)
    }
}

pub struct FreshnessWorker {
    instance: Arc<Instance>,
    owner: OwnerId,
    limit: usize,
    last_turn: Mutex<Option<u64>>,
    notices: Mutex<Vec<FreshnessTransition>>,
}
impl FreshnessWorker {
    pub fn new(instance: Arc<Instance>, owner: OwnerId, limit: usize) -> Self {
        Self {
            instance,
            owner,
            limit,
            last_turn: Mutex::new(None),
            notices: Mutex::new(Vec::new()),
        }
    }
    pub fn transitions(&self) -> Result<Vec<FreshnessTransition>, String> {
        self.notices
            .lock()
            .map(|value| value.clone())
            .map_err(|_| "Freshness notice lock poisoned".into())
    }
    pub fn evaluated_turn(&self) -> Result<Option<u64>, String> {
        self.last_turn
            .lock()
            .map(|value| *value)
            .map_err(|_| "Freshness turn lock poisoned".into())
    }
}
impl OwnerWorkerDriver for FreshnessWorker {
    fn kind(&self) -> OwnerWorkerKind {
        OwnerWorkerKind::Freshness
    }
    fn poll(&self) -> Result<OwnerWorkerPoll, String> {
        let snapshot = self
            .instance
            .retrieval_snapshot(self.owner, None)
            .map_err(|error| error.to_string())?;
        let mut last = self
            .last_turn
            .lock()
            .map_err(|_| "Freshness turn lock poisoned".to_string())?;
        if *last == Some(snapshot.rel_turn) {
            return Ok(OwnerWorkerPoll::Idle);
        }
        let notices = self
            .instance
            .due_freshness_transitions(self.owner, self.limit)
            .map_err(|error| error.to_string())?;
        *self
            .notices
            .lock()
            .map_err(|_| "Freshness notice lock poisoned".to_string())? = notices;
        *last = Some(snapshot.rel_turn);
        Ok(OwnerWorkerPoll::Progress)
    }
}

#[cfg(test)]
#[path = "workflow_workers/tests.rs"]
mod tests;
