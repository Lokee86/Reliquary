//! Owner-local Insomnia worker lifecycle.
//!
//! Workers claim a bounded batch and receive Archive snapshots from the owner
//! session, then release the owner lock before invoking the model endpoint.
//! A synced extraction checkpoint is reused when a publication lease expires.
use crate::{Gateway, GatewayError, Instance, OwnerId};
pub(crate) mod processing;
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::Value;
use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_WORKERS: usize = 8;
const MAX_ADMISSIONS: usize = 128;

#[derive(Clone, Debug)]
pub struct InsomniaWorkerConfig {
    pub workers: usize,
    pub destination_phy_owner: Option<OwnerId>,
    pub max_admissions_per_claim: usize,
    pub lease_duration_ns: i64,
    pub idle_poll: Duration,
}
impl Default for InsomniaWorkerConfig {
    fn default() -> Self {
        Self {
            workers: 1,
            destination_phy_owner: None,
            max_admissions_per_claim: 16,
            lease_duration_ns: 120_000_000_000,
            idle_poll: Duration::from_millis(250),
        }
    }
}
impl InsomniaWorkerConfig {
    fn validate(&self) -> Result<(), GatewayError> {
        if self.workers == 0
            || self.workers > MAX_WORKERS
            || self.max_admissions_per_claim == 0
            || self.max_admissions_per_claim > MAX_ADMISSIONS
            || self.lease_duration_ns <= 0
            || self.idle_poll.is_zero()
        {
            return Err(GatewayError::HostAdapter(
                "invalid Insomnia worker bounds".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct SharedEndpoint(pub(crate) Arc<dyn GeneralEndpoint>);
impl GeneralEndpoint for SharedEndpoint {
    fn model(&self) -> &str {
        self.0.model()
    }
    fn complete_json(
        &self,
        system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.0
            .complete_json(system_prompt, user_payload, schema_name, schema)
    }
}

impl Gateway {
    /// Start a trusted model endpoint against an authenticated writable REL mount.
    pub fn start_insomnia_workers(
        &self,
        instance: Arc<Instance>,
        owner: OwnerId,
        endpoint: Arc<dyn GeneralEndpoint>,
        config: InsomniaWorkerConfig,
    ) -> Result<InsomniaWorkerHandle, GatewayError> {
        if !Arc::ptr_eq(&self.owners, &instance.owners) {
            return Err(GatewayError::HostAdapter(
                "Insomnia Instance belongs to another Gateway".into(),
            ));
        }
        instance.with_writable_reliquary(owner, |_| ())?;
        if let Some(phy) = config.destination_phy_owner {
            instance.authorize_routed_owners(owner, phy)?;
        }
        InsomniaWorkerHandle::start(instance, owner, endpoint, config)
    }
}

struct WorkerControl {
    stopped: AtomicBool,
    wait: (Mutex<()>, Condvar),
}

/// Joinable owner-local background worker set. `stop` requests quiescence and
/// waits for in-flight provider calls to return; those calls hold no owner lock.
pub struct InsomniaWorkerHandle {
    control: Arc<WorkerControl>,
    workers: Vec<JoinHandle<()>>,
}
impl InsomniaWorkerHandle {
    fn start(
        instance: Arc<Instance>,
        owner: OwnerId,
        endpoint: Arc<dyn GeneralEndpoint>,
        config: InsomniaWorkerConfig,
    ) -> Result<Self, GatewayError> {
        config.validate()?;
        let control = Arc::new(WorkerControl {
            stopped: AtomicBool::new(false),
            wait: (Mutex::new(()), Condvar::new()),
        });
        let mut workers = Vec::with_capacity(config.workers);
        for index in 0..config.workers {
            let instance = Arc::clone(&instance);
            let endpoint = SharedEndpoint(Arc::clone(&endpoint));
            let config = config.clone();
            let control = Arc::clone(&control);
            workers.push(thread::spawn(move || {
                worker_loop(instance, owner, endpoint, config, index, control)
            }));
        }
        Ok(Self { control, workers })
    }
    pub fn stop(&mut self) -> Result<(), GatewayError> {
        self.control.stopped.store(true, Ordering::Release);
        self.control.wait.1.notify_all();
        let mut panicked = false;
        for worker in self.workers.drain(..) {
            panicked |= worker.join().is_err();
        }
        if panicked {
            return Err(GatewayError::HostAdapter("Insomnia worker panicked".into()));
        }
        Ok(())
    }
    pub fn is_stopped(&self) -> bool {
        self.control.stopped.load(Ordering::Acquire)
            && self.workers.iter().all(JoinHandle::is_finished)
    }
}
impl Drop for InsomniaWorkerHandle {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn worker_loop(
    instance: Arc<Instance>,
    owner: OwnerId,
    endpoint: SharedEndpoint,
    config: InsomniaWorkerConfig,
    index: usize,
    control: Arc<WorkerControl>,
) {
    let worker_id = format!("gateway-insomnia-{}", index + 1);
    while !control.stopped.load(Ordering::Acquire) {
        let now = now_ns();
        let claim = match instance.claim_next_insomnia_input(
            owner,
            &worker_id,
            now,
            config.lease_duration_ns,
            config.max_admissions_per_claim,
        ) {
            Ok(Some(claim)) => claim,
            Ok(None) => {
                wait(&control, config.idle_poll);
                continue;
            }
            Err(_) => {
                wait(&control, config.idle_poll);
                continue;
            }
        };
        let _ = processing::process_claim(
            &instance,
            owner,
            &endpoint,
            config.destination_phy_owner,
            claim,
            None,
            None,
        );
    }
}

fn wait(control: &WorkerControl, duration: Duration) {
    if let Ok(lock) = control.wait.0.lock() {
        let _ = control.wait.1.wait_timeout(lock, duration);
    }
}
pub(crate) fn now_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_nanos()).ok())
        .unwrap_or(0)
}
#[cfg(test)]
mod tests;
