use super::ConfiguredRuntime;
use crate::{
    AgenticMemoryContent, AgenticMemoryOperation, CredentialId, CredentialsConfig,
    DecisionEndpoint, DecisionModelEndpoint, GeneralModelEndpoint, MemoryId, MemoryRef,
    ModelProvider, ModelSwitchboard, ModelSwitchboardConfig,
};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
enum Consumer {
    EntityResolution,
    MemoryRetrieval,
}

#[derive(Clone, Copy)]
enum Tier {
    Dedicated,
    Shared,
    General,
}

struct TestServer {
    address: String,
    requests: thread::JoinHandle<Vec<String>>,
}

fn test_server(status: u16, response: Value, collect_for: Duration) -> TestServer {
    test_server_responses(status, vec![response], collect_for)
}

fn test_server_responses(status: u16, responses: Vec<Value>, collect_for: Duration) -> TestServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let requests = thread::spawn(move || {
        let request_deadline = Instant::now() + Duration::from_secs(5);
        let mut settle_deadline = None;
        let mut requests = Vec::new();
        loop {
            if settle_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                break;
            }
            if settle_deadline.is_none() && Instant::now() >= request_deadline {
                panic!("local test server timed out waiting for a request");
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let request = read_request(&mut stream);
                    let response_index = requests.len().min(responses.len() - 1);
                    let response = responses[response_index].clone();
                    let body = response.to_string();
                    requests.push(request);
                    let reason = if (200..300).contains(&status) {
                        "OK"
                    } else {
                        "Service Unavailable"
                    };
                    write!(
                        stream,
                        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .unwrap();
                    if requests.len() >= responses.len() {
                        settle_deadline = Some(Instant::now() + collect_for);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("local test server accept failed: {error}"),
            }
        }
        requests
    });
    TestServer {
        address: format!("http://{address}"),
        requests,
    }
}

fn read_request(stream: &mut TcpStream) -> String {
    // Windows may inherit the listener's nonblocking mode on accepted streams.
    stream.set_nonblocking(false).unwrap();
    let mut request = Vec::new();
    let mut chunk = [0_u8; 4096];
    let mut complete = false;
    loop {
        let count = stream.read(&mut chunk).unwrap();
        if count == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..count]);
        let Some(header_end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        if request.len() >= header_end + 4 + content_length {
            complete = true;
            break;
        }
    }
    assert!(complete, "local server received an incomplete HTTP request");
    String::from_utf8(request).unwrap()
}

fn config(server: &TestServer, consumer: Consumer, tier: Tier) -> ModelSwitchboard {
    let general_id = CredentialId::new("general-auth").unwrap();
    let shared_id = CredentialId::new("shared-auth").unwrap();
    let entity_id = CredentialId::new("entity-auth").unwrap();
    let memory_id = CredentialId::new("memory-auth").unwrap();
    let mut models = ModelSwitchboardConfig {
        general: Some(GeneralModelEndpoint {
            provider: ModelProvider::OpenAiReady,
            model: "general-model".into(),
            url: Some(format!("{}/chat/completions", server.address)),
            credential_id: general_id.clone(),
            reasoning_effort: None,
        }),
        ..ModelSwitchboardConfig::default()
    };
    if matches!(tier, Tier::Shared | Tier::Dedicated) {
        models.general_decision = Some(DecisionModelEndpoint {
            model: "shared-model".into(),
            url: format!("{}/shared", server.address),
            credential_id: shared_id.clone(),
        });
    }
    if matches!(tier, Tier::Dedicated) {
        match consumer {
            Consumer::EntityResolution => {
                models.entity_resolution_decision = Some(DecisionModelEndpoint {
                    model: "entity-model".into(),
                    url: format!("{}/entity", server.address),
                    credential_id: entity_id.clone(),
                });
            }
            Consumer::MemoryRetrieval => {
                models.memory_retrieval_decision = Some(DecisionModelEndpoint {
                    model: "memory-model".into(),
                    url: format!("{}/memory", server.address),
                    credential_id: memory_id.clone(),
                });
            }
        }
    }
    let mut credentials = CredentialsConfig::default();
    credentials
        .insert_api_key(general_id, "general-secret")
        .unwrap();
    if matches!(tier, Tier::Shared | Tier::Dedicated) {
        credentials
            .insert_api_key(shared_id, "shared-secret")
            .unwrap();
    }
    if matches!(tier, Tier::Dedicated) {
        let id = match consumer {
            Consumer::EntityResolution => entity_id,
            Consumer::MemoryRetrieval => memory_id,
        };
        credentials.insert_api_key(id, "dedicated-secret").unwrap();
    }
    ModelSwitchboard::new(models, credentials).unwrap()
}

fn runtime(server: &TestServer, consumer: Consumer, tier: Tier) -> ConfiguredRuntime {
    ConfiguredRuntime {
        switchboard: config(server, consumer, tier),
    }
}

