//! Managed Perception work on the same owner lease and mount fences as live work.
mod poll;

use super::*;
use crate::instance_owner::RelMount;
use reliquary_owner_session::observation::ObservationId;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Condvar, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub use reliquary_application::perception_workflow::PerceptionRoutes;

const POLL_INTERVAL: Duration = Duration::from_millis(250);
const RETRY_INTERVALS: [Duration; 3] = [
    Duration::from_secs(2),
    Duration::from_secs(10),
    Duration::from_secs(60),
];

struct Admission {
    instance: Weak<Instance>,
    managed: Vec<OwnerId>,
}

struct Control {
    topology: Weak<RwLock<HashMap<OwnerId, MountedOwner>>>,
    leases: Arc<Mutex<BTreeSet<(OwnerId, u8)>>>,
    routes: RwLock<Option<Arc<PerceptionRoutes>>>,
    admissions: Mutex<Vec<Admission>>,
    wake: Condvar,
    stopping: AtomicBool,
}

pub struct PerceptionLifecycle {
    control: Arc<Control>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Gateway {
    pub fn perception_lifecycle(&self) -> PerceptionLifecycle {
        PerceptionLifecycle {
            control: Arc::new(Control {
                topology: Arc::downgrade(&self.owners),
                leases: self.owner_worker_leases.clone(),
                routes: RwLock::new(None),
                admissions: Mutex::new(Vec::new()),
                wake: Condvar::new(),
                stopping: AtomicBool::new(false),
            }),
            worker: Mutex::new(None),
        }
    }
}

impl PerceptionLifecycle {
    pub fn set_routes(&self, routes: Option<PerceptionRoutes>) -> Result<(), GatewayError> {
        let mut current = self
            .control
            .routes
            .write()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        *current = routes.map(Arc::new);
        self.control.wake.notify_one();
        Ok(())
    }

