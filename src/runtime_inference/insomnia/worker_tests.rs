use crate::{
    Branch, Cva, EpisodeConfig, GeneralEndpoint, GeneralEndpointError, InsomniaExtractor,
    InsomniaProgressEvent, InsomniaWorkerConfig, SimulatedEmbeddingEndpoint, VectorNormalization,
};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-insomnia-workers-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn queue_import(cva: &mut Cva, conversation: &str, index: usize) {
    let user = format!("{conversation}-u");
    cva.append_node(
        user.clone(),
        conversation.into(),
        None,
        "user".into(),
        i64::try_from(index + 1).unwrap(),
        "No durable memory here.",
    )
    .unwrap();
    cva.finalize_import_path_and_queue(
        conversation,
        &user,
        EpisodeConfig::default(),
        i64::try_from(index + 100).unwrap(),
    )
    .unwrap();
}

struct TrackingEndpoint {
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
    failures_remaining: Arc<AtomicUsize>,
    invalid_configuration: bool,
    delay: Duration,
}

impl GeneralEndpoint for TrackingEndpoint {
    fn model(&self) -> &str {
        "tracking-model"
    }

    fn complete_json(
        &self,
        _system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.invalid_configuration {
            return Err(GeneralEndpointError::InvalidConfiguration("test failure"));
        }
        if self
            .failures_remaining
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                value.checked_sub(1)
            })
            .is_ok()
        {
            return Err(GeneralEndpointError::Failure(
                "transient test failure".into(),
            ));
        }
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        thread::sleep(self.delay);
        self.active.fetch_sub(1, Ordering::SeqCst);
        if schema_name != "insomnia_authority_disposition_ledger" {
            return Err(GeneralEndpointError::Failure(format!(
                "unexpected synthetic schema {schema_name}"
            )));
        }
        let payload: Value = serde_json::from_str(user_payload)
            .map_err(|error| GeneralEndpointError::Failure(error.to_string()))?;
        let mut ledger_turns = serde_json::Map::new();
        for turn in payload["turns"].as_array().into_iter().flatten() {
            if turn["role"] != "user" {
                continue;
            }
            let id = turn["id"].as_str().unwrap_or("");
            ledger_turns.insert(
                id.to_owned(),
                json!([{
                    "disposition":"omit", "authority_kind":"none", "category":"none",
                    "type":"none", "lifecycle":"none", "proposition":"",
                    "authority_source_node_id":"", "grounding_source_node_id":"",
                    "reason":"No durable memory here."
                }]),
            );
        }
        Ok(json!({"turns": ledger_turns, "evidence_requests": []}))
    }
}

fn tracking_endpoint(
    delay: Duration,
    failures: usize,
    invalid_configuration: bool,
) -> (TrackingEndpoint, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let peak = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    (
        TrackingEndpoint {
            active: Arc::new(AtomicUsize::new(0)),
            peak: Arc::clone(&peak),
            calls: Arc::clone(&calls),
            failures_remaining: Arc::new(AtomicUsize::new(failures)),
            invalid_configuration,
            delay,
        },
        peak,
        calls,
    )
}

#[test]
fn canonical_import_registration_materializes_and_queues_the_whole_file() {
    let path = test_path("registration.cva");
    let mut cva = Cva::create(path).unwrap();
    cva.append_node(
        "u0".into(),
        "c1".into(),
        None,
        "user".into(),
        1,
        "Remember this import.",
    )
    .unwrap();
    cva.append_branch(Branch {
        id: "main".into(),
        conversation_id: "c1".into(),
        leaf_node_id: "u0".into(),
        canonical: true,
    })
    .unwrap();

    cva.finalize_canonical_imports_and_queue(EpisodeConfig::default(), 10)
        .unwrap();
    assert_eq!(cva.episodes().len(), 1);
    assert_eq!(cva.insomnia_stats().total, 1);
    cva.finalize_canonical_imports_and_queue(EpisodeConfig::default(), 20)
        .unwrap();
    assert_eq!(cva.episodes().len(), 1);
    assert_eq!(cva.insomnia_stats().total, 1);
}

