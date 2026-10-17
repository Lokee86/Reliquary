use super::{LeaseGate, ReliquaryRuntimeHostError, ReliquaryRuntimeRoutes};
use crate::{CompatibilityProfile, CompatibilityProfileId, Phylactery};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[path = "runtime_host_phy_dream.rs"]
mod dream;
#[path = "runtime_host_phy_perception.rs"]
mod perception;
#[cfg(test)]
#[path = "runtime_host_phy_execution_tests.rs"]
mod tests;
#[path = "runtime_host_phy_vectors.rs"]
mod vectors;

const IDLE_POLL: Duration = Duration::from_secs(1);

#[derive(Default)]
struct Control {
    stop: bool,
    epoch: u64,
}

#[derive(Default)]
struct MentionQueue {
    order: VecDeque<crate::MemoryEntityMentionKey>,
    queued: HashSet<crate::MemoryEntityMentionKey>,
    deferred: HashMap<crate::MemoryEntityMentionKey, Instant>,
    in_flight: usize,
}

impl MentionQueue {
    fn push(&mut self, key: crate::MemoryEntityMentionKey) {
        if self.queued.insert(key) {
            self.order.push_back(key);
        }
    }

    fn pop_ready(&mut self, now: Instant) -> Option<crate::MemoryEntityMentionKey> {
        let count = self.order.len();
        for _ in 0..count {
            let key = self.order.pop_front()?;
            if self.deferred.get(&key).is_some_and(|until| *until > now) {
                self.order.push_back(key);
                continue;
            }
            self.deferred.remove(&key);
            self.queued.remove(&key);
            self.in_flight += 1;
            return Some(key);
        }
        None
    }

    fn finish(&mut self) {
        self.in_flight = self.in_flight.saturating_sub(1);
    }

    fn defer(&mut self, key: crate::MemoryEntityMentionKey) {
        self.push(key);
        self.deferred
            .insert(key, Instant::now() + Duration::from_secs(60));
    }

    fn len(&self) -> usize {
        self.queued.len() + self.in_flight
    }
}

pub(super) struct WorkerShared {
    pub(super) owner: Arc<Mutex<Phylactery>>,
    pub(super) routes: Arc<RwLock<ReliquaryRuntimeRoutes>>,
    pub(super) fence: Arc<LeaseGate>,
    control: Arc<(Mutex<Control>, Condvar)>,
    pub(super) background_enabled: AtomicBool,
    pub(super) profile: Mutex<Option<CompatibilityProfile>>,
    perception: Mutex<MentionQueue>,
}

