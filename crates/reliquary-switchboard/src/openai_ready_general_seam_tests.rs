use super::*;
use crate::GeneralDecisionEndpoint;
use crate::{
    ConfiguredGeneralEndpoint, CredentialId, CredentialsConfig, DecisionEndpoint,
    GeneralModelEndpoint, ModelProvider, ModelSwitchboard, ModelSwitchboardConfig,
};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn local_response(body: String) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut chunk = [0_u8; 4096];
        loop {
            let count = stream.read(&mut chunk).unwrap();
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
                break;
            }
        }
        let request = String::from_utf8(request).unwrap();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
        request
    });
    (format!("http://{address}/v1/chat/completions"), handle)
}

fn switchboard(url: String) -> ModelSwitchboard {
    let id = CredentialId::new("general-key").unwrap();
    let mut config = ModelSwitchboardConfig::default();
    config.general = Some(GeneralModelEndpoint {
        provider: ModelProvider::OpenAiReady,
        model: "ready-test-model".into(),
        url: Some(url),
        credential_id: id.clone(),
        reasoning_effort: None,
    });
    let mut credentials = CredentialsConfig::default();
    credentials.insert_api_key(id, "ready-test-secret").unwrap();
    ModelSwitchboard::new(config, credentials).unwrap()
}

#[test]
fn ready_general_adapter_uses_real_structured_transport_and_preserves_schema_and_auth() {
    let response = json!({
        "choices": [{"message": {"content": "{\"answers\":{\"resolution\":{\"type\":\"choice\",\"choice\":\"unresolved\"}}}"}}]
    });
    let (url, server) = local_response(response.to_string());
    let switchboard = switchboard(url);
    let general = Arc::new(OpenAiReadyGeneralEndpoint::from_switchboard(&switchboard).unwrap());
    let general = Arc::new(ConfiguredGeneralEndpoint::OpenAiReady((*general).clone()));
    let adapter = GeneralDecisionEndpoint::new(general);
    let questions = json!({
        "resolution": {
            "type": "choice", "instructions": "Choose one.",
            "criteria": {"candidate_0": "match", "unresolved": "uncertain"}
        }
    });
    let result = adapter
        .evaluate(&json!({"mention":"Ada"}), &questions)
        .unwrap();
    assert_eq!(result["answers"]["resolution"]["choice"], "unresolved");
    let request = server.join().unwrap();
    assert!(!request.is_empty());
    let headers = request
        .split("\r\n\r\n")
        .next()
        .unwrap()
        .to_ascii_lowercase();
    let body = request.split_once("\r\n\r\n").unwrap().1;
    assert!(headers.contains("authorization: bearer ready-test-secret"));
    let body: Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["model"], "ready-test-model");
    assert_eq!(body["temperature"], 0);
    assert_eq!(body["response_format"]["json_schema"]["strict"], true);
    assert_eq!(
        body["response_format"]["json_schema"]["schema"]["properties"]["answers"]["properties"]["resolution"]
            ["properties"]["choice"]["enum"],
        json!(["candidate_0", "unresolved"])
    );
    assert!(
        body["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("Ada")
    );
}

#[test]
fn ready_general_adapter_propagates_malformed_structured_json() {
    let response = json!({"choices": [{"message": {"content": "not-json"}}]});
    let (url, server) = local_response(response.to_string());
    let switchboard = switchboard(url);
    let general = OpenAiReadyGeneralEndpoint::from_switchboard(&switchboard).unwrap();
    let general = Arc::new(ConfiguredGeneralEndpoint::OpenAiReady(general));
    let adapter = GeneralDecisionEndpoint::new(general);
    let result = adapter.evaluate(
        &json!({}),
        &json!({"resolution":{"type":"choice","instructions":"Choose.","criteria":{"x":"X"}}}),
    );
    assert!(matches!(
        result,
        Err(crate::DecisionEndpointError::Failure(_))
    ));
    assert!(!server.join().unwrap().is_empty());
}