#[test]
fn worker_pool_overlaps_model_calls_and_drains_the_backlog() {
    let path = test_path("parallel.cva");
    let mut cva = Cva::create(path).unwrap();
    for index in 0..8 {
        queue_import(&mut cva, &format!("c{index}"), index);
    }
    let (endpoint, endpoint_peak, calls) = tracking_endpoint(Duration::from_millis(30), 0, false);
    let extractor = InsomniaExtractor::new(endpoint);
    let config = InsomniaWorkerConfig {
        workers: 4,
        poll_interval_ns: 1_000_000,
        ..Default::default()
    };
    let embedding = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 7);
    let result = cva
        .drain_insomnia_backlog(&extractor, &embedding, config)
        .unwrap();

    assert_eq!(result.completed_episodes, 8);
    assert_eq!(result.claimed_attempts, 8);
    assert_eq!(calls.load(Ordering::SeqCst), 8);
    assert!(endpoint_peak.load(Ordering::SeqCst) > 1);
    assert!(result.peak_active_workers > 1);
    let stats = cva.insomnia_stats();
    assert_eq!(stats.complete, 8);
    assert_eq!(stats.pending + stats.processing + stats.failed, 0);
}

#[test]
fn retryable_failure_is_reclaimed_and_completed_in_the_same_drain() {
    let path = test_path("retry.cva");
    let mut cva = Cva::create(path).unwrap();
    queue_import(&mut cva, "c1", 0);
    let (endpoint, _, calls) = tracking_endpoint(Duration::ZERO, 1, false);
    let extractor = InsomniaExtractor::new(endpoint);
    let config = InsomniaWorkerConfig {
        workers: 1,
        retry_delay_ns: 1_000_000,
        poll_interval_ns: 100_000,
        max_attempts: 3,
        ..Default::default()
    };
    let embedding = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 7);
    let result = cva
        .drain_insomnia_backlog(&extractor, &embedding, config)
        .unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(result.claimed_attempts, 2);
    assert_eq!(result.failed_attempts, 1);
    assert_eq!(result.completed_episodes, 1);
    assert_eq!(result.terminal_episodes, 0);
    assert_eq!(cva.insomnia_stats().attempts, 1);
}

struct InvalidLedgerOnceEndpoint {
    calls: AtomicUsize,
}

impl GeneralEndpoint for InvalidLedgerOnceEndpoint {
    fn model(&self) -> &str {
        "invalid-ledger-once"
    }

    fn complete_json(
        &self,
        _system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        assert_eq!(schema_name, "insomnia_authority_disposition_ledger");
        let payload: Value = serde_json::from_str(user_payload).unwrap();
        let id = payload["turns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|turn| turn["role"] == "user")
            .and_then(|turn| turn["id"].as_str())
            .unwrap();
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let clause = json!({
            "disposition":"omit", "authority_kind":"none", "category":"none",
            "type":"none", "lifecycle":"none", "proposition":"",
            "authority_source_node_id":"", "grounding_source_node_id":"",
            "reason":"No durable memory here."
        });
        Ok(json!({
            "turns": {id: [clause]},
            "evidence_requests": if call == 0 { json!("invalid") } else { json!([]) }
        }))
    }
}

#[test]
fn invalid_model_output_is_retried_before_terminalizing() {
    let path = test_path("invalid-output-retry.cva");
    let mut cva = Cva::create(path).unwrap();
    queue_import(&mut cva, "c1", 0);
    let extractor = InsomniaExtractor::new(InvalidLedgerOnceEndpoint {
        calls: AtomicUsize::new(0),
    });
    let config = InsomniaWorkerConfig {
        workers: 1,
        retry_delay_ns: 1_000_000,
        poll_interval_ns: 100_000,
        max_attempts: 3,
        ..Default::default()
    };
    let embedding = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 7);
    let result = cva
        .drain_insomnia_backlog(&extractor, &embedding, config)
        .unwrap();

    assert_eq!(result.claimed_attempts, 2);
    assert_eq!(result.failed_attempts, 1);
    assert_eq!(result.completed_episodes, 1);
    assert_eq!(result.terminal_episodes, 0);
    assert_eq!(cva.insomnia_stats().complete, 1);
}

struct MissingTypeRepairEndpoint {
    calls: AtomicUsize,
}

impl GeneralEndpoint for MissingTypeRepairEndpoint {
    fn model(&self) -> &str {
        "missing-type-repair"
    }