impl WorkerShared {
    pub(super) fn stopped(&self) -> Result<bool, ReliquaryRuntimeHostError> {
        self.control
            .0
            .lock()
            .map(|c| c.stop)
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)
    }

    pub(super) fn background_open(&self) -> Result<bool, ReliquaryRuntimeHostError> {
        Ok(self.background_enabled.load(Ordering::SeqCst) && self.admission_open()?)
    }

    pub(super) fn admission_open(&self) -> Result<bool, ReliquaryRuntimeHostError> {
        Ok(self.fence.snapshot()?.0 == super::OwnerAdmission::Open)
    }

    pub(super) fn wake(&self) -> Result<(), ReliquaryRuntimeHostError> {
        let mut c = self
            .control
            .0
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        c.epoch = c.epoch.wrapping_add(1);
        self.control.1.notify_all();
        Ok(())
    }

    pub(super) fn wait(
        &self,
        seen: &mut u64,
        timeout: Duration,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let mut c = self
            .control
            .0
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if c.epoch == *seen && !c.stop {
            let (next, _) = self
                .control
                .1
                .wait_timeout(c, timeout)
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            c = next;
        }
        *seen = c.epoch;
        Ok(())
    }

    /// Acquires a fresh owner-local lease for each bounded mutation. Fencing is
    /// expected during shutdown/quiesce and is therefore a harmless no-op.
    pub(super) fn lease(&self) -> Result<Option<super::LeaseGuard>, ReliquaryRuntimeHostError> {
        if self.stopped()? || !self.admission_open()? {
            return Ok(None);
        }
        match Arc::clone(&self.fence).acquire_current() {
            Ok(lease) => Ok(Some(lease)),
            Err(ReliquaryRuntimeHostError::Operation(message))
                if message == "runtime admission is fenced" =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    pub(super) fn enqueue(
        &self,
        keys: impl IntoIterator<Item = crate::MemoryEntityMentionKey>,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let mut q = self
            .perception
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        for key in keys {
            q.push(key);
        }
        Ok(())
    }
}

pub(super) struct PhyRuntime {
    shared: Arc<WorkerShared>,
    workers: Vec<JoinHandle<Result<(), ReliquaryRuntimeHostError>>>,
}

impl PhyRuntime {
    pub(super) fn start(
        owner: Arc<Mutex<Phylactery>>,
        routes: Arc<RwLock<ReliquaryRuntimeRoutes>>,
        fence: Arc<LeaseGate>,
    ) -> Result<Self, ReliquaryRuntimeHostError> {
        let shared = Arc::new(WorkerShared {
            owner,
            routes,
            fence,
            control: Arc::new((
                Mutex::new(Control {
                    stop: false,
                    epoch: 1,
                }),
                Condvar::new(),
            )),
            background_enabled: AtomicBool::new(true),
            profile: Mutex::new(None),
            perception: Mutex::new(MentionQueue::default()),
        });
        // Seed before any worker can race the durable startup scan.
        perception::seed(&shared)?;
        let mut runtime = Self {
            shared,
            workers: Vec::new(),
        };
        runtime.spawn_workers()?;
        Ok(runtime)
    }

    fn spawn_workers(&mut self) -> Result<(), ReliquaryRuntimeHostError> {
        if !self.workers.is_empty() {
            return Ok(());
        }
        {
            let mut c = self
                .shared
                .control
                .0
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            c.stop = false;
            c.epoch = c.epoch.wrapping_add(1);
            self.shared.control.1.notify_all();
        }
        for worker in [
            vectors::worker_loop as fn(Arc<WorkerShared>) -> Result<(), ReliquaryRuntimeHostError>,
            dream::worker_loop,
            perception::worker_loop,
        ] {
            self.workers.push(thread::spawn({
                let shared = Arc::clone(&self.shared);
                move || worker(shared)
            }));
        }
        Ok(())
    }

    pub(super) fn stop_workers(&mut self) -> Result<(), ReliquaryRuntimeHostError> {
        {
            let mut c = self
                .shared
                .control
                .0
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            c.stop = true;
            c.epoch = c.epoch.wrapping_add(1);
            self.shared.control.1.notify_all();
        }
        let mut failure = None;
        for worker in self.workers.drain(..) {
            match worker.join() {
                Err(_) if failure.is_none() => {
                    failure = Some(ReliquaryRuntimeHostError::ThreadPanicked)
                }
                Ok(Err(error)) if failure.is_none() => failure = Some(error),
                _ => {}
            }
        }
        failure.map_or(Ok(()), Err)
    }

    /// Rebuilds startup-derived personal queue work while workers are stopped.
    /// Existing pending queue items and a still-valid compatibility profile are
    /// retained; seed is deduplicating and performs no provider work.
    pub(super) fn reconcile(&mut self) -> Result<(), ReliquaryRuntimeHostError> {
        if !self.workers.is_empty() {
            return Err(ReliquaryRuntimeHostError::Operation(
                "PHY reconciliation requires stopped workers".into(),
            ));
        }
        let known_profiles = self
            .shared
            .owner
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .active_memory_compatibility_profiles()
            .into_iter()
            .map(|profile| profile.id)
            .collect::<std::collections::HashSet<_>>();
        let mut profile = self
            .shared
            .profile
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if profile
            .as_ref()
            .is_some_and(|value| !known_profiles.contains(&value.id))
        {
            *profile = None;
        }
        drop(profile);
        perception::seed(&self.shared)
    }

    pub(super) fn restart_workers(&mut self) -> Result<(), ReliquaryRuntimeHostError> {
        if !self.workers.is_empty() {
            return Ok(());
        }
        if self.shared.fence.snapshot()?.0 != super::OwnerAdmission::Open {
            return Err(ReliquaryRuntimeHostError::Operation(
                "PHY runtime fence is closed".into(),
            ));
        }
        self.reconcile()?;
        self.spawn_workers()
    }

    pub(super) fn wake(&self) -> Result<(), ReliquaryRuntimeHostError> {
        self.shared.wake()
    }

    pub(super) fn workers_stopped(&self) -> bool {
        self.workers.is_empty()
    }
    pub(super) fn control_handle(&self) -> Arc<WorkerShared> {
        Arc::clone(&self.shared)
    }

    pub(super) fn worker_state(&self) -> (usize, bool) {
        (
            self.workers
                .iter()
                .filter(|worker| !worker.is_finished())
                .count(),
            self.shared.background_enabled.load(Ordering::SeqCst),
        )
    }

    pub(super) fn worker_count(&self) -> usize {
        self.workers.len()
    }

    #[cfg(test)]
    pub(super) fn wake_epoch_for_test(&self) -> u64 {
        self.shared.control.0.lock().unwrap().epoch
    }

    pub(super) fn memory_profile(
        &self,
    ) -> Result<Option<CompatibilityProfileId>, ReliquaryRuntimeHostError> {
        self.shared
            .profile
            .lock()
            .map(|profile| profile.as_ref().map(|profile| profile.id))
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)
    }

    pub(super) fn perception_pending(&self) -> Result<usize, ReliquaryRuntimeHostError> {
        self.shared
            .perception
            .lock()
            .map(|queue| queue.len())
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)
    }
}

impl Drop for PhyRuntime {
    fn drop(&mut self) {
        let _ = self.stop_workers();
    }
}

pub(super) fn operation(error: impl std::fmt::Display) -> ReliquaryRuntimeHostError {
    ReliquaryRuntimeHostError::Operation(error.to_string())
}
