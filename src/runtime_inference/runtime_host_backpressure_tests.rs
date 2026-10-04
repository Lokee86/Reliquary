use crate::{
    Cva, EpisodeConfig, EpisodePolicy, GeneralEndpoint, GeneralEndpointError, InsomniaWorkerConfig,
    InteractionRuntime, ReliquaryRuntimeHost, ReliquaryRuntimeRoutes,
};
use serde_json::{Value, json};
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
        "host-backpressure"
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

struct OmitEndpoint {
    calls: Arc<AtomicUsize>,
}

impl GeneralEndpoint for OmitEndpoint {
    fn model(&self) -> &str {
        "host-omit"
    }

    fn complete_json(
        &self,
        _system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if schema_name != "insomnia_authority_disposition_ledger" {
            return Err(GeneralEndpointError::Failure(format!(
                "unexpected schema {schema_name}"
            )));
        }
        let payload: Value = serde_json::from_str(user_payload)
            .map_err(|error| GeneralEndpointError::Failure(error.to_string()))?;
        let mut turns = serde_json::Map::new();
        for turn in payload["turns"].as_array().into_iter().flatten() {
            if turn["role"] != "user" {
                continue;
            }
            let id = turn["id"].as_str().unwrap_or("");
            turns.insert(
                id.to_owned(),
                json!([{
                    "disposition":"omit", "authority_kind":"none", "category":"none",
                    "type":"none", "lifecycle":"none", "proposition":"",
                    "authority_source_node_id":"", "grounding_source_node_id":"",
                    "reason":"No durable memory here."
                }]),
            );
        }
        Ok(json!({"turns": turns, "evidence_requests": []}))
    }
}

fn test_path() -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("reliquary-host-backpressure-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join("project.prj.rel")
}

fn queue_imports(cva: &mut Cva, count: usize) {
    for index in 0..count {
        let conversation = format!("c{index}");
        let node = format!("u{index}");
        cva.append_node(
            node.clone(),
            conversation.clone(),
            None,
            "user".into(),
            i64::try_from(index + 1).unwrap(),
            "No durable memory here.",
        )
        .unwrap();
        cva.finalize_import_path_and_queue(
            &conversation,
            &node,
            EpisodeConfig::default(),
            i64::try_from(index + 100).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn runtime_host_gates_new_claims_during_provider_backpressure() {
    let mut cva = Cva::create(test_path()).unwrap();
    queue_imports(&mut cva, 8);
    let blocked_calls = Arc::new(AtomicUsize::new(0));
    let routes = ReliquaryRuntimeRoutes::new(
        None,
        Some(Arc::new(BackpressureEndpoint {
            calls: Arc::clone(&blocked_calls),
        })),
        None,
        None,
        None,
    );
    let config = InsomniaWorkerConfig {
        workers: 4,
        poll_interval_ns: 100_000,
        ..Default::default()
    };
    let host = ReliquaryRuntimeHost::start(
        InteractionRuntime::new(cva),
        routes,
        config,
        EpisodePolicy::default(),
    )
    .unwrap();

    std::thread::sleep(Duration::from_millis(100));
    let first_wave = blocked_calls.load(Ordering::SeqCst);
    assert!(first_wave > 0 && first_wave <= 4);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(blocked_calls.load(Ordering::SeqCst), first_wave);
    assert_eq!(host.insomnia_stats().unwrap().terminal, 0);

    let resumed_calls = Arc::new(AtomicUsize::new(0));
    host.set_routes(ReliquaryRuntimeRoutes::new(
        None,
        Some(Arc::new(OmitEndpoint {
            calls: Arc::clone(&resumed_calls),
        })),
        None,
        None,
        None,
    ))
    .unwrap();
    for _ in 0..100 {
        if resumed_calls.load(Ordering::SeqCst) > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(resumed_calls.load(Ordering::SeqCst) > 0);

    let cva = host.into_cva().unwrap();
    assert_eq!(cva.insomnia_stats().terminal, 0);
}