fn decision_route(
    routes: &crate::ReliquaryRuntimeRoutes,
    consumer: Consumer,
) -> Arc<dyn DecisionEndpoint> {
    match consumer {
        Consumer::EntityResolution => routes.entity_resolution_decision().unwrap(),
        Consumer::MemoryRetrieval => routes.memory_retrieval_decision().unwrap(),
    }
}

fn questions(consumer: Consumer) -> Value {
    match consumer {
        Consumer::EntityResolution => json!({
            "resolution": {
                "type": "choice",
                "instructions": "Choose the safe resolution.",
                "criteria": {"unresolved": "Do not merge."}
            }
        }),
        Consumer::MemoryRetrieval => json!({
            "instruction": "Select approved memories.",
            "schema": {
                "type": "object",
                "required": ["version", "approved"],
                "additionalProperties": false,
                "properties": {
                    "version": {"const": 1},
                    "approved": {"type": "array", "items": {"type": "string"}}
                }
            }
        }),
    }
}

fn general_response(consumer: Consumer) -> Value {
    let content = match consumer {
        Consumer::EntityResolution => json!({
            "answers": {"resolution": {"type": "choice", "choice": "unresolved"}}
        }),
        Consumer::MemoryRetrieval => json!({"version": 1, "approved": []}),
    };
    general_response_content(content)
}

fn general_response_content(content: Value) -> Value {
    json!({"choices": [{"message": {"content": content.to_string()}}]})
}

fn direct_response(consumer: Consumer) -> Value {
    match consumer {
        Consumer::EntityResolution => json!({
            "answers": {"resolution": {"type": "choice", "choice": "unresolved"}}
        }),
        Consumer::MemoryRetrieval => json!({"version": 1, "approved": []}),
    }
}

#[test]
fn temporal_selection_uses_shared_or_general_configured_route() {
    for tier in [Tier::Shared, Tier::General] {
        let selected = json!({"answers":{"u000":{"type":"choice","choice":"u000c00"}}});
        let response = if matches!(tier, Tier::General) {
            general_response_content(selected)
        } else {
            selected
        };
        let server = test_server(200, response, Duration::from_millis(50));
        let routes = runtime(&server, Consumer::EntityResolution, tier)
            .runtime_routes()
            .unwrap();
        let endpoint = routes.temporal_decision().unwrap();
        let inference = crate::TemporalDisambiguator::new(endpoint.as_ref())
            .select("biweekly", None)
            .unwrap()
            .unwrap();
        assert_eq!(
            inference.resolutions[0].canonical_expression,
            "every two weeks"
        );
        let requests = server.requests.join().unwrap();
        assert_eq!(requests.len(), 1);
        let path = if matches!(tier, Tier::Shared) {
            "/shared"
        } else {
            "/chat/completions"
        };
        assert!(requests[0].starts_with(&format!("POST {path} ")));
    }
    let runtime = ConfiguredRuntime {
        switchboard: ModelSwitchboard::new(
            ModelSwitchboardConfig::default(),
            CredentialsConfig::default(),
        )
        .unwrap(),
    };
    assert!(
        runtime
            .runtime_routes()
            .unwrap()
            .temporal_decision()
            .is_none()
    );
}

fn assert_request(request: &str, consumer: Consumer, tier: Tier) {
    let (headers, body) = request.split_once("\r\n\r\n").unwrap();
    let path = match tier {
        Tier::Dedicated => match consumer {
            Consumer::EntityResolution => "/entity",
            Consumer::MemoryRetrieval => "/memory",
        },
        Tier::Shared => "/shared",
        Tier::General => "/chat/completions",
    };
    assert!(request.starts_with(&format!("POST {path} ")), "{request}");
    let auth = match tier {
        Tier::Dedicated => "dedicated-secret",
        Tier::Shared => "shared-secret",
        Tier::General => "general-secret",
    };
    assert!(
        headers
            .to_ascii_lowercase()
            .contains(&format!("authorization: bearer {auth}")),
        "{headers}"
    );
    let body: Value = serde_json::from_str(body).unwrap();
    match tier {
        Tier::Dedicated => assert_eq!(
            body["model"],
            match consumer {
                Consumer::EntityResolution => "entity-model",
                Consumer::MemoryRetrieval => "memory-model",
            }
        ),
        Tier::Shared => assert_eq!(body["model"], "shared-model"),
        Tier::General => assert_eq!(body["model"], "general-model"),
    }
    match (consumer, tier) {
        (Consumer::MemoryRetrieval, Tier::General) => {
            let payload: Value =
                serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
            assert_eq!(payload["questions"], questions(consumer));
            assert_eq!(
                body["response_format"]["json_schema"]["schema"],
                questions(consumer)["schema"]
            );
        }
        (Consumer::EntityResolution, Tier::General) => {
            assert_eq!(body["response_format"]["json_schema"]["strict"], true);
            assert_eq!(
                body["response_format"]["json_schema"]["schema"]["properties"]["answers"]["properties"]
                    ["resolution"]["properties"]["choice"]["enum"],
                json!(["unresolved"])
            );
        }
        _ => {
            assert_eq!(body["questions"], questions(consumer));
            assert_eq!(body["state"], json!({"owner_id":"owner-1"}));
        }
    }
}

