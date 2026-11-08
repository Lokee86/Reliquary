use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Mutex;

#[path = "configured_tests.rs"]
mod configured;

fn mock_server(
    status: &str,
    headers: &str,
    body: &str,
) -> (String, std::thread::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/completion", listener.local_addr().unwrap());
    let status = status.to_owned();
    let headers = headers.to_owned();
    let body = body.to_owned();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        let header_end = loop {
            let mut chunk = [0_u8; 2048];
            let count = stream.read(&mut chunk).unwrap();
            assert!(count > 0);
            request.extend_from_slice(&chunk[..count]);
            if let Some(index) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                break index + 4;
            }
        };
        let header_text = String::from_utf8_lossy(&request[..header_end]);
        let content_length = header_text
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(|value| value.trim().parse::<usize>().unwrap())
            })
            .unwrap();
        while request.len() < header_end + content_length {
            let mut chunk = [0_u8; 2048];
            let count = stream.read(&mut chunk).unwrap();
            assert!(count > 0);
            request.extend_from_slice(&chunk[..count]);
        }
        write!(
            stream,
            "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        request
    });
    (url, server)
}

fn credential_store(id: &str, token: &str) -> CredentialsConfig {
    let mut credentials = CredentialsConfig::default();
    credentials
        .insert_api_key(CredentialId::new(id).unwrap(), token)
        .unwrap();
    credentials
}

struct GeneralStub {
    response: Value,
    observed: Mutex<Option<(String, String, String, Value)>>,
}

impl GeneralEndpoint for GeneralStub {
    fn model(&self) -> &str {
        "general-model"
    }

    fn complete_json(
        &self,
        prompt: &str,
        payload: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        *self.observed.lock().unwrap() = Some((
            prompt.to_owned(),
            payload.to_owned(),
            schema_name.to_owned(),
            schema.clone(),
        ));
        Ok(self.response.clone())
    }
}

fn general(response: Value) -> Arc<GeneralStub> {
    Arc::new(GeneralStub {
        response,
        observed: Mutex::new(None),
    })
}

fn choice_questions() -> Value {
    json!({
        "resolution": {
            "type": "choice",
            "instructions": "Resolve only the supplied mention.",
            "criteria": {
                "candidate_0": "The mention matches candidate zero.",
                "unresolved": "Evidence is ambiguous."
            }
        }
    })
}

#[test]
fn decision_route_precedence_is_specific_then_shared_then_general() {
    let mut config = ModelSwitchboardConfig::default();
    assert_eq!(
        select_decision_route(&config, DecisionConsumer::Chronos),
        DecisionRouteSelection::Unavailable
    );
    config.general = Some(GeneralModelEndpoint {
        provider: ModelProvider::OpenAiReady,
        model: "general".into(),
        url: "https://example.test/v1/chat/completions".into(),
        credential_id: CredentialId::new("general").unwrap(),
    });
    assert!(matches!(
        select_decision_route(&config, DecisionConsumer::Chronos),
        DecisionRouteSelection::General(route) if route.model == "general"
    ));
    config.general_decision = Some(DecisionModelEndpoint {
        model: "shared".into(),
        url: "https://example.test/decision".into(),
        credential_id: CredentialId::new("shared").unwrap(),
    });
    assert!(matches!(
        select_decision_route(&config, DecisionConsumer::Chronos),
        DecisionRouteSelection::Shared(route) if route.model == "shared"
    ));
    config.entity_resolution_decision = Some(DecisionModelEndpoint {
        model: "entity".into(),
        url: "https://example.test/entity".into(),
        credential_id: CredentialId::new("entity").unwrap(),
    });
    assert!(matches!(
        select_decision_route(&config, DecisionConsumer::EntityResolution),
        DecisionRouteSelection::Dedicated(route) if route.model == "entity"
    ));
}

