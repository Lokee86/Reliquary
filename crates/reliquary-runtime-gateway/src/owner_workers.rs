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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerWorkerPoll {
    Progress,
    Idle,
}

/// One specialty workflow bound to durable owner evidence.
///
/// Implementations prepare external work without an owner mutation lock and
/// use authenticated Instance operations to revalidate before commit.
pub trait OwnerWorkerDriver: Send + Sync + 'static {
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
}

impl Gateway {
    /// Start one lifecycle lane per supplied specialty. Duplicate specialties
    /// are rejected so one owner has one scheduling authority for each kind.
    pub fn start_owner_workers(
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
        instance.with_writable_reliquary(owner, |_| ())?;
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
        OwnerWorkerCoordinator::start(instance, owner, drivers, config)
    }
}

impl OwnerWorkerCoordinator {
    fn start(
        instance: Arc<Instance>,
        owner: OwnerId,
        drivers: Vec<Arc<dyn OwnerWorkerDriver>>,
        config: OwnerWorkerConfig,
    ) -> Result<Self, GatewayError> {
        let control = Arc::new(WorkerControl {
            stopping: AtomicBool::new(false),
            wait: (Mutex::new(()), Condvar::new()),
        });
        let workers = drivers
            .into_iter()
            .map(|driver| {
                let instance = Arc::clone(&instance);
                let control = Arc::clone(&control);
                let config = config.clone();
                thread::Builder::new()
                    .name(format!("reliquary-{:?}", driver.kind()).to_lowercase())
                    .spawn(move || worker_loop(instance, owner, driver, config, control))
                    .map_err(|error| GatewayError::HostAdapter(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { control, workers })
    }

    /// Fence new polls and join every in-flight specialty operation.
    pub fn stop(&mut self) -> Result<(), GatewayError> {
        self.control.stopping.store(true, Ordering::Release);
        self.control.wait.1.notify_all();
        let mut panicked = false;
        for worker in self.workers.drain(..) {
            panicked |= worker.join().is_err();
        }
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
        if instance.with_writable_reliquary(owner, |_| ()).is_err() {
            wait(&control, config.idle_poll);
            continue;
        }
        let idle = !matches!(driver.poll(), Ok(OwnerWorkerPoll::Progress));
        let authorized = instance.with_writable_reliquary(owner, |_| ()).is_ok();
        if idle || !authorized {
            wait(&control, config.idle_poll);
        }
    }
}

fn wait(control: &WorkerControl, duration: Duration) {
    if let Ok(lock) = control.wait.0.lock() {
        let _ = control.wait.1.wait_timeout(lock, duration);
    }
}

#[cfg(test)]
mod tests;
