//! Owner-local Insomnia worker lifecycle.
//!
//! Workers claim a bounded batch and receive Archive snapshots from the owner
//! session, then release the owner lock before invoking the model endpoint.
//! Unsupported evidence and user-memory routes are durably Held with their
//! complete bounded model result; they are never silently filtered.
use crate::{GatewayError, Instance, OwnerId, RelInsomniaClaimInput};
use reliquary_application::{PreparedIngest, prepare_native_ingest_from_claim};
use reliquary_insomnia::{InsomniaExtraction, InsomniaModelExtractor};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Condvar, Mutex, atomic::{AtomicBool, Ordering}},
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_WORKERS: usize = 8;
const MAX_ADMISSIONS: usize = 128;
const CHECKPOINT_VERSION: u64 = 1;

#[derive(Clone, Debug)]
pub struct InsomniaWorkerConfig {
    pub workers: usize,
    pub max_admissions_per_claim: usize,
    pub lease_duration_ns: i64,
    pub idle_poll: Duration,
}
impl Default for InsomniaWorkerConfig {
    fn default() -> Self {
        Self { workers: 1, max_admissions_per_claim: 16, lease_duration_ns: 120_000_000_000, idle_poll: Duration::from_millis(250) }
    }
}
impl InsomniaWorkerConfig {
    fn validate(&self) -> Result<(), GatewayError> {
        if self.workers == 0 || self.workers > MAX_WORKERS
            || self.max_admissions_per_claim == 0 || self.max_admissions_per_claim > MAX_ADMISSIONS
            || self.lease_duration_ns <= 0 || self.idle_poll.is_zero()
        { return Err(GatewayError::HostAdapter("invalid Insomnia worker bounds".into())); }
        Ok(())
    }
}

#[derive(Clone)]
struct SharedEndpoint(Arc<dyn GeneralEndpoint>);
impl GeneralEndpoint for SharedEndpoint {
    fn model(&self) -> &str { self.0.model() }
    fn complete_json(&self, system_prompt: &str, user_payload: &str, schema_name: &str, schema: &Value) -> Result<Value, GeneralEndpointError> {
        self.0.complete_json(system_prompt, user_payload, schema_name, schema)
    }
}

struct WorkerControl { stopped: AtomicBool, wait: (Mutex<()>, Condvar) }

/// Joinable owner-local background worker set. `stop` requests quiescence and
/// waits for in-flight provider calls to return; those calls hold no owner lock.
pub struct InsomniaWorkerHandle {
    control: Arc<WorkerControl>,
    workers: Vec<JoinHandle<()>>,
}
impl InsomniaWorkerHandle {
    pub(crate) fn start(instance: Arc<Instance>, owner: OwnerId, endpoint: Arc<dyn GeneralEndpoint>, config: InsomniaWorkerConfig) -> Result<Self, GatewayError> {
        config.validate()?;
        let control = Arc::new(WorkerControl { stopped: AtomicBool::new(false), wait: (Mutex::new(()), Condvar::new()) });
        let mut workers = Vec::with_capacity(config.workers);
        for index in 0..config.workers {
            let instance = Arc::clone(&instance);
            let endpoint = SharedEndpoint(Arc::clone(&endpoint));
            let config = config.clone();
            let control = Arc::clone(&control);
            workers.push(thread::spawn(move || worker_loop(instance, owner, endpoint, config, index, control)));
        }
        Ok(Self { control, workers })
    }
    pub fn stop(&mut self) -> Result<(), GatewayError> {
        self.control.stopped.store(true, Ordering::Release);
        self.control.wait.1.notify_all();
        for worker in self.workers.drain(..) {
            worker.join().map_err(|_| GatewayError::HostAdapter("Insomnia worker panicked".into()))?;
        }
        Ok(())
    }
    pub fn is_stopped(&self) -> bool { self.control.stopped.load(Ordering::Acquire) && self.workers.iter().all(JoinHandle::is_finished) }
}
impl Drop for InsomniaWorkerHandle { fn drop(&mut self) { let _ = self.stop(); } }

