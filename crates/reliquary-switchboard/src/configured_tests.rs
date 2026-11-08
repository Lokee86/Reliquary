use super::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const TEST_KEY: &str = "synthetic-oc02-test-key";

struct ConfigFile(PathBuf);

impl ConfigFile {
    fn new(document: Value) -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let directory =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/oc02-fixtures");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!(
            "switchboard-{}-{}.json",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, document.to_string()).unwrap();
        Self(path)
    }
}

impl Drop for ConfigFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[derive(Default)]
struct Resolver(RefCell<Vec<String>>);

impl CredentialResolver for Resolver {
    fn resolve(&self, id: &CredentialId) -> Result<CredentialMaterial, String> {
        self.0.borrow_mut().push(id.to_string());
        match id.to_string().as_str() {
            "KEY" => Ok(CredentialMaterial::ApiKey(TEST_KEY.into())),
            "WRONG_TYPE" => Ok(CredentialMaterial::CodexOAuth {
                access_token: TEST_KEY.into(),
                account_id: "synthetic-account".into(),
            }),
            _ => Err(format!("private resolver failure with {TEST_KEY}")),
        }
    }
}

#[test]
fn offline_load_resolves_each_reference_once_and_selects_dedicated_then_shared() {
    let path = ConfigFile::new(json!({
        "general": {"model":"general-model", "url":"https://offline.example.invalid", "credential_id":"KEY"},
        "decision": {"model":"shared-model", "url":"https://offline.example.invalid", "credential_id":"KEY"},
        "insomnia": {"model":"insomnia-model", "url":"https://offline.example.invalid", "credential_id":"KEY"},
        "entity_resolution": {"model":"entity-general-model", "url":"https://offline.example.invalid", "credential_id":"KEY"},
        "entity_resolution_decision": {"model":"entity-model", "url":"https://offline.example.invalid", "credential_id":"KEY"}
    }));
    let resolver = Resolver::default();
    let runtime = ConfiguredRoutes::from_path(&path.0, &resolver).unwrap();
    assert_eq!(*resolver.0.borrow(), ["KEY"]);
    assert_eq!(runtime.general_endpoint().unwrap().model(), "general-model");
    assert_eq!(
        runtime.insomnia_endpoint().unwrap().model(),
        "insomnia-model"
    );
    assert_eq!(runtime.dream_endpoint().unwrap().model(), "general-model");
    assert_eq!(
        runtime.entity_endpoint().unwrap().model(),
        "entity-general-model"
    );
    assert_eq!(
        runtime
            .decision_endpoint(DecisionConsumer::EntityResolution)
            .unwrap()
            .model(),
        "entity-model"
    );
    assert_eq!(
        runtime
            .decision_endpoint(DecisionConsumer::MemoryRetrieval)
            .unwrap()
            .model(),
        "shared-model"
    );
    let report = runtime.readiness();
    assert!(report.contains("entity_resolution=entity_resolution_decision"));
    assert!(report.contains("memory_retrieval=decision"));
    assert!(report.contains(
        "general selection: insomnia=insomnia, dream=general, entity_resolution=entity_resolution"
    ));
    for private in [TEST_KEY, "KEY", "entity-model", "offline.example.invalid"] {
        assert!(!report.contains(private));
    }
}

#[test]
fn invalid_configuration_and_credential_failures_are_redacted_offline() {
    let resolver = Resolver::default();
    let invalid = ConfigFile::new(json!({
        "general": {"model":"m", "url":"https://offline.example.invalid", "credential_id":"KEY"},
        "decision": {"model":"m", "url":format!("https://{TEST_KEY}@offline.example.invalid"), "credential_id":"MISSING"}
    }));
    let error = ConfiguredRoutes::from_path(&invalid.0, &resolver)
        .err()
        .unwrap();
    assert!(matches!(
        error,
        ConfiguredRuntimeError::Invalid {
            route: "decision",
            ..
        }
    ));
    assert!(!error.to_string().contains(TEST_KEY));
    assert!(resolver.0.borrow().is_empty());

    let invalid_lease = ConfigFile::new(json!({"worker": {"lease_duration_ms": 600001}}));
    assert!(matches!(
        ConfiguredRoutes::from_path(&invalid_lease.0, &resolver),
        Err(ConfiguredRuntimeError::Invalid {
            route: "worker",
            ..
        })
    ));

    for reference in ["MISSING", "WRONG_TYPE"] {
        let path = ConfigFile::new(json!({
            "general": {"model":"m", "url":"https://offline.example.invalid", "credential_id":"KEY"},
            "entity_resolution_decision": {"model":"m", "url":"https://offline.example.invalid", "credential_id":reference}
        }));
        let error = ConfiguredRoutes::from_path(&path.0, &resolver)
            .err()
            .unwrap();
        assert!(matches!(
            error,
            ConfiguredRuntimeError::Credential {
                route: "entity_resolution_decision",
                ..
            }
        ));
        assert!(!format!("{error:?} {error}").contains(TEST_KEY));
        assert!(!error.to_string().contains(reference));
    }
}

