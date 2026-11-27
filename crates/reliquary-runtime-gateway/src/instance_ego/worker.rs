//! Managed Ego audit and publication, bound to an explicitly enrolled PHY.

use super::*;
use crate::{
    ego_audit_handoff::{EgoAuditRegistration, shared_ego_run_closed},
    instance_owner::RelMount,
    owner_workers::{OwnerWorkerDriver, OwnerWorkerPoll},
};
use reliquary_application::{
    ego_pending::{AcceptedEgoBatch, EgoBatchKey, EgoPendingWork, EgoRun, EgoRunKind},
    perception_workflow::{
        AUDIT_CONTRACT_VERSION as CONTRACT_VERSION, BatchLimits, EgoCandidate, OwnerCut,
    },
};
use reliquary_owner_session::{EgoCompletion, EgoSourceCut};
use reliquary_switchboard::{EmbeddingEndpoint, GeneralEndpoint};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::AtomicUsize,
    time::{Duration, Instant},
};

mod discovery;
mod publication;

type TokenCounter = dyn Fn(&str) -> u64 + Send + Sync;

pub struct EgoRoutes {
    pub general: Arc<dyn GeneralEndpoint>,
    pub embedding: Option<Arc<dyn EmbeddingEndpoint>>,
    pub identity: [u8; 32],
    pub embedding_profile: [u8; 32],
    pub count_tokens: Arc<TokenCounter>,
}

pub struct EgoWorkerHandle {
    coordinator: OwnerWorkerCoordinator,
    running: Arc<AtomicBool>,
    registration: Option<EgoAuditRegistration>,
}

impl EgoWorkerHandle {
    pub fn stop(&mut self) -> Result<(), GatewayError> {
        self.running.store(false, Ordering::Release);
        self.registration.take();
        self.coordinator.stop()
    }
}

impl Drop for EgoWorkerHandle {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub(super) struct EgoWorker {
    instance: Arc<Instance>,
    phy: OwnerId,
    phy_mount: Weak<MountedPhyWriter>,
    source_mounts: BTreeMap<OwnerId, RelMount>,
    owners: Vec<OwnerId>,
    pending: Mutex<EgoPendingWork>,
    running: Arc<AtomicBool>,
    routes: EgoRoutes,
    ticket: AtomicUsize,
    retry: Mutex<BTreeMap<OwnerId, (usize, Instant)>>,
}

impl Gateway {
    pub fn start_ego_worker(
        &self,
        instance: Arc<Instance>,
        phy: OwnerId,
        mut rels: Vec<OwnerId>,
        profile_dir: &Path,
        routes: EgoRoutes,
    ) -> Result<EgoWorkerHandle, GatewayError> {
        if !Arc::ptr_eq(&self.owners, &instance.owners) || !profile_dir.is_dir() {
            return Err(GatewayError::HostAdapter(
                "invalid Ego profile admission".into(),
            ));
        }
        let phy_mount = instance.ego_phy_writer_mount(phy)?;
        rels.sort_unstable();
        rels.dedup();
        let mut source_mounts = BTreeMap::new();
        for &rel in &rels {
            instance.authorize_rel_access(rel)?;
            source_mounts.insert(rel, RelMount::capture(&instance, rel)?);
        }
        let pending_path: PathBuf = profile_dir.join("ego-pending.sqlite3");
        let pending = EgoPendingWork::open(&pending_path)
            .map_err(|_| GatewayError::RetrievalUnavailable("Ego pending work unavailable"))?;
        let running = Arc::new(AtomicBool::new(true));
        let driver = Arc::new(EgoWorker {
            instance: instance.clone(),
            phy,
            phy_mount: Arc::downgrade(&phy_mount),
            source_mounts,
            owners: std::iter::once(phy).chain(rels.iter().copied()).collect(),
            pending: Mutex::new(pending),
            running: running.clone(),
            routes,
            ticket: AtomicUsize::new(0),
            retry: Mutex::new(BTreeMap::new()),
        });
        let registration = self
            .ego_audit_registry
            .admit(&instance, phy, &rels, &pending_path)?;
        let coordinator = self.start_owner_workers(
            instance,
            phy,
            [driver as Arc<dyn OwnerWorkerDriver>],
            OwnerWorkerConfig::default(),
        )?;
        Ok(EgoWorkerHandle {
            coordinator,
            running,
            registration: Some(registration),
        })
    }
}

impl OwnerWorkerDriver for EgoWorker {
    fn kind(&self) -> OwnerWorkerKind {
        OwnerWorkerKind::Ego
    }

