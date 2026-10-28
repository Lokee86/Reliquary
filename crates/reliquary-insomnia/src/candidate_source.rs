use super::extraction::InsomniaExtractionError;
use serde_json::Value;

#[derive(Clone, Default)]
pub(super) struct RawSource {
    pub(super) conversation_id: String,
    pub(super) node_id: String,
    pub(super) quote: String,
}

pub(super) fn parse_source(
    value: &Value,
    prefix: &str,
) -> Result<RawSource, InsomniaExtractionError> {
    Ok(RawSource {
        conversation_id: required_string(value, &format!("{prefix}_conversation_id"))?,
        node_id: required_string(value, &format!("{prefix}_node_id"))?,
        quote: required_string(value, &format!("{prefix}_quote"))?,
    })
}

pub(super) fn required_string(
    value: &Value,
    field: &str,
) -> Result<String, InsomniaExtractionError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            InsomniaExtractionError::InvalidOutput(format!("missing string field {field}"))
        })
}

pub(super) fn trim_source(source: &RawSource) -> RawSource {
    RawSource {
        conversation_id: source.conversation_id.trim().to_owned(),
        node_id: source.node_id.trim().to_owned(),
        quote: source.quote.trim().to_owned(),
    }
}

pub(super) fn source_complete(source: &RawSource) -> bool {
    let present = [
        !source.conversation_id.is_empty(),
        !source.node_id.is_empty(),
        !source.quote.is_empty(),
    ];
    present.iter().all(|value| *value) || present.iter().all(|value| !*value)
}

pub(super) fn optional(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}
