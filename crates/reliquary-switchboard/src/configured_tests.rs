use super::*;
use crate::test_support::mock_master_key_store;
use ModelSlot::*;
use std::sync::atomic::{AtomicUsize, Ordering};

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

struct HostedGeneral {
    calls: AtomicUsize,
    model: String,
}

impl GeneralEndpoint for HostedGeneral {
    fn model(&self) -> &str {
        &self.model
    }

    fn complete_json(
        &self,
        _system_prompt: &str,
        _user_payload: &str,
        _schema_name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(json!({"ok": true}))
    }
}

struct HostedEmbedding {
    calls: AtomicUsize,
    model: String,
    dimensions: u32,
    normalization: EmbeddingNormalization,
}

impl EmbeddingEndpoint for HostedEmbedding {
    fn model(&self) -> Option<&str> {
        Some(&self.model)
    }

    fn dimensions(&self) -> u32 {
        self.dimensions
    }

    fn normalization(&self) -> EmbeddingNormalization {
        self.normalization
    }

    fn supports_bounded_embedding(&self) -> bool {
        true
    }

    fn embed_bounded(
        &self,
        _mode: EmbeddingMode,
        inputs: &[String],
        _max_response_bytes: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(inputs
            .iter()
            .map(|_| vec![1.0; self.dimensions as usize])
            .collect())
    }

    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.embed_bounded(mode, inputs, usize::MAX)
    }
}

fn hosted_identity(
    embedding_model: &str,
    dimensions: u32,
    normalization: EmbeddingNormalization,
) -> HostedInferenceIdentity {
    HostedInferenceIdentity::new(
        "hosted-general",
        "general-model",
        "hosted-embedding",
        embedding_model,
        dimensions,
        HostedEmbeddingInputSemantics::Shared,
        normalization,
    )
    .unwrap()
}

#[test]
fn hosted_routes_bind_shared_general_decisions_and_validate_identity() {
    let general = Arc::new(HostedGeneral {
        calls: AtomicUsize::new(0),
        model: "general-model".into(),
    });
    let embedding = Arc::new(HostedEmbedding {
        calls: AtomicUsize::new(0),
        model: "embedding-model".into(),
        dimensions: 3,
        normalization: EmbeddingNormalization::L2,
    });
    let routes = ConfiguredRoutes::hosted(
        general.clone(),
        embedding.clone(),
        hosted_identity("embedding-model", 3, EmbeddingNormalization::L2),
    )
    .unwrap();

    assert!(Arc::ptr_eq(
        &routes.general_endpoint().unwrap(),
        &(general.clone() as Arc<dyn GeneralEndpoint>)
    ));
    assert_eq!(routes.insomnia_endpoint().unwrap().model(), "general-model");
    assert_eq!(routes.dream_endpoint().unwrap().model(), "general-model");
    assert_eq!(routes.entity_endpoint().unwrap().model(), "general-model");
    assert_eq!(routes.embedding_endpoint().unwrap().dimensions(), 3);
    for consumer in [
        DecisionConsumer::Chronos,
        DecisionConsumer::EntityResolution,
        DecisionConsumer::MemoryRetrieval,
    ] {
        assert_eq!(
            routes.decision_endpoint(consumer).unwrap().model(),
            "general-model"
        );
    }
    let readiness = routes.readiness();
    assert!(readiness.contains("general=true, decision=true, embedding=true"));
    assert!(readiness.contains("entity_resolution=general"));

    routes.online_probe().unwrap();
    assert_eq!(general.calls.load(Ordering::SeqCst), 2);
    assert_eq!(embedding.calls.load(Ordering::SeqCst), 1);

    let changed_embedding: Arc<dyn EmbeddingEndpoint> = Arc::new(HostedEmbedding {
        calls: AtomicUsize::new(0),
        model: "embedding-model-v2".into(),
        dimensions: 3,
        normalization: EmbeddingNormalization::L2,
    });
    let changed = ConfiguredRoutes::hosted(
        general.clone(),
        changed_embedding,
        HostedInferenceIdentity::new(
            "hosted-general",
            "general-model",
            "hosted-embedding",
            "embedding-model-v2",
            3,
            HostedEmbeddingInputSemantics::Shared,
            EmbeddingNormalization::L2,
        )
        .unwrap(),
    )
    .unwrap();
    assert_ne!(
        routes.processing_route_identity().import,
        changed.processing_route_identity().import
    );
}