    fn poll(&self) -> Result<OwnerWorkerPoll, String> {
        if !self.running.load(Ordering::Acquire) {
            return Ok(OwnerWorkerPoll::Idle);
        }
        let index = self.ticket.fetch_add(1, Ordering::Relaxed) % self.owners.len();
        let owner = self.owners[index];
        let mut retry = self
            .retry
            .lock()
            .map_err(|_| "Ego retry state unavailable")?;
        if retry
            .get(&owner)
            .is_some_and(|(_, next)| *next > Instant::now())
        {
            return Ok(OwnerWorkerPoll::Idle);
        }
        let result = self.poll_owner(owner);
        match &result {
            Ok(_) => {
                retry.remove(&owner);
            }
            Err(_) => {
                const BACKOFF: [u64; 3] = [2, 10, 60];
                let entry = retry.entry(owner).or_insert((0, Instant::now()));
                entry.1 = Instant::now() + Duration::from_secs(BACKOFF[entry.0]);
                entry.0 = (entry.0 + 1).min(BACKOFF.len() - 1);
            }
        }
        result.map(|progress| {
            if progress {
                OwnerWorkerPoll::Progress
            } else {
                OwnerWorkerPoll::Idle
            }
        })
    }
}

impl EgoWorker {
    fn still_authorized(&self, owner: OwnerId) -> Result<(), GatewayError> {
        if !self.running.load(Ordering::Acquire)
            || !self.phy_mount.ptr_eq(&Arc::downgrade(
                &self.instance.ego_phy_writer_mount(self.phy)?,
            ))
        {
            return Err(OwnerSessionError::ManagementStale.into());
        }
        if owner != self.phy {
            self.instance.authorize_rel_access(owner)?;
            if self.source_mounts.get(&owner) != Some(&RelMount::capture(&self.instance, owner)?) {
                return Err(OwnerSessionError::ManagementStale.into());
            }
        }
        Ok(())
    }

    fn poll_owner(&self, owner: OwnerId) -> Result<bool, String> {
        self.still_authorized(owner).map_err(as_error)?;
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "Ego pending work unavailable")?;
        let phy = self.phy.0;
        let source = owner.0;
        if owner != self.phy {
            for run in pending.pending_runs(phy, source).map_err(as_error)? {
                if matches!(
                    run.kind,
                    EgoRunKind::RelSharedBootstrap | EgoRunKind::RelRecurring
                ) && self
                    .instance
                    .with_reliquary(owner, |rel| shared_ego_run_closed(rel, run))
                    .map_err(as_error)?
                    .map_err(as_error)?
                {
                    pending.seal(run).map_err(as_error)?;
                }
            }
        }
        let ready = pending.ready_run(phy, source).map_err(as_error)?;
        if owner == self.phy {
            if let Some(run) = ready {
                return self.publish_phy(run, &mut pending);
            }
            return self.advance_phy(&mut pending);
        }

        let current_summary = self
            .instance
            .with_ego_phy_writer(self.phy, |writer| {
                Ok(writer
                    .ego_rel_summary(source)
                    .and_then(|summary| summary.completion.clone()))
            })
            .map_err(as_error)?;
        let history = self
            .instance
            .with_reliquary(owner, |rel| rel.perception_source_cut().history_generation)
            .map_err(as_error)?;
        let has_coverage = current_summary
            .as_ref()
            .is_some_and(|complete| complete.cut.history_generation == history);
        let ready_initial = ready.is_some_and(|run| run.kind == EgoRunKind::RelBootstrap);
        if !has_coverage && !ready_initial {
            return self.advance_rel_bootstrap(owner, &mut pending);
        }
        if let Some(run) = ready {
            return self.publish_rel(owner, run, &mut pending);
        }
        Ok(false)
    }

    fn new_run(
        &self,
        owner: OwnerId,
        generation: [u8; 32],
        memory: u64,
        settled: u64,
        kind: EgoRunKind,
    ) -> EgoRun {
        EgoRun {
            phy: self.phy.0,
            source: OwnerCut {
                owner_id: owner.0,
                history_generation: generation,
                frontier: settled,
            },
            kind,
            memory_version: memory,
            route: self.routes.identity,
            contract: CONTRACT_VERSION,
        }
    }
}

fn as_error(error: impl fmt::Display) -> String {
    error.to_string()
}

fn limits() -> BatchLimits {
    BatchLimits::default()
}

fn collect_batches(pending: &EgoPendingWork, run: EgoRun) -> Result<Vec<AcceptedEgoBatch>, String> {
    let mut after = 0;
    let mut batches = Vec::new();
    while let Some(batch) = pending.next_batch(run, after).map_err(as_error)? {
        after = batch.sequence;
        batches.push(batch);
    }
    Ok(batches)
}

fn completion(run: EgoRun, accepted: &[AcceptedEgoBatch]) -> EgoCompletion {
    EgoCompletion {
        id: run.id(),
        cut: EgoSourceCut {
            owner_id: run.source.owner_id,
            history_generation: run.source.history_generation,
            memory_version: run.memory_version,
            audit_frontier: run.source.frontier,
        },
        batch_ids: accepted.iter().map(|batch| batch.id).collect(),
    }
}