#[test]
fn general_fallback_builds_constrained_choice_schema_and_validates_answers() {
    let endpoint = general(json!({
        "answers": {"resolution": {"type": "choice", "choice": "unresolved"}}
    }));
    let adapter = GeneralDecisionEndpoint::new(endpoint.clone());
    let result = adapter
        .evaluate(&json!({"mention": "Ada"}), &choice_questions())
        .unwrap();
    assert_eq!(result["answers"]["resolution"]["choice"], "unresolved");
    let observed = endpoint.observed.lock().unwrap();
    let (prompt, payload, schema_name, schema) = observed.as_ref().unwrap();
    assert!(prompt.contains("Resolve only the supplied mention."));
    assert_eq!(schema_name, "decision_answers");
    assert_eq!(
        serde_json::from_str::<Value>(payload).unwrap()["state"]["mention"],
        "Ada"
    );
    assert_eq!(
        schema["properties"]["answers"]["properties"]["resolution"]["properties"]["choice"]["enum"],
        json!(["candidate_0", "unresolved"])
    );
}

#[test]
fn general_fallback_rejects_unknown_or_invalid_choices() {
    for response in [
        json!({}),
        json!({"answers": {}}),
        json!({"answers": {"resolution": {"type": "choice", "choice": "invented"}}}),
        json!({"answers": {"resolution": {"type": "choice", "choice": "candidate_0", "confidence": 1.0}}}),
    ] {
        let adapter = GeneralDecisionEndpoint::new(general(response));
        assert!(matches!(
            adapter.evaluate(&json!({}), &choice_questions()),
            Err(DecisionEndpointError::InvalidResponse(_))
        ));
    }
}

#[test]
fn general_fallback_preserves_explicit_json_schema_requests() {
    let schema = json!({"type": "object", "required": ["approved"]});
    let response = json!({"approved": ["id-1"]});
    let endpoint = general(response.clone());
    let adapter = GeneralDecisionEndpoint::new(endpoint.clone());
    assert_eq!(
        adapter
            .evaluate(
                &json!({"operation": "retrieval"}),
                &json!({"instruction": "Choose from supplied IDs.", "schema": schema})
            )
            .unwrap(),
        response
    );
    assert_eq!(
        endpoint.observed.lock().unwrap().as_ref().unwrap().2,
        "decision_response"
    );
}

#[test]
fn general_fallback_normalizes_const_for_strict_provider_schemas() {
    let endpoint = general(json!({"version": 1}));
    let adapter = GeneralDecisionEndpoint::new(endpoint.clone());
    let response = adapter
        .evaluate(
            &json!({"operation": "retrieval"}),
            &json!({
                "instruction": "Return version one.",
                "schema": {
                    "type": "object",
                    "required": ["version"],
                    "additionalProperties": false,
                    "properties": {"version": {"const": 1}}
                }
            }),
        )
        .unwrap();
    assert_eq!(response, json!({"version": 1}));
    let observed = endpoint.observed.lock().unwrap();
    let schema = &observed.as_ref().unwrap().3;
    assert!(schema["properties"]["version"].get("const").is_none());
    assert_eq!(schema["properties"]["version"]["type"], "integer");
    assert_eq!(schema["properties"]["version"]["enum"], json!([1]));
}

