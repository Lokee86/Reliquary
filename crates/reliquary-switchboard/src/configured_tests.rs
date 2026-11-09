use super::*;
use crate::test_support::mock_master_key_store;
use ModelSlot::*;

const TEST_KEY: &str = "synthetic-oc02-test-key";

fn test_id() -> CredentialId {
    CredentialId::new("KEY").unwrap()
}

fn general(model: &str, url: impl Into<String>) -> ModelRoute {
    ModelRoute::General(GeneralRoute {
        endpoint: GeneralModelEndpoint {
            provider: ModelProvider::OpenAiReady,
            model: model.into(),
            url: url.into(),
            credential_id: test_id(),
        },
        reasoning_effort: None,
    })
}

fn decision(model: &str, url: impl Into<String>) -> ModelRoute {
    ModelRoute::Decision(DecisionModelEndpoint {
        model: model.into(),
        url: url.into(),
        credential_id: test_id(),
    })
}

fn embedding(url: impl Into<String>, dimensions: u32) -> ModelRoute {
    ModelRoute::Embedding(EmbeddingModelEndpoint {
        provider: ModelProvider::OpenAiReady,
        model: "embedding-model".into(),
        url: url.into(),
        credential_id: test_id(),
        dimensions,
        normalization: EmbeddingNormalization::L2,
    })
}

fn bind(routes: &[(ModelSlot, ModelRoute)]) -> ConfiguredRoutes {
    let _keys = mock_master_key_store();
    let parent =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/oc02-fixtures");
    std::fs::create_dir_all(&parent).unwrap();
    let dir = tempfile::tempdir_in(parent).unwrap();
    let path = dir.path().join("routes.cfg");
    let mut config = ConfigFile::new(&path);
    config.add_api_key(test_id(), SecretString::new(TEST_KEY).unwrap());
    for (slot, route) in routes {
        config.set_model(*slot, route).unwrap();
    }
    config.save().unwrap();
    ConfiguredRoutes::from_path(&path).unwrap()
}

#[test]
fn offline_load_selects_dedicated_then_shared() {
    let runtime = bind(&[
        (
            General,
            general("general-model", "https://offline.example.invalid"),
        ),
        (
            GeneralDecision,
            decision("shared-model", "https://offline.example.invalid"),
        ),
        (
            Insomnia,
            general("insomnia-model", "https://offline.example.invalid"),
        ),
        (
            EntityResolution,
            general("entity-general-model", "https://offline.example.invalid"),
        ),
        (
            EntityResolutionDecision,
            decision("entity-model", "https://offline.example.invalid"),
        ),
    ]);
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
    let mut file = ConfigFile::new("unused.cfg");
    let unsafe_route = decision(
        "model",
        format!("https://{TEST_KEY}@offline.example.invalid"),
    );
    let error = file.set_model(GeneralDecision, &unsafe_route).unwrap_err();
    assert!(matches!(
        error,
        ConfiguredRuntimeError::Invalid {
            route: "decision",
            ..
        }
    ));
    assert!(!error.to_string().contains(TEST_KEY));

    let mut route = decision("model", "https://offline.example.invalid");
    let ModelRoute::Decision(endpoint) = &mut route else {
        unreachable!()
    };
    let id = CredentialId::new("oauth-test").unwrap();
    endpoint.credential_id = id.clone();
    file.set_model(EntityResolutionDecision, &route).unwrap();
    for reason in [
        "credential is missing",
        "credential has the wrong auth kind or is missing account data",
    ] {
        let error = file.bind().err().unwrap();
        assert_eq!(
            error,
            ConfiguredRuntimeError::Credential {
                route: "entity_resolution_decision",
                reason
            }
        );
        assert!(!format!("{error:?} {error}").contains(id.as_str()));
        file.credentials_for_test()
            .insert_codex_oauth(id.clone(), TEST_KEY, "test-account")
            .unwrap();
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
    let (general_url, general_request) = general_server();
    let (insomnia, insomnia_request) = general_server();
    let (dream, dream_request) = general_server();
    let (entity, entity_request) = general_server();
    let (embedding_url, embedding_request) =
        mock_server("200 OK", "", r#"{"data":[{"index":0,"embedding":[3,4]}]}"#);
    let (decision_url, decision_request) = mock_server(
        "200 OK",
        "",
        r#"{"answers":{"readiness":{"type":"choice","choice":"ready","probabilities":{"ready":1.0,"not_ready":0.0}}}}"#,
    );
    let runtime = bind(&[
        (General, general("general-model", general_url)),
        (Insomnia, general("insomnia-model", insomnia)),
        (Dream, general("dream-model", dream)),
        (EntityResolution, general("entity-general-model", entity)),
        (
            EntityResolutionDecision,
            decision("entity-model", decision_url),
        ),
        (Embedding, embedding(embedding_url, 2)),
    ]);
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
    assert_eq!(body["questions"]["readiness"]["type"], "choice");
    assert_eq!(
        body["questions"]["readiness"]["criteria"]["ready"],
        "The provider is ready."
    );
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
    let runtime = bind(&[
        (
            GeneralDecision,
            decision(
                "shared-model",
                format!("http://{}", fallback.local_addr().unwrap()),
            ),
        ),
        (
            MemoryRetrievalDecision,
            decision("dedicated-model", dedicated),
        ),
    ]);
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
    let runtime = bind(&[(Dream, general("dream-model", url))]);
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
    let mut routes = vec![
        (
            Insomnia,
            general("extractor", "https://example.test/insomnia"),
        ),
        (Dream, general("organizer", "https://example.test/dream")),
        (Embedding, embedding("https://example.test/embed", 3)),
    ];
    let bound = bind(&routes).processing_route_identity();
    routes.push((
        MemoryRetrievalDecision,
        decision("retrieval", "https://example.test/decision"),
    ));
    let with_retrieval = bind(&routes).processing_route_identity();
    assert_eq!(bound.import, with_retrieval.import);
    assert_eq!(bound.live_dream, with_retrieval.live_dream);
    routes[0].1 = general("another-extractor", "https://example.test/insomnia");
    let changed_extractor = bind(&routes).processing_route_identity();
    assert_ne!(bound.import, changed_extractor.import);
    assert_eq!(bound.live_dream, changed_extractor.live_dream);
    routes[2].1 = embedding("https://example.test/embed", 4);
    assert_ne!(
        bound.live_dream,
        bind(&routes).processing_route_identity().live_dream
    );
}

#[test]
fn online_probe_reports_http_status_without_the_provider_payload() {
    let (url, request) = mock_server("401 Unauthorized", "", "private provider payload");
    let runtime = bind(&[(Insomnia, general("m", url))]);
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
        let runtime = bind(&[(General, general("m", url))]);
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