#[test]
fn configured_online_probe_checks_each_general_transport_and_other_selected_routes() {
    let general_server = || {
        mock_server(
            "200 OK",
            "",
            r#"{"choices":[{"message":{"content":"{\"ok\":true}"}}]}"#,
        )
    };
    let (general, general_request) = general_server();
    let (insomnia, insomnia_request) = general_server();
    let (dream, dream_request) = general_server();
    let (entity, entity_request) = general_server();
    let (embedding, embedding_request) =
        mock_server("200 OK", "", r#"{"data":[{"index":0,"embedding":[3,4]}]}"#);
    let (decision, decision_request) = mock_server("200 OK", "", r#"{"ok":true}"#);
    let path = ConfigFile::new(json!({
        "general": {"model":"general-model", "url":general, "credential_id":"KEY"},
        "insomnia": {"model":"insomnia-model", "url":insomnia, "credential_id":"KEY"},
        "dream": {"model":"dream-model", "url":dream, "credential_id":"KEY"},
        "entity_resolution": {"model":"entity-general-model", "url":entity, "credential_id":"KEY"},
        "entity_resolution_decision": {"model":"entity-model", "url":decision, "credential_id":"KEY"},
        "embedding": {"model":"embedding-model", "url":embedding, "credential_id":"KEY",
                      "dimensions":2, "normalization":"l2"}
    }));
    let runtime = ConfiguredRoutes::from_path(&path.0, &Resolver::default()).unwrap();
    assert_eq!(
        runtime
            .decision_endpoint(DecisionConsumer::MemoryRetrieval)
            .unwrap()
            .model(),
        "general-model"
    );
    assert!(
        runtime
            .online_probe()
            .unwrap()
            .starts_with("provider routes probed: 6;")
    );
    for (request, model) in [
        (general_request, "general-model"),
        (insomnia_request, "insomnia-model"),
        (dream_request, "dream-model"),
        (entity_request, "entity-general-model"),
        (embedding_request, "embedding-model"),
    ] {
        let request = String::from_utf8(request.join().unwrap()).unwrap();
        assert!(request.contains(TEST_KEY));
        assert!(request.contains("Reliquary provider readiness probe"));
        assert!(request.contains(model));
    }
    let decision_request = String::from_utf8(decision_request.join().unwrap()).unwrap();
    let body: Value =
        serde_json::from_str(decision_request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["state"]["operation"], "readiness_probe");
    assert_eq!(body["model"], "entity-model");
}

#[test]
fn failed_selected_decision_preserves_backpressure_without_falling_through() {
    let (dedicated, dedicated_request) = mock_server(
        "429 Too Many Requests",
        "Retry-After: 7\r\n",
        r#"{"error":"private provider payload"}"#,
    );
    let fallback = TcpListener::bind("127.0.0.1:0").unwrap();
    fallback.set_nonblocking(true).unwrap();
    let path = ConfigFile::new(json!({
        "decision": {"model":"shared-model", "url":format!("http://{}", fallback.local_addr().unwrap()), "credential_id":"KEY"},
        "memory_retrieval_decision": {"model":"dedicated-model", "url":dedicated, "credential_id":"KEY"}
    }));
    let runtime = ConfiguredRoutes::from_path(&path.0, &Resolver::default()).unwrap();
    let endpoint = runtime
        .decision_endpoint(DecisionConsumer::MemoryRetrieval)
        .unwrap();
    assert_eq!(endpoint.model(), "dedicated-model");
    assert!(matches!(
        endpoint.evaluate(&json!({}), &json!({})),
        Err(DecisionEndpointError::Backpressure { retry_after: Some(delay), .. })
            if delay == Duration::from_secs(7)
    ));
    dedicated_request.join().unwrap();
    assert!(
        matches!(fallback.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
}

#[test]
fn specialty_probe_errors_exclude_provider_payload_and_preserve_retry_hint() {
    let (url, request) = mock_server(
        "429 Too Many Requests",
        "Retry-After: 3\r\n",
        r#"{"error":"private provider payload"}"#,
    );
    let path = ConfigFile::new(json!({
        "dream": {"model":"dream-model", "url":url, "credential_id":"KEY"}
    }));
    let runtime = ConfiguredRoutes::from_path(&path.0, &Resolver::default()).unwrap();
    let error = runtime.online_probe().unwrap_err();
    assert_eq!(
        error,
        ConfiguredRuntimeError::Probe {
            route: "dream",
            failure: RuntimeEndpointFailure::Backpressure {
                retry_after: Some(Duration::from_secs(3))
            }
        }
    );
    assert!(!format!("{error:?} {error}").contains("private provider payload"));
    request.join().unwrap();
}

#[test]
fn processing_identity_excludes_retrieval_and_scopes_the_live_dream_route() {
    let original = json!({
        "insomnia": {"model":"extractor","url":"https://example.test/insomnia","credential_id":"KEY"},
        "dream": {"model":"organizer","url":"https://example.test/dream","credential_id":"KEY"},
        "embedding": {"model":"vectors","url":"https://example.test/embed","credential_id":"KEY","dimensions":3}
    });
    let identity = |config| {
        let file = ConfigFile::new(config);
        ConfiguredRoutes::from_path(&file.0, &Resolver::default())
            .unwrap()
            .processing_route_identity()
    };
    let bound = identity(original.clone());
    let mut changed = original;
    changed["memory_retrieval_decision"] = json!({
        "model":"retrieval","url":"https://example.test/decision","credential_id":"KEY"
    });
    let with_retrieval = identity(changed.clone());
    assert_eq!(bound.import, with_retrieval.import);
    assert_eq!(bound.live_dream, with_retrieval.live_dream);
    changed["insomnia"]["model"] = json!("another-extractor");
    let changed_extractor = identity(changed.clone());
    assert_ne!(bound.import, changed_extractor.import);
    assert_eq!(bound.live_dream, changed_extractor.live_dream);
    changed["embedding"]["dimensions"] = json!(4);
    assert_ne!(bound.live_dream, identity(changed).live_dream);
}

#[test]
fn online_probe_reports_http_status_without_the_provider_payload() {
    let (url, request) = mock_server("401 Unauthorized", "", "private provider payload");
    let path = ConfigFile::new(json!({
        "insomnia": {"model":"m", "url":url, "credential_id":"KEY"}
    }));
    let runtime = ConfiguredRoutes::from_path(&path.0, &Resolver::default()).unwrap();
    let error = runtime.online_probe().unwrap_err();
    assert_eq!(
        error,
        ConfiguredRuntimeError::Probe {
            route: "insomnia",
            failure: RuntimeEndpointFailure::HttpStatus(401)
        }
    );
    assert!(!format!("{error:?} {error}").contains("private provider payload"));
    request.join().unwrap();
}

#[test]
fn online_probe_rejects_invalid_and_oversized_structured_responses() {
    for (body, failure) in [
        (
            r#"{"choices":[{"message":{"content":"{\"ok\":false}"}}]}"#.to_owned(),
            RuntimeEndpointFailure::InvalidResponse,
        ),
        (
            "x".repeat(1024 * 1024 + 1),
            RuntimeEndpointFailure::RequestFailed,
        ),
    ] {
        let (url, request) = mock_server("200 OK", "", &body);
        let path = ConfigFile::new(json!({
            "general": {"model":"m", "url":url, "credential_id":"KEY"}
        }));
        let runtime = ConfiguredRoutes::from_path(&path.0, &Resolver::default()).unwrap();
        assert_eq!(
            runtime.online_probe(),
            Err(ConfiguredRuntimeError::Probe {
                route: "general",
                failure
            })
        );
        request.join().unwrap();
    }
}
