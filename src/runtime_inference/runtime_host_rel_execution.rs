//! Gateway REL execution: one native worker set for each mounted REL owner.
use super::super::{
    Control, DREAM_RETRY_POLL, IDLE_POLL, ReliquaryRuntimeHostError, ReliquaryRuntimeRoutes,
    VECTOR_RETRY_POLL, notify_work, operation, stopped, wait_for_work, wait_for_work_timeout,
};
use super::{LeaseGate, LeaseGuard};
use crate::{CompatibilityProfileId, EpisodePolicy, InsomniaWorkerConfig, InteractionRuntime};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{self, JoinHandle};

#[path = "runtime_host_rel_dream.rs"]
mod dream;
#[path = "runtime_host_rel_insomnia.rs"]
mod insomnia;
#[path = "runtime_host_rel_perception.rs"]
mod perception;
#[path = "runtime_host_rel_vectors.rs"]
mod vectors;
use perception::RelPerceptionQueue;

#[path = "runtime_host_gateway_background.rs"]
mod background;

pub(super) struct RelShared {
    pub(super) runtime: Arc<Mutex<InteractionRuntime>>,
    pub(super) sequencer: Arc<super::events::RelSequencer>,
    pub(super) routes: Arc<RwLock<ReliquaryRuntimeRoutes>>,
    pub(super) signal: Arc<(Mutex<Control>, Condvar)>,
    pub(super) fence: Arc<LeaseGate>,
    pub(super) memory_profile: Arc<Mutex<Option<CompatibilityProfileId>>>,
    pub(super) perception_queue: Arc<Mutex<RelPerceptionQueue>>,
    pub(super) insomnia_enabled: Arc<AtomicBool>,
    pub(super) background_enabled: AtomicBool,
    pub(super) insomnia_backpressure_until_ns: AtomicI64,
    pub(super) config: InsomniaWorkerConfig,
    pub(super) episode_policy: EpisodePolicy,
    pub(super) publication: Arc<super::publication::PublicationCoordinator>,
}

impl RelShared {
    // Caller holds the existing owner admission lease; provider I/O is forbidden.
    pub(super) fn mutate<T>(
        &self,
        family: super::mutation_events::RelMutationFamily,
        action: impl FnOnce(
            &mut InteractionRuntime,
        ) -> Result<
            (T, bool, Vec<super::events::RelInvalidation>),
            ReliquaryRuntimeHostError,
        >,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        let mut gate = self.sequencer.current_gate()?;
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        super::mutation_events::commit(&mut gate, &mut runtime, family, action)
    }

    pub(super) fn background_open(&self) -> Result<bool, ReliquaryRuntimeHostError> {
        Ok(!stopped(&self.signal)?
            && self
                .background_enabled
                .load(std::sync::atomic::Ordering::SeqCst)
            && self.fence.snapshot()?.0 == super::OwnerAdmission::Open)
    }

    pub(super) fn admit(&self) -> Result<Option<LeaseGuard>, ReliquaryRuntimeHostError> {
        if stopped(&self.signal)? {
            return Ok(None);
        }
        match self.fence.acquire_current() {
            Ok(lease) => Ok(Some(lease)),
            Err(error) => {
                if self.fence.snapshot()?.0 == super::OwnerAdmission::Closed {
                    Ok(None)
                } else {
                    Err(error)
                }
            }
        }
    }
}

pub(super) struct RelRuntime {
    pub(super) runtime: Option<Arc<Mutex<InteractionRuntime>>>,
    pub(super) signal: Arc<(Mutex<Control>, Condvar)>,
    pub(super) insomnia_enabled: Arc<AtomicBool>,
    pub(super) memory_profile: Arc<Mutex<Option<CompatibilityProfileId>>>,
    pub(super) perception_queue: Arc<Mutex<RelPerceptionQueue>>,
    shared: Arc<RelShared>,
    workers: Vec<JoinHandle<Result<(), ReliquaryRuntimeHostError>>>,
}

impl RelRuntime {
    pub(super) fn start(
        runtime: InteractionRuntime,
        sequencer: Arc<super::events::RelSequencer>,
        routes: Arc<RwLock<ReliquaryRuntimeRoutes>>,
        fence: Arc<LeaseGate>,
        config: InsomniaWorkerConfig,
        episode_policy: EpisodePolicy,
        publication: Arc<super::publication::PublicationCoordinator>,
    ) -> Self {
        let runtime = Arc::new(Mutex::new(runtime));
        let signal = Arc::new((
            Mutex::new(Control {
                stop: false,
                epoch: 1,
            }),
            Condvar::new(),
        ));
        let insomnia_enabled = Arc::new(AtomicBool::new(true));
        let shared = Arc::new(RelShared {
            runtime: Arc::clone(&runtime),
            sequencer,
            routes,
            signal: Arc::clone(&signal),
            fence,
            memory_profile: Arc::new(Mutex::new(None)),
            perception_queue: Arc::new(Mutex::new(RelPerceptionQueue::default())),
            insomnia_enabled: Arc::clone(&insomnia_enabled),
            background_enabled: AtomicBool::new(true),
            insomnia_backpressure_until_ns: AtomicI64::new(0),
            config,
            episode_policy,
            publication,
        });
        let mut execution = Self {
            runtime: Some(runtime),
            signal,
            insomnia_enabled,
            memory_profile: Arc::clone(&shared.memory_profile),
            perception_queue: Arc::clone(&shared.perception_queue),
            shared,
            workers: Vec::new(),
        };
        execution.spawn_workers();
        execution
    }