fn assert_request_identity(request: &str, consumer: Consumer, tier: Tier) {
    let (headers, raw_body) = request.split_once("\r\n\r\n").unwrap();
    let (path, auth, model) = match tier {
        Tier::Dedicated => match consumer {
            Consumer::EntityResolution => ("/entity", "dedicated-secret", "entity-model"),
            Consumer::MemoryRetrieval => ("/memory", "dedicated-secret", "memory-model"),
        },
        Tier::Shared => ("/shared", "shared-secret", "shared-model"),
        Tier::General => ("/chat/completions", "general-secret", "general-model"),
    };
    assert!(request.starts_with(&format!("POST {path} ")), "{request}");
    assert!(
        headers
            .to_ascii_lowercase()
            .contains(&format!("authorization: bearer {auth}")),
        "{headers}"
    );
    let body: Value = serde_json::from_str(raw_body).unwrap();
    assert_eq!(body["model"], model);
}

#[test]
fn runtime_routes_select_each_tier_with_consumer_specific_auth_and_contracts() {
    for consumer in [Consumer::EntityResolution, Consumer::MemoryRetrieval] {
        for tier in [Tier::Dedicated, Tier::Shared, Tier::General] {
            let response = if matches!(tier, Tier::General) {
                general_response(consumer)
            } else {
                direct_response(consumer)
            };
            let server = test_server(200, response, Duration::from_millis(150));
            let routes = runtime(&server, consumer, tier).runtime_routes().unwrap();
            let endpoint = decision_route(&routes, consumer);
            assert_eq!(
                endpoint.model(),
                match tier {
                    Tier::Dedicated => match consumer {
                        Consumer::EntityResolution => "entity-model",
                        Consumer::MemoryRetrieval => "memory-model",
                    },
                    Tier::Shared => "shared-model",
                    Tier::General => "general-model",
                }
            );
            let result = endpoint
                .evaluate(&json!({"owner_id":"owner-1"}), &questions(consumer))
                .unwrap();
            assert_eq!(result, direct_response(consumer));
            let requests = server.requests.join().unwrap();
            assert_eq!(requests.len(), 1, "selected route made unexpected requests");
            assert_request(&requests[0], consumer, tier);
        }
    }
}

#[test]
fn entity_engine_uses_selected_general_or_shared_decision_with_existing_resolver() {
    use crate::entity_candidate_test_support::{
        entity_draft, install_rel_mention, publish_rel_memory, temp_path,
    };
    use crate::{
        Cva, EntityCandidateConfig, EntityResolutionDecision, EntityResolutionEngine,
        SimulatedGeneralEndpoint,
    };

    for tier in [Tier::General, Tier::Shared] {
        let answer = json!({
            "answers": {"resolution": {"type": "choice", "choice": "candidate_0"}}
        });
        let response = if matches!(tier, Tier::General) {
            general_response_content(answer)
        } else {
            answer
        };
        let server = test_server(200, response, Duration::from_millis(150));
        let routes = runtime(&server, Consumer::EntityResolution, tier)
            .runtime_routes()
            .unwrap();
        let decision = decision_route(&routes, Consumer::EntityResolution);

        let mut rel =
            Cva::create_project(temp_path("runtime-selected-decision-engine.rel")).unwrap();
        let memory = publish_rel_memory(
            &mut rel,
            "runtime-selected-decision-engine",
            "Editor",
            "Zephyr is the editor used for this project.",
        );
        let mention = install_rel_mention(&mut rel, memory, "Zephyr");
        let entity_id = crate::EntityId([0x37; 32]);
        rel.publish_entity(
            Some(entity_id),
            0,
            entity_draft("Zephyr", &[], "runtime-decision-candidate", 1),
        )
        .unwrap();
        let resolver = SimulatedGeneralEndpoint::new(
            "separate-entity-resolver",
            vec![json!({
                "decision": "unresolved",
                "reason": "insufficient_evidence",
                "target_candidate_index": -1
            })],
        );
        let engine = EntityResolutionEngine::new(resolver).with_decision_endpoint(Some(decision));
        let outcome = rel
            .resolve_entity_mention_with_engine(
                &engine,
                mention,
                EntityCandidateConfig::default(),
                330,
            )
            .unwrap();

        assert_eq!(outcome.decision, EntityResolutionDecision::Unresolved);
        assert_eq!(outcome.entity_id, None);
        assert_eq!(rel.entities().len(), 1);
        assert_eq!(rel.entities()[0].id, entity_id);
        let requests = server.requests.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert_request_identity(&requests[0], Consumer::EntityResolution, tier);
    }
}