#[test]
fn endpoint_resolution_uses_selected_binding_and_reports_unavailable() {
    struct DecisionStub;
    impl DecisionEndpoint for DecisionStub {
        fn model(&self) -> &str {
            "dedicated-model"
        }
        fn evaluate(
            &self,
            _state: &Value,
            _questions: &Value,
        ) -> Result<Value, DecisionEndpointError> {
            Ok(json!({"selected": "dedicated"}))
        }
    }

    let dedicated: Arc<dyn DecisionEndpoint> = Arc::new(DecisionStub);
    let config = ModelSwitchboardConfig {
        entity_resolution_decision: Some(DecisionModelEndpoint {
            model: "dedicated-model".into(),
            url: "https://example.test/entity".into(),
            credential_id: CredentialId::new("entity").unwrap(),
        }),
        ..Default::default()
    };
    let endpoint = resolve_decision_endpoint(
        &config,
        DecisionConsumer::EntityResolution,
        Some(dedicated),
        None,
        None,
    )
    .unwrap()
    .unwrap();
    assert_eq!(endpoint.model(), "dedicated-model");

    assert!(
        resolve_decision_endpoint(
            &ModelSwitchboardConfig::default(),
            DecisionConsumer::Chronos,
            None,
            None,
            None,
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn configured_general_http_sends_structured_request_with_api_key() {
    let (url, server) = mock_server(
        "200 OK",
        "Content-Type: application/json\r\n",
        r#"{"choices":[{"message":{"content":"{\"ok\":true}"}}]}"#,
    );
    let route = GeneralModelEndpoint {
        provider: ModelProvider::OpenAiReady,
        model: "mock-model".into(),
        url,
        credential_id: CredentialId::new("general").unwrap(),
    };
    let endpoint =
        ConfiguredGeneralEndpoint::from_route(&route, &credential_store("general", "secret-value"))
            .unwrap();
    assert_eq!(
        endpoint
            .complete_json("system", "payload", "answer", &json!({"type":"object"}))
            .unwrap(),
        json!({"ok": true})
    );
    let request = String::from_utf8(server.join().unwrap()).unwrap();
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer secret-value")
    );
    let request_body: Value =
        serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(request_body["model"], "mock-model");
    assert_eq!(
        request_body["response_format"]["json_schema"]["name"],
        "answer"
    );
}

#[test]
fn configured_decision_http_redacts_backpressure_body_and_keeps_retry_after() {
    let (url, server) = mock_server(
        "429 Too Many Requests",
        "Retry-After: 7\r\n",
        r#"{"error":"secret-value"}"#,
    );
    let route = DecisionModelEndpoint {
        model: "decision-model".into(),
        url,
        credential_id: CredentialId::new("decision").unwrap(),
    };
    let endpoint = ConfiguredDecisionEndpoint::from_route(
        &route,
        &credential_store("decision", "secret-value"),
    )
    .unwrap();
    match endpoint.evaluate(&json!({"state":1}), &json!({"q":2})) {
        Err(DecisionEndpointError::Backpressure {
            message,
            retry_after,
        }) => {
            assert!(!message.contains("secret-value"));
            assert_eq!(message, "provider returned HTTP 429");
            assert_eq!(retry_after, Some(Duration::from_secs(7)));
        }
        result => panic!("unexpected result: {result:?}"),
    }
    let request = String::from_utf8(server.join().unwrap()).unwrap();
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer secret-value")
    );
}

#[test]
fn codex_http_uses_oauth_account_and_completed_event_stream() {
    let body = concat!(
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"ok\\\":true}\"}\n\n",
        "data: {\"type\":\"response.completed\"}\n\n",
        "data: [DONE]\n\n"
    );
    let (url, server) = mock_server("200 OK", "Content-Type: text/event-stream\r\n", body);
    let id = CredentialId::new("codex").unwrap();
    let route = GeneralModelEndpoint {
        provider: ModelProvider::OpenAiCodex,
        model: "gpt-5.6-test".into(),
        url,
        credential_id: id.clone(),
    };
    let mut credentials = CredentialsConfig::default();
    credentials
        .insert_codex_oauth(id, "oauth-secret", "account-42")
        .unwrap();
    let endpoint = ConfiguredGeneralEndpoint::from_route(&route, &credentials).unwrap();
    assert_eq!(
        endpoint
            .complete_json("system", "payload", "answer", &json!({"type":"object"}))
            .unwrap(),
        json!({"ok": true})
    );
    let request = String::from_utf8(server.join().unwrap()).unwrap();
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer oauth-secret")
    );
    assert!(
        request
            .to_ascii_lowercase()
            .contains("chatgpt-account-id: account-42")
    );
    assert!(request.to_ascii_lowercase().contains("version: 0.156.0"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("x-openai-internal-codex-responses-lite: true")
    );
    let request_body: Value =
        serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(request_body["store"], false);
    assert_eq!(request_body["stream"], true);
    assert_eq!(request_body["service_tier"], "priority");
    assert_eq!(request_body["reasoning"]["context"], "all_turns");
}

#[test]
fn configured_embedding_http_is_authenticated_bounded_and_decodes_indices() {
    let body = r#"{"data":[{"index":1,"embedding":[0.0,1.0]},{"index":0,"embedding":[3.0,4.0]}]}"#;
    let (url, server) = mock_server("200 OK", "Content-Type: application/json\r\n", body);
    let id = CredentialId::new("embed-key").unwrap();
    let route = EmbeddingModelEndpoint {
        provider: ModelProvider::OpenAiReady,
        model: "embed-model".into(),
        url,
        credential_id: id.clone(),
        dimensions: 2,
        normalization: EmbeddingNormalization::L2,
    };
    let endpoint = ConfiguredEmbeddingEndpoint::from_route(
        &route,
        &credential_store("embed-key", "secret-token"),
    )
    .unwrap();
    let vectors = endpoint
        .embed_bounded(
            EmbeddingMode::Query,
            &["first".into(), "second".into()],
            256,
        )
        .unwrap();
    assert_eq!(vectors, vec![vec![0.6, 0.8], vec![0.0, 1.0]]);
    let request = String::from_utf8(server.join().unwrap()).unwrap();
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer secret-token")
    );
    let body: Value = serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["input_type"], "search_query");
    assert_eq!(body["dimensions"], 2);

    let (url, server) = mock_server(
        "200 OK",
        "Content-Type: application/json\r\n",
        body_string_oversize(),
    );
    let route = EmbeddingModelEndpoint { url, ..route };
    let endpoint = ConfiguredEmbeddingEndpoint::from_route(
        &route,
        &credential_store("embed-key", "secret-token"),
    )
    .unwrap();
    assert!(matches!(
        endpoint.embed_bounded(EmbeddingMode::Document, &["x".into()], 8),
        Err(EmbeddingEndpointError::InvalidResponse(
            "bounded response exceeded"
        ))
    ));
    server.join().unwrap();
}