    pub(super) fn routes_changed(&self) -> Result<(), ReliquaryRuntimeHostError> {
        self.shared
            .insomnia_backpressure_until_ns
            .store(0, Ordering::SeqCst);
        notify_work(&self.signal)
    }

    fn spawn_workers(&mut self) {
        if !self.workers.is_empty() {
            return;
        }
        for index in 0..self.shared.config.workers {
            let shared = Arc::clone(&self.shared);
            self.workers
                .push(thread::spawn(move || insomnia::worker_loop(shared, index)));
        }
        for worker in [
            vectors::worker_loop as fn(Arc<RelShared>) -> Result<(), ReliquaryRuntimeHostError>,
            dream::worker_loop as fn(Arc<RelShared>) -> Result<(), ReliquaryRuntimeHostError>,
            perception::worker_loop as fn(Arc<RelShared>) -> Result<(), ReliquaryRuntimeHostError>,
        ] {
            let shared = Arc::clone(&self.shared);
            self.workers.push(thread::spawn(move || worker(shared)));
        }
    }

    #[cfg(test)]
    pub(super) fn start_insomnia_for_test(&mut self) {
        let shared = Arc::clone(&self.shared);
        self.workers
            .push(thread::spawn(move || insomnia::worker_loop(shared, 0)));
    }

    #[cfg(test)]
    pub(super) fn replay_claim_for_test(
        &self,
        claim: &crate::insomnia::RuntimeInsomniaClaim,
        endpoint: Option<Arc<dyn crate::GeneralEndpoint>>,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        insomnia::replay_claim_for_test(&self.shared, claim, endpoint)
    }

    #[cfg(test)]
    pub(super) fn reconcile_publication_for_test(&self) -> Result<(), ReliquaryRuntimeHostError> {
        insomnia::reconcile_completed(&self.shared)
    }

    pub(super) fn control_handle(&self) -> Arc<RelShared> {
        Arc::clone(&self.shared)
    }

    pub(super) fn worker_state(&self) -> (usize, bool) {
        (
            self.workers
                .iter()
                .filter(|worker| !worker.is_finished())
                .count(),
            self.shared
                .background_enabled
                .load(std::sync::atomic::Ordering::SeqCst),
        )
    }

    pub(super) fn workers_stopped(&self) -> bool {
        self.workers.is_empty()
    }

    pub(super) fn stop_workers(&mut self) -> Result<(), ReliquaryRuntimeHostError> {
        {
            let mut control = self
                .signal
                .0
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            control.stop = true;
            self.signal.1.notify_all();
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

    pub(super) fn restart_workers(&mut self) -> Result<(), ReliquaryRuntimeHostError> {
        if !self.workers.is_empty() {
            return Ok(());
        }
        if self.runtime.is_none() {
            return Err(operation("REL runtime is unavailable"));
        }
        if self.shared.fence.snapshot()?.0 != super::OwnerAdmission::Open {
            return Err(operation("REL runtime fence is closed"));
        }
        // A replacement/repack may retire profiles. Never expose a stale active profile.
        let known = self
            .shared
            .runtime
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .cva
            .compatibility_profiles()
            .into_iter()
            .map(|profile| profile.id)
            .collect::<std::collections::HashSet<_>>();
        let mut profile = self
            .memory_profile
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if profile.is_some_and(|id| !known.contains(&id)) {
            *profile = None;
        }
        drop(profile);
        {
            let mut control = self
                .signal
                .0
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            control.stop = false;
            control.epoch = control.epoch.wrapping_add(1);
            self.signal.1.notify_all();
        }
        self.spawn_workers();
        Ok(())
    }

    pub(super) fn into_cva(mut self) -> Result<crate::Cva, ReliquaryRuntimeHostError> {
        self.stop_workers()?;
        let runtime = self
            .runtime
            .take()
            .ok_or_else(|| operation("REL runtime is unavailable"))?;
        // The stopped shared execution still owns one runtime reference.
        drop(self);
        let runtime = Arc::try_unwrap(runtime)
            .map_err(|_| operation("REL runtime still shared"))?
            .into_inner()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        Ok(runtime.into_cva())
    }
}

impl Drop for RelRuntime {
    fn drop(&mut self) {
        let _ = self.stop_workers();
    }
}