    pub fn admit(
        &self,
        instance: &Arc<Instance>,
        mut managed: Vec<OwnerId>,
    ) -> Result<(), GatewayError> {
        if managed.is_empty() {
            return Ok(());
        }
        if !self
            .control
            .topology
            .ptr_eq(&Arc::downgrade(&instance.owners))
        {
            return Err(GatewayError::HostAdapter(
                "Perception Instance belongs to another Gateway".into(),
            ));
        }
        managed.sort_unstable();
        managed.dedup();
        for &owner in &managed {
            instance.authorize_rel_access(owner)?;
        }
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let mut admissions = self
            .control
            .admissions
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        if worker.is_none() {
            let control = self.control.clone();
            *worker = Some(
                thread::Builder::new()
                    .name("reliquary-perception".into())
                    .spawn(move || run(&control))
                    .map_err(|error| GatewayError::HostAdapter(error.to_string()))?,
            );
        }
        let instance = Arc::downgrade(instance);
        if let Some(admission) = admissions.iter_mut().find(|a| a.instance.ptr_eq(&instance)) {
            admission.managed = managed;
        } else {
            admissions.push(Admission { instance, managed });
        }
        self.control.wake.notify_one();
        Ok(())
    }
}

impl Drop for PerceptionLifecycle {
    fn drop(&mut self) {
        self.control.stopping.store(true, Ordering::Release);
        self.control.wake.notify_all();
        if let Ok(worker) = self.worker.get_mut()
            && let Some(worker) = worker.take()
        {
            let _ = worker.join();
        }
    }
}

#[derive(Default)]
struct WorkerState {
    review_after: Option<ObservationId>,
    review_cut: Option<(u64, u64, u64)>,
    review_exhausted: bool,
    review_deferred: BTreeMap<ObservationId, Instant>,
    ego_candidates: Vec<reliquary_application::perception_workflow::EgoCandidate>,
    failures: usize,
    retry_at: Option<Instant>,
    route: Option<Weak<PerceptionRoutes>>,
    mount: Option<RelMount>,
}

fn run(control: &Arc<Control>) {
    let mut states = BTreeMap::<OwnerId, WorkerState>::new();
    while !control.stopping.load(Ordering::Acquire) {
        let routes = control.routes.read().ok().and_then(|routes| routes.clone());
        let mut progress = false;
        if let Some(routes) = routes {
            let admitted = collect_admissions(control);
            states.retain(|id, _| admitted.contains_key(id));
            for (id, candidates) in admitted {
                if control.stopping.load(Ordering::Acquire) {
                    break;
                }
                let Some(instance) = candidates.into_iter().find_map(|candidate| {
                    let instance = candidate.upgrade()?;
                    instance.authorize_rel_access(id).ok()?;
                    Some(instance)
                }) else {
                    continue;
                };
                let Ok(mount @ RelMount::Write(_)) = RelMount::capture(&instance, id) else {
                    continue;
                };
                let state = states.entry(id).or_default();
                if Some(&mount) != state.mount.as_ref()
                    || !state
                        .route
                        .as_ref()
                        .is_some_and(|current| current.ptr_eq(&Arc::downgrade(&routes)))
                {
                    *state = WorkerState {
                        route: Some(Arc::downgrade(&routes)),
                        mount: Some(mount.clone()),
                        ..WorkerState::default()
                    };
                }
                if state.retry_at.is_some_and(|retry| Instant::now() < retry) {
                    continue;
                }
                let key = (id, OwnerWorkerKind::Perception.lease_id());
                let Some(_lease) = WorkLease::try_acquire(&control.leases, key) else {
                    continue;
                };
                let owner = OwnerWork {
                    instance: &instance,
                    control,
                    id,
                    mount,
                    routes: &routes,
                };
                match poll::tick(&owner, state) {
                    Ok(changed) => {
                        state.failures = 0;
                        state.retry_at = None;
                        progress |= changed;
                    }
                    Err(_) => {
                        let delay = RETRY_INTERVALS[state.failures.min(RETRY_INTERVALS.len() - 1)];
                        state.failures = state.failures.saturating_add(1);
                        state.retry_at = Some(Instant::now() + delay);
                    }
                }
            }
        }
        if !progress && let Ok(admissions) = control.admissions.lock() {
            let _guard = control.wake.wait_timeout(admissions, POLL_INTERVAL);
        }
    }
}

fn collect_admissions(control: &Control) -> BTreeMap<OwnerId, Vec<Weak<Instance>>> {
    let Ok(mut admissions) = control.admissions.lock() else {
        return BTreeMap::new();
    };
    admissions.retain(|a| a.instance.strong_count() != 0);
    let mut by_owner = BTreeMap::<_, Vec<_>>::new();
    for admission in admissions.iter() {
        for &owner in &admission.managed {
            by_owner
                .entry(owner)
                .or_default()
                .push(admission.instance.clone());
        }
    }
    by_owner
}

struct WorkLease<'a> {
    leases: &'a Mutex<BTreeSet<(OwnerId, u8)>>,
    key: (OwnerId, u8),
}

impl<'a> WorkLease<'a> {
    fn try_acquire(leases: &'a Mutex<BTreeSet<(OwnerId, u8)>>, key: (OwnerId, u8)) -> Option<Self> {
        if !leases.lock().ok()?.insert(key) {
            return None;
        }
        Some(Self { leases, key })
    }
}

impl Drop for WorkLease<'_> {
    fn drop(&mut self) {
        if let Ok(mut leases) = self.leases.lock() {
            leases.remove(&self.key);
        }
    }
}

struct OwnerWork<'a> {
    instance: &'a Instance,
    control: &'a Control,
    id: OwnerId,
    mount: RelMount,
    routes: &'a Arc<PerceptionRoutes>,
}

impl OwnerWork<'_> {
    fn read<R>(&self, action: impl FnOnce(&RelSession) -> R) -> Result<R, GatewayError> {
        self.with_writer(|writer| Ok(action(writer.read_session())))
    }

    fn with_writer<R>(
        &self,
        action: impl FnOnce(&mut RelWriter) -> Result<R, GatewayError>,
    ) -> Result<R, GatewayError> {
        let _topology = self
            .instance
            .rel_topology
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        if self.mount != RelMount::capture(self.instance, self.id)? {
            return Err(OwnerSessionError::ManagementStale.into());
        }
        self.instance.with_rel_writer(self.id, |writer| {
            let _publication = self
                .instance
                .credential_publication
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            self.instance.authorize_rel_access(self.id)?;
            let live = self
                .control
                .routes
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            if self.control.stopping.load(Ordering::Acquire)
                || live
                    .as_ref()
                    .is_none_or(|current| !Arc::ptr_eq(current, self.routes))
            {
                return Err(OwnerSessionError::ManagementStale.into());
            }
            action(writer)
        })?
    }
}
