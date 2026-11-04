use super::candidate_receipt_policy::violates_execution_receipt_policy;
use super::candidate_text::{informative_tokens, normalize};

#[allow(clippy::too_many_arguments)]
pub fn validate_semantic_authority(
    authority_kind: &str,
    category: &str,
    title: &str,
    source_quote: &str,
    content: &str,
    has_authority_source: bool,
    has_grounding_source: bool,
) -> Option<String> {
    if matches!(authority_kind, "direct" | "correction") && has_authority_source {
        return Some(
            "direct/correction authority must not use assistant authority provenance".into(),
        );
    }
    if authority_kind == "adoption" && !has_authority_source {
        return Some("adoption authority requires assistant authority provenance".into());
    }
    if authority_kind == "retention"
        && retention_requires_authority_source(source_quote)
        && !has_authority_source
    {
        return Some("retention callback requires assistant authority provenance".into());
    }
    if !has_authority_source
        && !has_grounding_source
        && vague_authority_cannot_support(source_quote, content)
    {
        return Some(
            "vague authority turn cannot support detailed memory without authority or grounding provenance"
                .into(),
        );
    }
    if looks_like_unsupported_question(source_quote, category) {
        return Some("interrogative authority does not assert the extracted proposition".into());
    }
    if violates_execution_receipt_policy(title, content) {
        return Some("candidate contains execution receipt/progress framing".into());
    }
    None
}

fn retention_requires_authority_source(source_quote: &str) -> bool {
    let normalized = normalize(source_quote);
    if matches!(
        normalized.as_str(),
        "remember that" | "remember this" | "remember it" | "retain that" | "retain this"
    ) {
        return true;
    }
    if let Some(rest) = normalized
        .strip_prefix("remember that ")
        .or_else(|| normalized.strip_prefix("remember this "))
    {
        return !source_quote.contains(':') && !contains_inline_predicate(rest);
    }
    normalized.starts_with("remember the ")
        && [
            " discussed",
            " talked about",
            " mentioned",
            " earlier",
            " before",
        ]
        .iter()
        .any(|marker| normalized.contains(marker))
}

fn contains_inline_predicate(value: &str) -> bool {
    let padded = format!(" {value} ");
    [
        " am ",
        " is ",
        " are ",
        " was ",
        " were ",
        " use ",
        " uses ",
        " prefer ",
        " prefers ",
        " want ",
        " wants ",
        " need ",
        " needs ",
        " should ",
        " must ",
        " will ",
        " have ",
        " has ",
        " own ",
        " owns ",
        " run ",
        " runs ",
        " store ",
        " stores ",
        " require ",
        " requires ",
    ]
    .iter()
    .any(|marker| padded.contains(marker))
}

fn vague_authority_cannot_support(source_quote: &str, content: &str) -> bool {
    let normalized = normalize(source_quote);
    if is_acknowledgement_or_deictic_action(&normalized) {
        return informative_tokens(content) >= 3;
    }
    let has_deictic = normalized
        .split_whitespace()
        .any(|word| matches!(word, "that" | "this" | "it" | "them" | "those" | "these"));
    has_deictic
        && normalized.split_whitespace().count() <= 8
        && informative_tokens(source_quote) <= 2
        && informative_tokens(content) >= 5
}

fn is_acknowledgement_or_deictic_action(value: &str) -> bool {
    matches!(
        value,
        "sounds good"
            | "that works"
            | "this works"
            | "good rule"
            | "do that"
            | "do this"
            | "do it"
            | "add that"
            | "add this"
            | "change that"
            | "change this"
            | "finish that"
            | "finish this"
            | "finish it"
            | "get it done"
            | "get that done"
            | "alright do that then"
            | "alright that works"
            | "ok done"
            | "okay done"
    )
}

fn looks_like_unsupported_question(source_quote: &str, category: &str) -> bool {
    let trimmed = source_quote.trim();
    let Some(before_question) = trimmed.strip_suffix('?') else {
        return false;
    };
    if before_question.contains(". ") || before_question.contains("! ") {
        return false;
    }
    let normalized = normalize(trimmed);
    let lower = format!(" {normalized} ");
    if [
        " have to ",
        " has to ",
        " must ",
        " will ",
        " i prefer ",
        " i don't want ",
        " i do not want ",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return false;
    }
    if matches!(category, "fact" | "correction") && is_declarative_tag_question(&normalized) {
        return false;
    }
    true
}

fn is_declarative_tag_question(value: &str) -> bool {
    let starts_as_question = [
        "what ", "why ", "how ", "when ", "where ", "who ", "can ", "could ", "would ", "should ",
        "is ", "are ", "am ", "do ", "does ", "did ", "will ", "have ", "has ",
    ]
    .iter()
    .any(|prefix| value.starts_with(prefix));
    !starts_as_question
        && [
            " right",
            " correct",
            " is it",
            " isn't it",
            " are they",
            " aren't they",
            " don't they",
            " doesn't it",
            " yeah",
            " no",
        ]
        .iter()
        .any(|suffix| value.ends_with(suffix))
}