    fn complete_json(
        &self,
        _system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match schema_name {
            "insomnia_authority_disposition_ledger" => {
                let payload: Value = serde_json::from_str(user_payload).unwrap();
                let id = payload["turns"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|turn| turn["role"] == "user")
                    .and_then(|turn| turn["id"].as_str())
                    .unwrap();
                Ok(json!({
                    "turns": {id: [
                        {
                            "disposition":"retain", "authority_kind":"direct", "category":"fact",
                            "lifecycle":"current", "proposition":"",
                            "authority_source_node_id":"", "grounding_source_node_id":"",
                            "reason":"Retain."
                        },
                        {
                            "disposition":"retain", "authority_kind":"invalid-authority", "category":"fact",
                            "lifecycle":"current", "proposition":"This is additional durable project state.",
                            "authority_source_node_id":"", "grounding_source_node_id":"",
                            "reason":"Retain."
                        }
                    ]},
                    "evidence_requests": []
                }))
            }
            "insomnia_authority_disposition_clause_repair" => Ok(json!({
                "repairs": {
                    "r000": {"proposition":"This is durable project state.", "type":"project"},
                    "r001": {"authority_kind":"direct", "type":"project"}
                }
            })),
            "insomnia_memory_wording" => Ok(json!({
                "groups": {"g000": {
                    "title":"Durable project state",
                    "content":"This is durable project state. This is additional durable project state."
                }}
            })),
            other => Err(GeneralEndpointError::Failure(format!(
                "unexpected repair test schema {other}"
            ))),
        }
    }
}

#[test]
fn malformed_ledger_fields_are_batched_and_repaired_without_episode_retry() {
    let path = test_path("missing-type-repair.cva");
    let mut cva = Cva::create(path).unwrap();
    queue_import(&mut cva, "c1", 0);
    let calls = Arc::new(AtomicUsize::new(0));
    struct SharedRepairEndpoint(Arc<AtomicUsize>);
    impl GeneralEndpoint for SharedRepairEndpoint {
        fn model(&self) -> &str {
            "missing-type-repair"
        }
        fn complete_json(
            &self,
            system_prompt: &str,
            user_payload: &str,
            schema_name: &str,
            schema: &Value,
        ) -> Result<Value, GeneralEndpointError> {
            let endpoint = MissingTypeRepairEndpoint {
                calls: AtomicUsize::new(0),
            };
            self.0.fetch_add(1, Ordering::SeqCst);
            endpoint.complete_json(system_prompt, user_payload, schema_name, schema)
        }
    }
    let extractor = InsomniaExtractor::new(SharedRepairEndpoint(Arc::clone(&calls)));
    let embedding = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 7);
    let result = cva
        .drain_insomnia_backlog(
            &extractor,
            &embedding,
            InsomniaWorkerConfig {
                workers: 1,
                poll_interval_ns: 100_000,
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(result.claimed_attempts, 1);
    assert_eq!(result.failed_attempts, 0);
    assert_eq!(result.completed_episodes, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[test]
fn progress_reports_episode_retries_and_completion() {
    let path = test_path("progress.cva");
    let mut cva = Cva::create(path).unwrap();
    queue_import(&mut cva, "c1", 0);
    let (endpoint, _, _) = tracking_endpoint(Duration::ZERO, 1, false);
    let extractor = InsomniaExtractor::new(endpoint);
    let config = InsomniaWorkerConfig {
        workers: 1,
        retry_delay_ns: 1_000_000,
        poll_interval_ns: 100_000,
        max_attempts: 3,
        ..Default::default()
    };
    let embedding = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 7);
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink_events = Arc::clone(&events);
    let sink = move |event: &InsomniaProgressEvent| {
        sink_events.lock().unwrap().push(event.clone());
    };

    cva.drain_insomnia_backlog_with_progress(&extractor, &embedding, config, &sink)
        .unwrap();

    let events = events.lock().unwrap();
    assert!(matches!(
        events.first(),
        Some(InsomniaProgressEvent::DrainStarted {
            total: 1,
            complete: 0,
            terminal: 0,
            workers: 1,
        })
    ));
    assert!(events.iter().any(|event| matches!(
        event,
        InsomniaProgressEvent::EpisodeStage {
            stage: crate::InsomniaSemanticStage::Ledger,
            ..
        }
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        InsomniaProgressEvent::EpisodeRetry {
            attempt: 1,
            backpressure: false,
            ..
        }
    )));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, InsomniaProgressEvent::EpisodeStarted { .. }))
            .count(),
        2
    );
    assert!(events.iter().any(|event| matches!(
        event,
        InsomniaProgressEvent::EpisodeCompleted {
            attempt: 2,
            total: 1,
            complete: 1,
            terminal: 0,
            ..
        }
    )));
}

struct PanickingProvider;

impl GeneralEndpoint for PanickingProvider {
    fn model(&self) -> &str {
        "panicking-provider"
    }

