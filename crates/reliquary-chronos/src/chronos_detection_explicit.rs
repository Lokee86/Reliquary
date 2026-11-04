use crate::chronos_detection::Token;
use crate::chronos_detection_context::standalone_year_context;
use crate::{TemporalIndication, TemporalIndicationKind};
use regex::Regex;
use std::sync::LazyLock;

static EXPLICIT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:\d{4}(?:-\d{1,2}(?:-\d{1,2})?)?|\d{1,2}:\d{2}(?::\d{2})?)\b")
        .expect("valid Chronos explicit regex")
});

pub(crate) fn explicit_indications(text: &str, tokens: &[Token<'_>]) -> Vec<TemporalIndication> {
    EXPLICIT_RE
        .find_iter(text)
        .filter(|found| {
            let value = found.as_str();
            if value.contains('-') || value.contains(':') {
                return true;
            }
            let Some(index) = tokens
                .iter()
                .position(|token| token.start == found.start() && token.end == found.end())
            else {
                return false;
            };
            standalone_year_context(
                value,
                index
                    .checked_sub(1)
                    .and_then(|value| tokens.get(value))
                    .map(|token| token.lower.as_str()),
                tokens.get(index + 1).map(|token| token.lower.as_str()),
            )
        })
        .map(|found| TemporalIndication {
            start_byte: found.start(),
            end_byte: found.end(),
            kind: TemporalIndicationKind::Explicit,
            evidence: text[found.start()..found.end()].to_owned(),
            normalized: None,
        })
        .collect()
}