fn worker_loop(instance: Arc<Instance>, owner: OwnerId, endpoint: SharedEndpoint, config: InsomniaWorkerConfig, index: usize, control: Arc<WorkerControl>) {
    let worker_id = format!("gateway-insomnia-{}", index + 1);
    while !control.stopped.load(Ordering::Acquire) {
        let now = now_ns();
        let claim = match instance.claim_next_insomnia_input(owner, &worker_id, now, config.lease_duration_ns, config.max_admissions_per_claim) {
            Ok(Some(claim)) => claim,
            Ok(None) => { wait(&control, config.idle_poll); continue; }
            Err(_) => { wait(&control, config.idle_poll); continue; }
        };
        process_claim(&instance, owner, &endpoint, &config, &claim, &worker_id);
    }
}

fn process_claim(instance: &Instance, owner: OwnerId, endpoint: &SharedEndpoint, config: &InsomniaWorkerConfig, claim: &RelInsomniaClaimInput, worker: &str) {
    if let Some(payload) = claim.work.held_payload.as_deref() {
        match decode_checkpoint(owner, claim, payload) {
            Ok(("extraction", extraction, model)) => {
                process_extraction(instance, owner, config, claim, worker, extraction, model, payload.to_vec());
                return;
            }
            Ok(("evidence", _, _)) => {
                hold(instance, owner, claim, "evidence resolution requires an authorized resolver", Some(payload.to_vec()));
                return;
            }
            Ok(_) | Err(_) => {
                hold(instance, owner, claim, "stored Insomnia checkpoint has an unsupported version or phase", Some(payload.to_vec()));
                return;
            }
        }
    }

    let extractor = InsomniaModelExtractor::new(endpoint.clone());
    let stage = match extractor.start(&claim.episode, &claim.turns) {
        Ok(stage) => stage,
        Err(error) => { hold(instance, owner, claim, &bounded_reason(error), None); return; }
    };
    if !stage.requests.is_empty() {
        let payload = serde_json::to_vec(&json!({
            "version": CHECKPOINT_VERSION,
            "phase": "evidence",
            "owner_id": hex(&owner.as_bytes()),
            "episode_id": hex(&claim.episode.id.0),
            "attempt": claim.work.attempt_count,
            "extractor_model": endpoint.model(),
            "initial_ledger": stage.initial_ledger,
            "requests": stage.requests.iter().map(|r| json!({"kind": r.kind, "conversation_id": r.conversation_id, "node_id": r.node_id, "start_node_id": r.start_node_id, "end_node_id": r.end_node_id, "query": r.query, "limit": r.limit})).collect::<Vec<_>>(),
        })).ok();
        hold(instance, owner, claim, "Insomnia evidence requests require an authorized resolver", payload);
        return;
    }
    let extraction = match extractor.finish_without_evidence(&claim.episode, &claim.turns, stage) {
        Ok(extraction) => extraction,
        Err(error) => { hold(instance, owner, claim, &bounded_reason(error), None); return; }
    };
    let model = endpoint.model().to_owned();
    let payload = match encode_extraction(owner, claim, &model, &extraction) {
        Ok(payload) => payload,
        Err(error) => { hold(instance, owner, claim, &bounded_reason(error), None); return; }
    };
    process_extraction(instance, owner, config, claim, worker, extraction, model, payload);
}

