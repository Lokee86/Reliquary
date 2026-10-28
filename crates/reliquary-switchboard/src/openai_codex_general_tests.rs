use crate::ModelReasoningEffort;
use crate::openai_codex_general::{build_request_body, parse_sse_json};
use serde_json::json;

#[test]
fn luna_low_request_uses_responses_lite_shape_and_strict_schema() {
    let schema = json!({
        "type": "object",
        "properties": {"answer": {"type": "string"}},
        "required": ["answer"],
        "additionalProperties": false
    });
    let body = build_request_body(
        "gpt-5.6-luna",
        ModelReasoningEffort::Low,
        "system",
        "user",
        "continuity_probe",
        &schema,
    );

    assert_eq!(body["model"], "gpt-5.6-luna");
    assert_eq!(body["reasoning"]["effort"], "low");
    assert_eq!(body["reasoning"]["summary"], "auto");
    assert_eq!(body["reasoning"]["context"], "all_turns");
    assert_eq!(body["service_tier"], "priority");
    assert_eq!(body["stream"], true);
    assert_eq!(body["store"], false);
    assert_eq!(body["text"]["verbosity"], "low");
    assert_eq!(body["text"]["format"]["type"], "json_schema");
    assert_eq!(body["text"]["format"]["name"], "continuity_probe");
    assert_eq!(body["text"]["format"]["strict"], true);
    assert_eq!(body["text"]["format"]["schema"], schema);
    assert_eq!(body["instructions"], "system");
    assert_eq!(body["input"][0]["content"][0]["text"], "user");
}

#[test]
fn codex_sse_parser_reassembles_structured_output() {
    let stream = concat!(
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"answer\\\":\"}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"\\\"ok\\\"}\"}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r1\"}}\n\n",
        "data: [DONE]\n\n"
    );
    assert_eq!(parse_sse_json(stream).unwrap(), json!({"answer": "ok"}));
}

#[test]
fn codex_sse_parser_rejects_failed_or_incomplete_streams() {
    let failed = concat!("data: {\"type\":\"response.failed\",\"response\":{\"id\":\"r1\"}}\n\n");
    assert!(parse_sse_json(failed).is_err());

    let missing_completion =
        concat!("data: {\"type\":\"response.output_text.delta\",\"delta\":\"{}\"}\n\n");
    assert!(parse_sse_json(missing_completion).is_err());
}