#[test]
fn hosted_routes_reject_endpoint_identity_mismatches() {
    let general: Arc<dyn GeneralEndpoint> = Arc::new(HostedGeneral {
        calls: AtomicUsize::new(0),
        model: "reported-model".into(),
    });
    let embedding: Arc<dyn EmbeddingEndpoint> = Arc::new(HostedEmbedding {
        calls: AtomicUsize::new(0),
        model: "embedding-model".into(),
        dimensions: 3,
        normalization: EmbeddingNormalization::L2,
    });
    let identity = hosted_identity("embedding-model", 3, EmbeddingNormalization::L2);
    assert!(matches!(
        ConfiguredRoutes::hosted(general, embedding.clone(), identity.clone()),
        Err(ConfiguredRuntimeError::Invalid {
            route: "hosted general endpoint",
            ..
        })
    ));

    let general: Arc<dyn GeneralEndpoint> = Arc::new(HostedGeneral {
        calls: AtomicUsize::new(0),
        model: "general-model".into(),
    });
    let wrong_dimensions: Arc<dyn EmbeddingEndpoint> = Arc::new(HostedEmbedding {
        calls: AtomicUsize::new(0),
        model: "embedding-model".into(),
        dimensions: 4,
        normalization: EmbeddingNormalization::L2,
    });
    assert!(matches!(
        ConfiguredRoutes::hosted(general, wrong_dimensions, identity),
        Err(ConfiguredRuntimeError::Invalid {
            route: "hosted embedding endpoint",
            ..
        })
    ));

    let general: Arc<dyn GeneralEndpoint> = Arc::new(HostedGeneral {
        calls: AtomicUsize::new(0),
        model: "general-model".into(),
    });
    let wrong_model: Arc<dyn EmbeddingEndpoint> = Arc::new(HostedEmbedding {
        calls: AtomicUsize::new(0),
        model: "unexpected-embedding-model".into(),
        dimensions: 3,
        normalization: EmbeddingNormalization::L2,
    });
    assert!(matches!(
        ConfiguredRoutes::hosted(
            general,
            wrong_model,
            hosted_identity("embedding-model", 3, EmbeddingNormalization::L2)
        ),
        Err(ConfiguredRuntimeError::Invalid {
            route: "hosted embedding endpoint",
            ..
        })
    ));
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

fn hosted_route(endpoint: &str, model: &str) -> HostedModelRoute {
    HostedModelRoute {
        endpoint: endpoint.into(),
        model: Some(model.into()),
        reasoning_effort: None,
        embedding: None,
    }
}

fn hosted_embedding_route(endpoint: &str, model: &str) -> HostedModelRoute {
    HostedModelRoute {
        endpoint: endpoint.into(),
        model: Some(model.into()),
        reasoning_effort: None,
        embedding: Some(HostedEmbeddingSettings {
            dimensions: 3,
            normalization: EmbeddingNormalization::L2,
        }),
    }
}

fn hosted_embedding_route_with_normalization(
    endpoint: &str,
    model: &str,
    normalization: EmbeddingNormalization,
) -> HostedModelRoute {
    HostedModelRoute {
        endpoint: endpoint.into(),
        model: Some(model.into()),
        reasoning_effort: None,
        embedding: Some(HostedEmbeddingSettings {
            dimensions: 3,
            normalization,
        }),
    }
}

#[test]
fn hosted_switchboard_resolves_each_slot_and_uses_shared_decision_precedence() {
    use std::cell::RefCell;

    let temp = tempfile::tempdir().unwrap();
    let mut config = HostedSwitchboardConfig::new(temp.path().join("hosted.cfg"));
    for (slot, route) in [
        (
            ModelSlot::General,
            hosted_route("general-ref", "general-model"),
        ),
        (
            ModelSlot::Insomnia,
            hosted_route("insomnia-ref", "insomnia-model"),
        ),
        (
            ModelSlot::InsomniaMetadata,
            hosted_route("metadata-ref", "metadata-model"),
        ),
        (
            ModelSlot::EntityExtraction,
            hosted_route("extraction-ref", "extraction-model"),
        ),
        (ModelSlot::Dream, hosted_route("dream-ref", "dream-model")),
        (
            ModelSlot::EntityResolution,
            hosted_route("entity-ref", "entity-model"),
        ),
        (
            ModelSlot::GeneralDecision,
            hosted_route("shared-ref", "shared-decision"),
        ),
        (
            ModelSlot::EntityResolutionDecision,
            hosted_route("entity-decision-ref", "entity-decision"),
        ),
        (
            ModelSlot::MemoryRetrievalDecision,
            hosted_route("retrieval-ref", "retrieval-decision"),
        ),
        (
            ModelSlot::Embedding,
            hosted_embedding_route("embedding-ref", "embedding-model"),
        ),
    ] {
        config.set_model(slot, Some(route)).unwrap();
    }

    let resolved = RefCell::new(Vec::new());
    let routes = ConfiguredRoutes::hosted_switchboard(
        &config,
        |slot, route| {
            resolved.borrow_mut().push(slot);
            Ok((
                format!("scope:{}", route.endpoint),
                Arc::new(HostedGeneral {
                    calls: AtomicUsize::new(0),
                    model: route.model.clone().unwrap(),
                }) as Arc<dyn GeneralEndpoint>,
            ))
        },
        |slot, route| {
            resolved.borrow_mut().push(slot);
            let settings = route.embedding.as_ref().unwrap();
            Ok((
                format!("scope:{}", route.endpoint),
                Arc::new(HostedEmbedding {
                    calls: AtomicUsize::new(0),
                    model: route.model.clone().unwrap(),
                    dimensions: settings.dimensions,
                    normalization: settings.normalization,
                }) as Arc<dyn EmbeddingEndpoint>,
            ))
        },
    )
    .unwrap();

    let mut resolved_slots = resolved.borrow().clone();
    resolved_slots.sort();
    assert_eq!(
        resolved_slots,
        ModelSlot::ALL.to_vec(),
        "every configured Switchboard slot is resolved exactly once"
    );
    assert_eq!(routes.general_endpoint().unwrap().model(), "general-model");
    assert_eq!(
        routes.insomnia_endpoint().unwrap().model(),
        "insomnia-model"
    );
    assert_eq!(
        routes.insomnia_ownership_endpoint().unwrap().model(),
        "metadata-model"
    );
    assert_eq!(
        routes.entity_extraction_endpoint().unwrap().model(),
        "extraction-model"
    );
    assert_eq!(routes.dream_endpoint().unwrap().model(), "dream-model");
    assert_eq!(routes.entity_endpoint().unwrap().model(), "entity-model");
    assert_eq!(
        routes.embedding_endpoint().unwrap().model(),
        Some("embedding-model")
    );
    assert_eq!(
        routes
            .decision_endpoint(DecisionConsumer::Chronos)
            .unwrap()
            .model(),
        "shared-decision"
    );
    assert_eq!(
        routes
            .decision_endpoint(DecisionConsumer::EntityResolution)
            .unwrap()
            .model(),
        "entity-decision"
    );
    assert_eq!(
        routes
            .decision_endpoint(DecisionConsumer::MemoryRetrieval)
            .unwrap()
            .model(),
        "retrieval-decision"
    );
}

#[test]
fn hosted_switchboard_preserves_legacy_identity_only_for_migrated_route_shape() {
    fn bind_config(general_ref: &str, entity_ref: &str, entity: bool) -> ConfiguredRoutes {
        let temp = tempfile::tempdir().unwrap();
        let mut config = HostedSwitchboardConfig::new(temp.path().join("hosted.cfg"));
        config
            .set_model(ModelSlot::General, Some(hosted_route(general_ref, "chat")))
            .unwrap();
        if entity {
            config
                .set_model(
                    ModelSlot::EntityResolution,
                    Some(hosted_route(entity_ref, "chat")),
                )
                .unwrap();
        }
        config
            .set_model(
                ModelSlot::Embedding,
                Some(hosted_embedding_route_with_normalization(
                    "embedding-ref",
                    "embed",
                    EmbeddingNormalization::None,
                )),
            )
            .unwrap();
        ConfiguredRoutes::hosted_switchboard(
            &config,
            |_slot, route| {
                Ok((
                    "canonical-provider".into(),
                    Arc::new(HostedGeneral {
                        calls: AtomicUsize::new(0),
                        model: route.model.clone().unwrap(),
                    }) as Arc<dyn GeneralEndpoint>,
                ))
            },
            |_slot, route| {
                let settings = route.embedding.as_ref().unwrap();
                Ok((
                    "canonical-provider".into(),
                    Arc::new(HostedEmbedding {
                        calls: AtomicUsize::new(0),
                        model: route.model.clone().unwrap(),
                        dimensions: settings.dimensions,
                        normalization: settings.normalization,
                    }) as Arc<dyn EmbeddingEndpoint>,
                ))
            },
        )
        .unwrap()
    }

    let migrated = bind_config("chat", "chat", true).processing_route_identity();
    let legacy = ConfiguredRoutes::hosted(
        Arc::new(HostedGeneral {
            calls: AtomicUsize::new(0),
            model: "chat".into(),
        }),
        Arc::new(HostedEmbedding {
            calls: AtomicUsize::new(0),
            model: "embed".into(),
            dimensions: 3,
            normalization: EmbeddingNormalization::None,
        }),
        HostedInferenceIdentity::new(
            "canonical-provider",
            "chat",
            "canonical-provider",
            "embed",
            3,
            HostedEmbeddingInputSemantics::Shared,
            EmbeddingNormalization::None,
        )
        .unwrap(),
    )
    .unwrap()
    .processing_route_identity();
    assert_eq!(migrated.import, legacy.import);
    assert_eq!(migrated.live_dream, legacy.live_dream);
    assert_eq!(migrated.entity_identity, legacy.entity_identity);

    let changed_auth_scope = bind_config("chat-ref-v2", "chat", true).processing_route_identity();
    assert_ne!(migrated.import, changed_auth_scope.import);
    assert_ne!(migrated.entity_identity, changed_auth_scope.entity_identity);
    let new_profile = bind_config("chat", "", false).processing_route_identity();
    assert_ne!(new_profile.import, legacy.import);
}