#[test]
fn memory_selection_and_rewrite_controllers_use_selected_general_or_shared_route() {
    use crate::agentic_memory_retrieval_decision::{rewrite_query, select_memories};

    for tier in [Tier::General, Tier::Shared] {
        let owner_id = "owner-1";
        let memory_id = MemoryId([0x2a; 32]);
        let memory = AgenticMemoryContent {
            memory: MemoryRef {
                owner_id: owner_id.into(),
                memory_id,
            },
            title: "Basketball notes".into(),
            content: "A complete note about basketball.".into(),
        };
        let selection = json!({
            "version": 1,
            "approved": [{
                "owner_id": owner_id,
                "memory_id": "2a".repeat(32)
            }]
        });
        let rewrite = json!({"version": 1, "query": "basketball games this week"});
        let responses = if matches!(tier, Tier::General) {
            vec![
                general_response_content(selection),
                general_response_content(rewrite),
            ]
        } else {
            vec![selection, rewrite]
        };
        let server = test_server_responses(200, responses, Duration::from_millis(200));
        let routes = runtime(&server, Consumer::MemoryRetrieval, tier)
            .runtime_routes()
            .unwrap();
        let endpoint = decision_route(&routes, Consumer::MemoryRetrieval);
        let selected = select_memories(
            endpoint.as_ref(),
            AgenticMemoryOperation::Query,
            "basketball this week",
            std::slice::from_ref(&memory),
            None,
            &[],
        )
        .unwrap();
        let rewritten = rewrite_query(
            endpoint.as_ref(),
            "baseball this week",
            "basketball instead",
        )
        .unwrap();

        assert_eq!(selected, std::collections::BTreeSet::from([memory.memory]));
        assert_eq!(rewritten, "basketball games this week");
        let requests = server.requests.join().unwrap();
        assert_eq!(requests.len(), 2);
        for (index, request) in requests.iter().enumerate() {
            assert_request_identity(request, Consumer::MemoryRetrieval, tier);
            let (headers, raw_body) = request.split_once("\r\n\r\n").unwrap();
            let body: Value = serde_json::from_str(raw_body).unwrap();
            let (state, questions) = if matches!(tier, Tier::General) {
                let payload: Value =
                    serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
                assert_eq!(
                    body["response_format"]["json_schema"]["schema"],
                    payload["questions"]["schema"]
                );
                (payload["state"].clone(), payload["questions"].clone())
            } else {
                (body["state"].clone(), body["questions"].clone())
            };
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("authorization: bearer")
            );
            if index == 0 {
                assert_eq!(state["operation"], "memory_query");
                assert_eq!(state["query"], "basketball this week");
                assert_eq!(
                    questions["schema"],
                    serde_json::from_str::<Value>(
                        crate::agentic_memory_retrieval_decision::MEMORY_SELECTION_SCHEMA_V1,
                    )
                    .unwrap()
                );
                assert_eq!(
                    questions["instruction"],
                    crate::agentic_memory_retrieval_decision::MEMORY_SELECTION_INSTRUCTION_V1
                );
            } else {
                assert_eq!(state["operation"], "refine_query_rewrite");
                assert_eq!(state["current_query"], "baseball this week");
                assert_eq!(state["refinement"], "basketball instead");
                assert_eq!(
                    questions["schema"],
                    serde_json::from_str::<Value>(
                        crate::agentic_memory_retrieval_decision::MEMORY_REWRITE_SCHEMA_V1,
                    )
                    .unwrap()
                );
                assert_eq!(
                    questions["instruction"],
                    crate::agentic_memory_retrieval_decision::MEMORY_REWRITE_INSTRUCTION_V1
                );
            }
        }
    }
}

#[test]
fn selected_dedicated_or_shared_failure_does_not_retry_a_lower_tier() {
    for (consumer, tier) in [
        (Consumer::EntityResolution, Tier::Dedicated),
        (Consumer::MemoryRetrieval, Tier::Shared),
    ] {
        let server = test_server(
            503,
            json!({"error":"selected route failed"}),
            Duration::from_millis(350),
        );
        let routes = runtime(&server, consumer, tier).runtime_routes().unwrap();
        let endpoint = decision_route(&routes, consumer);
        assert!(
            endpoint
                .evaluate(&json!({"owner_id":"owner-1"}), &questions(consumer))
                .is_err()
        );
        let requests = server.requests.join().unwrap();
        assert_eq!(requests.len(), 1, "a lower decision tier was retried");
        assert_request(&requests[0], consumer, tier);
    }
}
