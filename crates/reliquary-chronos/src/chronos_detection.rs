use crate::chronos_detection_explicit::explicit_indications;
use crate::chronos_fuzzy::fuzzy_word;
use crate::chronos_vocabulary::exact_kind;
use crate::{TemporalDetection, TemporalIndication, TemporalIndicationKind};
use regex::Regex;
use std::sync::LazyLock;

static TOKEN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z]+|\d+").expect("valid Chronos token regex"));
#[derive(Clone, Debug)]
pub(crate) struct DetectedTemporalText {
    pub(crate) detection: TemporalDetection,
    pub(crate) normalized_text: Option<String>,
    corrections: Vec<AppliedCorrection>,
}

#[derive(Clone, Debug)]
struct AppliedCorrection {
    original_start: usize,
    original_end: usize,
    normalized_start: usize,
    normalized_end: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Token<'a> {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) value: &'a str,
    pub(crate) lower: String,
}

pub(crate) fn detect_and_normalize(text: &str) -> DetectedTemporalText {
    let tokens = tokens(text);
    let mut indications = explicit_indications(text, &tokens);
    let mut replacements = Vec::new();

    for (index, token) in tokens.iter().enumerate() {
        let previous = index.checked_sub(1).and_then(|value| tokens.get(value));
        let next = tokens.get(index + 1);
        let capitalized = token
            .value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_uppercase());
        if let Some(kind) = exact_kind(
            token.value,
            previous.map(|value| value.lower.as_str()),
            next.map(|value| value.lower.as_str()),
            capitalized,
        ) {
            indications.push(indication(text, token.start, token.end, kind, None));
            continue;
        }
        let previous_adjacent = previous.is_some_and(|value| {
            text[value.end..token.start]
                .chars()
                .all(|character| character.is_ascii_whitespace() || character == '-')
        });
        let Some((normalized, kind)) = fuzzy_word(
            token.value,
            previous.map(|value| value.lower.as_str()),
            next.map(|value| value.lower.as_str()),
            previous_adjacent,
            capitalized,
        ) else {
            continue;
        };
        indications.push(indication(
            text,
            token.start,
            token.end,
            kind,
            Some(normalized.to_owned()),
        ));
        replacements.push((token.start, token.end, normalized));
    }

    indications.sort_by_key(|value| (value.start_byte, value.end_byte, value.kind));
    indications.dedup_by(|left, right| {
        left.start_byte == right.start_byte
            && left.end_byte == right.end_byte
            && left.kind == right.kind
            && left.normalized == right.normalized
    });
    let (normalized_text, corrections) = apply_replacements(text, &replacements);
    DetectedTemporalText {
        detection: TemporalDetection { indications },
        normalized_text,
        corrections,
    }
}

impl DetectedTemporalText {
    pub(crate) fn restore_evidence(&self, original: &str, evidence: &str) -> String {
        let Some(normalized_text) = self.normalized_text.as_deref() else {
            return evidence.to_owned();
        };
        for (start, _) in normalized_text.match_indices(evidence) {
            let end = start + evidence.len();
            if !self
                .corrections
                .iter()
                .any(|value| start < value.normalized_end && value.normalized_start < end)
            {
                continue;
            }
            let original_start = self.original_boundary(start);
            let original_end = self.original_boundary(end);
            return original[original_start..original_end].to_owned();
        }
        evidence.to_owned()
    }

    fn original_boundary(&self, normalized_offset: usize) -> usize {
        let mut delta: isize = 0;
        for correction in &self.corrections {
            if normalized_offset < correction.normalized_start {
                break;
            }
            if normalized_offset == correction.normalized_start {
                return correction.original_start;
            }
            if normalized_offset <= correction.normalized_end {
                return correction.original_end;
            }
            delta += (correction.original_end - correction.original_start) as isize
                - (correction.normalized_end - correction.normalized_start) as isize;
        }
        normalized_offset.saturating_add_signed(delta)
    }
}

fn tokens(text: &str) -> Vec<Token<'_>> {
    TOKEN_RE
        .find_iter(text)
        .map(|found| Token {
            start: found.start(),
            end: found.end(),
            value: found.as_str(),
            lower: found.as_str().to_ascii_lowercase(),
        })
        .collect()
}

fn indication(
    text: &str,
    start: usize,
    end: usize,
    kind: TemporalIndicationKind,
    normalized: Option<String>,
) -> TemporalIndication {
    TemporalIndication {
        start_byte: start,
        end_byte: end,
        kind,
        evidence: text[start..end].to_owned(),
        normalized,
    }
}

fn apply_replacements(
    text: &str,
    replacements: &[(usize, usize, &'static str)],
) -> (Option<String>, Vec<AppliedCorrection>) {
    if replacements.is_empty() {
        return (None, Vec::new());
    }
    let mut result = String::with_capacity(text.len());
    let mut corrections = Vec::new();
    let mut cursor = 0;
    for &(start, end, replacement) in replacements {
        result.push_str(&text[cursor..start]);
        let normalized_start = result.len();
        result.push_str(replacement);
        corrections.push(AppliedCorrection {
            original_start: start,
            original_end: end,
            normalized_start,
            normalized_end: result.len(),
        });
        cursor = end;
    }
    result.push_str(&text[cursor..]);
    (Some(result), corrections)
}