    fn complete_json(
        &self,
        _system_prompt: &str,
        _user_payload: &str,
        _schema_name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        panic!("provider sentinel")
    }
}

struct EvidenceRequestEndpoint;

impl GeneralEndpoint for EvidenceRequestEndpoint {
    fn model(&self) -> &str {
        "evidence-request-endpoint"
    }

    fn complete_json(
        &self,
        _system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        assert_eq!(schema_name, "insomnia_authority_disposition_ledger");
        let payload: Value = serde_json::from_str(user_payload).unwrap();
        let id = payload["turns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|turn| turn["role"] == "user")
            .and_then(|turn| turn["id"].as_str())
            .unwrap();
        Ok(json!({
            "turns": {id: [{
                "disposition":"omit", "authority_kind":"none", "category":"none",
                "type":"none", "lifecycle":"none", "proposition":"",
                "authority_source_node_id":"", "grounding_source_node_id":"",
                "reason":"No durable memory here."
            }]},
            "evidence_requests": [{
                "kind":"turn", "conversation_id":"c1", "node_id":id,
                "start_node_id":"", "end_node_id":"", "query":"", "limit":0
            }]
        }))
    }
}

fn drain_in_thread_with_timeout<E, P>(
    mut cva: Cva,
    extractor: InsomniaExtractor<E>,
    progress: P,
) -> Result<(), crate::InsomniaWorkerError>
where
    E: GeneralEndpoint + Send + 'static,
    P: Fn(&InsomniaProgressEvent) + Send + Sync + 'static,
{
    let (done_sender, done_receiver) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let embedding = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 7);
        let result = cva.drain_insomnia_backlog_with_progress(
            &extractor,
            &embedding,
            InsomniaWorkerConfig {
                workers: 1,
                poll_interval_ns: 100_000,
                ..Default::default()
            },
            &progress,
        );
        let _ = done_sender.send(result.map(|_| ()));
    });
    done_receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("Insomnia worker completed instead of hanging")
}

#[test]
fn provider_panic_closes_stage_reporter_and_returns_worker_panic() {
    let path = test_path("provider-panic-progress.cva");
    let mut cva = Cva::create(path).unwrap();
    queue_import(&mut cva, "c1", 0);

    let result =
        drain_in_thread_with_timeout(cva, InsomniaExtractor::new(PanickingProvider), |_| {});

    assert!(matches!(
        result,
        Err(crate::InsomniaWorkerError::ThreadPanicked)
    ));
}

#[test]
fn evidence_progress_panic_is_acknowledged_without_hanging() {
    let path = test_path("evidence-progress-panic.cva");
    let mut cva = Cva::create(path).unwrap();
    queue_import(&mut cva, "c1", 0);
    let events = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&events);
    let progress = move |event: &InsomniaProgressEvent| {
        captured.lock().unwrap().push(event.clone());
        if matches!(
            event,
            InsomniaProgressEvent::EpisodeStage {
                stage: crate::InsomniaSemanticStage::Evidence,
                ..
            }
        ) {
            panic!("evidence reporter sentinel");
        }
    };

    let result = drain_in_thread_with_timeout(
        cva,
        InsomniaExtractor::new(EvidenceRequestEndpoint),
        progress,
    );

    assert!(matches!(
        result,
        Err(crate::InsomniaWorkerError::ThreadPanicked)
    ));
    assert!(events.lock().unwrap().iter().any(|event| matches!(
        event,
        InsomniaProgressEvent::EpisodeStage {
            stage: crate::InsomniaSemanticStage::Evidence,
            ..
        }
    )));
}

#[test]
fn invalid_endpoint_configuration_becomes_terminal_without_retry() {
    let path = test_path("terminal.cva");
    let mut cva = Cva::create(path).unwrap();
    queue_import(&mut cva, "c1", 0);
    let (endpoint, _, calls) = tracking_endpoint(Duration::ZERO, 0, true);
    let extractor = InsomniaExtractor::new(endpoint);
    let config = InsomniaWorkerConfig {
        workers: 2,
        poll_interval_ns: 100_000,
        ..Default::default()
    };
    let embedding = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 7);
    let result = cva
        .drain_insomnia_backlog(&extractor, &embedding, config)
        .unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(result.claimed_attempts, 1);
    assert_eq!(result.completed_episodes, 0);
    assert_eq!(result.terminal_episodes, 1);
    assert_eq!(cva.insomnia_stats().terminal, 1);
}
