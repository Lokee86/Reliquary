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

fn local_sse_response(body: String) -> (String, thread::JoinHandle<String>) {
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
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
        request
    });
    (format!("http://{address}/responses"), handle)
}

#[test]
fn codex_general_adapter_uses_oauth_account_reasoning_schema_and_sse_parser() {
    let output = json!({
        "answers": {"resolution": {"type": "choice", "choice": "unresolved"}}
    })
    .to_string();
    let output_delta = json!({
        "type": "response.output_text.delta", "delta": output
    })
    .to_string();
    let completed = json!({"type": "response.completed", "response": {"id":"local"}}).to_string();
    let stream = format!(
        "event: response.output_text.delta\ndata: {output_delta}\n\nevent: response.completed\ndata: {completed}\n\ndata: [DONE]\n\n"
    );
    let (url, server) = local_sse_response(stream);
    let id = CredentialId::new("codex-test").unwrap();
    let mut config = ModelSwitchboardConfig::default();
    config.general = Some(GeneralModelEndpoint {
        provider: ModelProvider::OpenAiCodex,
        model: "gpt-5.6-luna".into(),
        url: None,
        credential_id: id.clone(),
        reasoning_effort: Some(crate::ModelReasoningEffort::Low),
    });
    let mut credentials = CredentialsConfig::default();
    credentials
        .insert_chatgpt_oauth(
            id,
            "codex-id-token",
            "codex-access-token",
            "codex-refresh-token",
            Some("codex-account".into()),
        )
        .unwrap();
    let switchboard = ModelSwitchboard::new(config, credentials).unwrap();
    let mut endpoint = OpenAiCodexGeneralEndpoint::from_switchboard(&switchboard).unwrap();
    endpoint.url = url;
    let general = Arc::new(ConfiguredGeneralEndpoint::OpenAiCodex(endpoint));
    let adapter = GeneralDecisionEndpoint::new(general);
    let questions = json!({
        "resolution": {
            "type": "choice", "instructions": "Choose one.",
            "criteria": {"candidate_0": "match", "unresolved": "uncertain"}
        }
    });
    let response = adapter
        .evaluate(&json!({"mention":"Ada"}), &questions)
        .unwrap();
    assert_eq!(response["answers"]["resolution"]["choice"], "unresolved");

    let request = server.join().unwrap();
    assert!(!request.is_empty());
    let headers = request
        .split("\r\n\r\n")
        .next()
        .unwrap()
        .to_ascii_lowercase();
    let body = request.split_once("\r\n\r\n").unwrap().1;
    assert!(headers.contains("authorization: bearer codex-access-token"));
    assert!(headers.contains("chatgpt-account-id: codex-account"));
    assert!(headers.contains("accept: text/event-stream"));
    let body: Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["model"], "gpt-5.6-luna");
    assert_eq!(body["reasoning"]["effort"], "low");
    assert_eq!(body["text"]["format"]["strict"], true);
    assert_eq!(
        body["text"]["format"]["schema"]["properties"]["answers"]["properties"]["resolution"]["properties"]
            ["choice"]["enum"],
        json!(["candidate_0", "unresolved"])
    );
    assert!(
        body["input"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Ada")
    );
}

#[test]
fn codex_general_adapter_propagates_failed_incomplete_sse_response() {
    let stream = concat!(
        "event: response.failed\n",
        "data: {\"type\":\"response.failed\",\"response\":{\"id\":\"failed\"}}\n\n"
    );
    let (url, server) = local_sse_response(stream.into());
    let id = CredentialId::new("codex-failure").unwrap();
    let mut config = ModelSwitchboardConfig::default();
    config.general = Some(GeneralModelEndpoint {
        provider: ModelProvider::OpenAiCodex,
        model: "gpt-5.6-luna".into(),
        url: None,
        credential_id: id.clone(),
        reasoning_effort: Some(crate::ModelReasoningEffort::Low),
    });
    let mut credentials = CredentialsConfig::default();
    credentials
        .insert_chatgpt_oauth(
            id,
            "codex-id-token",
            "codex-access-token",
            "codex-refresh-token",
            Some("codex-account".into()),
        )
        .unwrap();
    let switchboard = ModelSwitchboard::new(config, credentials).unwrap();
    let mut endpoint = OpenAiCodexGeneralEndpoint::from_switchboard(&switchboard).unwrap();
    endpoint.url = url;
    let general = Arc::new(ConfiguredGeneralEndpoint::OpenAiCodex(endpoint));
    let adapter = GeneralDecisionEndpoint::new(general);
    let questions = json!({
        "resolution": {
            "type": "choice", "instructions": "Choose one.", "criteria": {"x":"X"}
        }
    });
    assert!(matches!(
        adapter.evaluate(&json!({}), &questions),
        Err(crate::DecisionEndpointError::Failure(_))
    ));
    assert!(!server.join().unwrap().is_empty());
}
