use serde_json::Value;

pub const MAX_INSOMNIA_EVIDENCE_REQUESTS: usize = 4;
pub const MAX_INSOMNIA_EVIDENCE_TURNS: usize = 64;
pub const MAX_INSOMNIA_EVIDENCE_BYTES: usize = 128 << 10;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceRequest {
    pub kind: String,
    pub conversation_id: String,
    pub node_id: String,
    pub start_node_id: String,
    pub end_node_id: String,
    pub query: String,
    pub limit: usize,
}

pub fn parse_evidence_requests(value: &Value) -> Result<Vec<EvidenceRequest>, String> {
    let Some(requests) = value.get("evidence_requests") else {
        return Ok(Vec::new());
    };
    let requests = requests
        .as_array()
        .ok_or_else(|| "evidence_requests must be an array".to_owned())?;
    if requests.len() > MAX_INSOMNIA_EVIDENCE_REQUESTS {
        return Err(format!(
            "{} evidence requests exceeds maximum {}",
            requests.len(),
            MAX_INSOMNIA_EVIDENCE_REQUESTS
        ));
    }
    requests.iter().map(parse_request).collect()
}

fn parse_request(value: &Value) -> Result<EvidenceRequest, String> {
    Ok(EvidenceRequest {
        kind: string_field(value, "kind")?.trim().to_lowercase(),
        conversation_id: optional_string(value, "conversation_id")?,
        node_id: optional_string(value, "node_id")?,
        start_node_id: optional_string(value, "start_node_id")?,
        end_node_id: optional_string(value, "end_node_id")?,
        query: optional_string(value, "query")?,
        limit: optional_usize(value, "limit")?,
    })
}

fn string_field(value: &Value, field: &str) -> Result<String, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("missing string field {field}"))
}

fn optional_string(value: &Value, field: &str) -> Result<String, String> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(value) => value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{field} must be a string")),
    }
}

fn optional_usize(value: &Value, field: &str) -> Result<usize, String> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(0),
        Some(value) => value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| format!("{field} must be a non-negative integer")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_bounded_requests_and_normalizes_kind() {
        let requests = parse_evidence_requests(&json!({"evidence_requests": [{
            "kind":" TURN ", "conversation_id":"c", "node_id":"n"
        }]}))
        .unwrap();
        assert_eq!(requests[0].kind, "turn");
        assert_eq!(requests[0].conversation_id, "c");
        assert_eq!(requests[0].limit, 0);
    }

    #[test]
    fn rejects_too_many_or_malformed_requests() {
        assert!(
            parse_evidence_requests(&json!({"evidence_requests": [
                {}, {}, {}, {}, {}
            ]}))
            .is_err()
        );
        assert!(parse_evidence_requests(&json!({"evidence_requests": [{"kind": 3}]})).is_err());
    }
}