fn process_extraction(instance: &Instance, owner: OwnerId, config: &InsomniaWorkerConfig, claim: &RelInsomniaClaimInput, worker: &str, extraction: InsomniaExtraction, model: String, payload: Vec<u8>) {
    if !extraction.rejected.is_empty() {
        hold(instance, owner, claim, "Insomnia returned rejected candidates; review is required", Some(payload));
        return;
    }
    if extraction.candidates.iter().any(|candidate| candidate.ownership == reliquary_insomnia::InsomniaOwnership::User) {
        hold(instance, owner, claim, "user-owned Memory route requires an authorized PHY publication workflow", Some(payload));
        return;
    }
    // Sync the exact extraction before attempting semantic publication. On
    // restart the worker resumes these bytes and never repeats model inference.
    if let Err(error) = instance.checkpoint_insomnia_claim(owner, claim, now_ns(), payload.clone()) {
        hold(instance, owner, claim, &error.to_string(), Some(payload));
        return;
    }
    let prepared: PreparedIngest = match prepare_native_ingest_from_claim(claim, extraction) {
        Ok(prepared) => prepared,
        Err(error) => { hold(instance, owner, claim, &error.to_string(), Some(payload)); return; }
    };
    // Stable across an uncertain commit response, allowing the grouped receipt
    // path to recognize an already-synced completion for this lease attempt.
    match instance.commit_prepared_insomnia_claim(owner, claim, &prepared, &model, "v3-4", claim.started_at_ns) {
        Ok(_) => {}
        Err(error) => hold(instance, owner, claim, &format!("publication outcome requires reconciliation: {error}"), Some(payload)),
    }
    let _ = (config, worker); // reserved for lease renewal and per-worker policy.
}

fn encode_extraction(owner: OwnerId, claim: &RelInsomniaClaimInput, model: &str, extraction: &InsomniaExtraction) -> Result<Vec<u8>, String> {
    let value = json!({"version": CHECKPOINT_VERSION, "phase": "extraction", "owner_id": hex(&owner.as_bytes()), "episode_id": hex(&claim.episode.id.0), "attempt": claim.work.attempt_count, "extractor_model": model, "extraction": extraction});
    let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    if bytes.len() > reliquary_owner_session::MAX_INSOMNIA_CHECKPOINT_BYTES { return Err("Insomnia extraction exceeds durable checkpoint limit".into()); }
    Ok(bytes)
}

fn decode_checkpoint<'a>(owner: OwnerId, claim: &RelInsomniaClaimInput, bytes: &'a [u8]) -> Result<(&'static str, InsomniaExtraction, String), String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if value.get("version").and_then(Value::as_u64) != Some(CHECKPOINT_VERSION)
        || value.get("owner_id").and_then(Value::as_str) != Some(hex(&owner.as_bytes()).as_str())
        || value.get("episode_id").and_then(Value::as_str) != Some(hex(&claim.episode.id.0).as_str())
    { return Err("checkpoint identity mismatch".into()); }
    let phase = value.get("phase").and_then(Value::as_str).ok_or("checkpoint phase missing")?;
    let model = value.get("extractor_model").and_then(Value::as_str).unwrap_or("unknown").to_owned();
    if phase == "evidence" { return Ok(("evidence", InsomniaExtraction { candidates: vec![], rejected: vec![] }, model)); }
    if phase != "extraction" { return Err("unsupported checkpoint phase".into()); }
    let extraction = serde_json::from_value(value.get("extraction").cloned().ok_or("extraction checkpoint missing")?).map_err(|e| e.to_string())?;
    Ok(("extraction", extraction, model))
}

fn hold(instance: &Instance, owner: OwnerId, claim: &RelInsomniaClaimInput, reason: &str, payload: Option<Vec<u8>>) {
    let reason = bounded_reason(reason.to_owned());
    let _ = instance.hold_insomnia_claim_with_payload(owner, claim, now_ns(), reason, payload);
}
fn wait(control: &WorkerControl, duration: Duration) {
    if let Ok(lock) = control.wait.0.lock() { let _ = control.wait.1.wait_timeout(lock, duration); }
}
fn now_ns() -> i64 { SystemTime::now().duration_since(UNIX_EPOCH).ok().and_then(|d| i64::try_from(d.as_nanos()).ok()).unwrap_or(0) }
fn bounded_reason(reason: String) -> String { reason.chars().take(2048).collect() }
fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }
