use crate::openai_ready_general::{
    parse_forced_tool_result, parse_streamed_forced_tool_result, parse_structured_content,
};
use serde_json::json;

#[test]
fn structured_content_accepts_raw_json() {
    let value = parse_structured_content(r#"{"ok":true}"#).unwrap();
    assert_eq!(value["ok"], true);
}

#[test]
fn structured_content_accepts_json_code_fences() {
    let value = parse_structured_content("```json\n{\"ok\":true}\n```\n").unwrap();
    assert_eq!(value["ok"], true);
}

#[test]
fn structured_content_rejects_unstructured_prose() {
    assert!(parse_structured_content("the answer is {\"ok\":true}").is_err());
}

#[test]
fn forced_tool_result_requires_exact_named_single_call() {
    let response = json!({
        "choices": [{
            "message": {
                "tool_calls": [{
                    "type": "function",
                    "function": {
                        "name": "dream_pair_classification",
                        "arguments": "{\"relation\":\"none\",\"direction\":\"none\",\"evidence\":[]}"
                    }
                }]
            }
        }]
    });
    let value = parse_forced_tool_result(&response, "dream_pair_classification").unwrap();
    assert_eq!(value["relation"], "none");
    assert!(value["evidence"].as_array().unwrap().is_empty());
}

#[test]
fn streamed_forced_tool_result_reassembles_arguments() {
    let stream = concat!(
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"insomnia_ledger\",\"arguments\":\"{\\\"turns\\\":\"}}]}}]}\n",
        "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{},\\\"evidence_requests\\\":[]}\"}}]}}]}\n",
        "data: [DONE]\n"
    );
    let value = parse_streamed_forced_tool_result(stream, "insomnia_ledger").unwrap();
    assert!(value["turns"].is_object());
    assert!(value["evidence_requests"].as_array().unwrap().is_empty());
}

#[test]
fn forced_tool_result_rejects_wrong_name_or_multiple_calls() {
    let wrong = json!({
        "choices": [{"message": {"tool_calls": [{
            "function": {"name": "wrong", "arguments": "{}"}
        }]}}]
    });
    assert!(parse_forced_tool_result(&wrong, "expected").is_err());

    let multiple = json!({
        "choices": [{"message": {"tool_calls": [
            {"function": {"name": "expected", "arguments": "{}"}},
            {"function": {"name": "expected", "arguments": "{}"}}
        ]}}]
    });
    assert!(parse_forced_tool_result(&multiple, "expected").is_err());
}
