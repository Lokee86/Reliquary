use crate::{
    Cva, EpisodeConfig, GeneralEndpoint, GeneralEndpointError, InsomniaExtractionError,
    InsomniaExtractor, InsomniaWorkerConfig, SimulatedEmbeddingEndpoint, VectorNormalization,
};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

struct BackpressureEndpoint {
    calls: Arc<AtomicUsize>,
}

impl GeneralEndpoint for BackpressureEndpoint {
    fn model(&self) -> &str {
        "backpressure-model"
    }

    fn complete_json(
        &self,
        _system_prompt: &str,
        _user_payload: &str,
        _schema_name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(GeneralEndpointError::backpressure(
            "HTTP 429 Too Many Requests",
            Some(Duration::from_secs(120)),
        ))
    }
}

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-insomnia-backpressure-{unique}"));
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

#[test]
fn finite_drain_pauses_on_backpressure_without_terminalizing_backlog() {
    let path = test_path("finite.cva");
    let mut cva = Cva::create(&path).unwrap();
    for index in 0..8 {
        queue_import(&mut cva, &format!("c{index}"), index);
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let extractor = InsomniaExtractor::new(BackpressureEndpoint {
        calls: Arc::clone(&calls),
    });
    let embedding = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 7);
    let result = cva
        .drain_insomnia_backlog(
            &extractor,
            &embedding,
            InsomniaWorkerConfig {
                workers: 4,
                poll_interval_ns: 100_000,
                ..Default::default()
            },
        )
        .unwrap();

    assert!(result.paused_for_backpressure);
    assert_eq!(result.completed_episodes, 0);
    assert_eq!(result.terminal_episodes, 0);
    assert!(result.failed_attempts > 0);
    assert!(calls.load(Ordering::SeqCst) <= 4);
    let stats = cva.insomnia_stats();
    assert_eq!(stats.terminal, 0);
    assert_eq!(stats.pending + stats.failed, 8);

    cva.sync().unwrap();
    drop(cva);
    let reopened = Cva::open(&path).unwrap();
    let stats = reopened.insomnia_stats();
    assert_eq!(stats.pending, 8);
    assert_eq!(stats.failed, 0);
    assert_eq!(stats.terminal, 0);
}

#[test]
fn runtime_backpressure_ignores_normal_attempt_budget() {
    let path = test_path("runtime.cva");
    let mut cva = Cva::create(path).unwrap();
    queue_import(&mut cva, "c1", 0);
    let config = InsomniaWorkerConfig {
        workers: 1,
        max_attempts: 1,
        ..Default::default()
    };
    let cva = &mut cva;
    let claim = crate::insomnia::runtime_step::claim_runtime_insomnia(
        &mut cva.container,
        &cva.archive,
        &mut cva.insomnia,
        "runtime",
        &config,
    )
    .unwrap()
    .unwrap();
    assert_eq!(claim.work.attempt_count, config.max_attempts);

    let error = InsomniaExtractionError::Endpoint(GeneralEndpointError::backpressure(
        "HTTP 429 Too Many Requests",
        Some(Duration::from_secs(120)),
    ));
    crate::insomnia::runtime_step::fail_runtime_insomnia(
        &mut cva.container,
        &mut cva.insomnia,
        &claim,
        &error,
        &config,
    )
    .unwrap();

    let stats = cva.insomnia_stats();
    assert_eq!(stats.failed, 1);
    assert_eq!(stats.terminal, 0);
    let work = cva.insomnia_work(claim.work.episode_id).unwrap();
    assert!(work.retry_after_ns.unwrap() > work.updated_at_ns);
}
