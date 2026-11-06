//! Bounded lifecycle for owner-local Dream, Entity, Vector, and Freshness drivers.
//!
//! Specialty drivers own durable work identity and semantic policy. Gateway
//! owns only scheduling, owner/grant fences, quiescence, and joining. No owner
//! lock is held while a driver polls model, embedding, or other external work.

use super::*;
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const MAX_SPECIALTY_WORKERS: usize = 16;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OwnerWorkerKind {
    Dream,
    Entity,
    Vector,
    Freshness,
}
impl OwnerWorkerKind {
    const fn lease_id(self) -> u8 {
        match self {
            Self::Dream => 1,
            Self::Entity => 2,
            Self::Vector => 3,
            Self::Freshness => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerWorkerPoll {
    Progress,
    Idle,
}

/// One specialty workflow bound to durable owner evidence.
///
/// Implementations prepare external work without an owner mutation lock and
/// use authenticated Instance operations to revalidate before commit.
pub(crate) trait OwnerWorkerDriver: Send + Sync + 'static {
    fn kind(&self) -> OwnerWorkerKind;
    fn poll(&self) -> Result<OwnerWorkerPoll, String>;
}

#[derive(Clone, Debug)]
pub struct OwnerWorkerConfig {
    pub idle_poll: Duration,
}
impl Default for OwnerWorkerConfig {
    fn default() -> Self {
        Self {
            idle_poll: Duration::from_millis(250),
        }
    }
}

struct WorkerControl {
    stopping: AtomicBool,
    wait: (Mutex<()>, Condvar),
}

pub struct OwnerWorkerCoordinator {
    control: Arc<WorkerControl>,
    workers: Vec<JoinHandle<()>>,
    leases: Arc<Mutex<BTreeSet<(OwnerId, u8)>>>,
    lease_keys: Vec<(OwnerId, u8)>,
}

impl Gateway {
    /// Start one lifecycle lane per supplied specialty. Duplicate specialties
    /// are rejected so one owner has one scheduling authority for each kind.
    pub(crate) fn start_owner_workers(
        &self,
        instance: Arc<Instance>,
        owner: OwnerId,
        drivers: impl IntoIterator<Item = Arc<dyn OwnerWorkerDriver>>,
        config: OwnerWorkerConfig,
    ) -> Result<OwnerWorkerCoordinator, GatewayError> {
        if !Arc::ptr_eq(&self.owners, &instance.owners) {
            return Err(GatewayError::HostAdapter(
                "owner worker Instance belongs to another Gateway".into(),
            ));
        }
        if config.idle_poll.is_zero() {
            return Err(GatewayError::HostAdapter(
                "owner worker idle poll must be nonzero".into(),
            ));
        }
        require_writable_owner(&instance, owner)?;
        let drivers = drivers.into_iter().collect::<Vec<_>>();
        if drivers.is_empty() || drivers.len() > MAX_SPECIALTY_WORKERS {
            return Err(GatewayError::HostAdapter(
                "owner worker count is outside bounds".into(),
            ));
        }
        let mut kinds = BTreeSet::new();
        if drivers.iter().any(|driver| !kinds.insert(driver.kind())) {
            return Err(GatewayError::HostAdapter(
                "duplicate owner worker specialty".into(),
            ));
        }
        let lease_keys = kinds
            .into_iter()
            .map(|kind| (owner, kind.lease_id()))
            .collect::<Vec<_>>();
        {
            let mut leases = self
                .owner_worker_leases
                .lock()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            if lease_keys.iter().any(|key| leases.contains(key)) {
                return Err(GatewayError::HostAdapter(
                    "owner worker specialty is already active".into(),
                ));
            }
            leases.extend(lease_keys.iter().copied());
        }
        OwnerWorkerCoordinator::start(
            instance,
            owner,
            drivers,
            config,
            Arc::clone(&self.owner_worker_leases),
            lease_keys,
        )
    }
}

impl OwnerWorkerCoordinator {
    fn start(
        instance: Arc<Instance>,
        owner: OwnerId,
        drivers: Vec<Arc<dyn OwnerWorkerDriver>>,
        config: OwnerWorkerConfig,
        leases: Arc<Mutex<BTreeSet<(OwnerId, u8)>>>,
        lease_keys: Vec<(OwnerId, u8)>,
    ) -> Result<Self, GatewayError> {
        let control = Arc::new(WorkerControl {
            stopping: AtomicBool::new(false),
            wait: (Mutex::new(()), Condvar::new()),
        });
        let mut workers = Vec::with_capacity(drivers.len());
        for driver in drivers {
            let instance = Arc::clone(&instance);
            let thread_control = Arc::clone(&control);
            let config = config.clone();
            match thread::Builder::new()
                .name(format!("reliquary-{:?}", driver.kind()).to_lowercase())
                .spawn(move || worker_loop(instance, owner, driver, config, thread_control))
            {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    control.stopping.store(true, Ordering::Release);
                    control.wait.1.notify_all();
                    for worker in workers {
                        let _ = worker.join();
                    }
                    release_leases(&leases, &lease_keys);
                    return Err(GatewayError::HostAdapter(error.to_string()));
                }
            }
        }
        Ok(Self {
            control,
            workers,
            leases,
            lease_keys,
        })
    }

    /// Fence new polls and join every in-flight specialty operation.
    pub fn stop(&mut self) -> Result<(), GatewayError> {
        self.control.stopping.store(true, Ordering::Release);
        self.control.wait.1.notify_all();
        let mut panicked = false;
        for worker in self.workers.drain(..) {
            panicked |= worker.join().is_err();
        }
        release_leases(&self.leases, &self.lease_keys);
        self.lease_keys.clear();
        if panicked {
            Err(GatewayError::HostAdapter(
                "owner specialty worker panicked".into(),
            ))
        } else {
            Ok(())
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.control.stopping.load(Ordering::Acquire)
            && self.workers.iter().all(JoinHandle::is_finished)
    }
}

fn release_leases(leases: &Mutex<BTreeSet<(OwnerId, u8)>>, keys: &[(OwnerId, u8)]) {
    if let Ok(mut leases) = leases.lock() {
        for key in keys {
            leases.remove(key);
        }
    }
}
impl Drop for OwnerWorkerCoordinator {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn worker_loop(
    instance: Arc<Instance>,
    owner: OwnerId,
    driver: Arc<dyn OwnerWorkerDriver>,
    config: OwnerWorkerConfig,
    control: Arc<WorkerControl>,
) {
    while !control.stopping.load(Ordering::Acquire) {
        if require_writable_owner(&instance, owner).is_err() {
            wait(&control, config.idle_poll);
            continue;
        }
        let idle = !matches!(driver.poll(), Ok(OwnerWorkerPoll::Progress));
        let authorized = require_writable_owner(&instance, owner).is_ok();
        if idle || !authorized {
            wait(&control, config.idle_poll);
        }
    }
}

fn require_writable_owner(instance: &Instance, owner: OwnerId) -> Result<(), GatewayError> {
    if instance.rel_grants.contains(&owner) {
        instance.with_writable_reliquary(owner, |_| ())
    } else {
        instance.with_entity_phy_writer(owner, |_| ())
    }
}

fn wait(control: &WorkerControl, duration: Duration) {
    if let Ok(lock) = control.wait.0.lock() {
        let _ = control.wait.1.wait_timeout(lock, duration);
    }
}

#[cfg(test)]
mod tests;