fn body_string_oversize() -> &'static str {
    "01234567890123456789"
}

#[test]
fn codex_stream_rejects_failed_incomplete_and_truncated_responses() {
    for (body, provider_failed) in [
        (
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"ok\\\":true}\"}\n\ndata: {\"type\":\"response.failed\"}\n\n",
            true,
        ),
        ("data: {\"type\":\"response.incomplete\"}\n\n", true),
        (
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"ok\\\":true}\"}\n\n",
            false,
        ),
    ] {
        let (url, server) = mock_server("200 OK", "Content-Type: text/event-stream\r\n", body);
        let credential = CredentialId::new("codex-failure").unwrap();
        let route = GeneralModelEndpoint {
            provider: ModelProvider::OpenAiCodex,
            model: "codex-model".into(),
            url,
            credential_id: credential.clone(),
        };
        let mut credentials = CredentialsConfig::default();
        credentials
            .insert_codex_oauth(credential, "oauth-secret", "account-42")
            .unwrap();
        let endpoint = ConfiguredGeneralEndpoint::from_route(&route, &credentials).unwrap();
        let result =
            endpoint.complete_json("system", "payload", "answer", &json!({"type":"object"}));
        if provider_failed {
            assert!(matches!(result, Err(GeneralEndpointError::Failure(_))));
        } else {
            assert!(matches!(
                result,
                Err(GeneralEndpointError::InvalidResponse(_))
            ));
        }
        server.join().unwrap();
    }
}
