use crate::{TemporalAnalysis, TemporalDetection, TemporalIndication};
use regex::Regex;
use std::sync::LazyLock;

/// The result of checking one exact ambiguous source indication for a local definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ExplicitDefinitionOutcome {
    Proven {
        canonical_expression: String,
        phrase_span: (usize, usize),
    },
    Blocked,
    None,
}

static DEFINITION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(biweekly|bimonthly)\s*(?:,\s*(?:meaning|i\.e\.|that is|not|maybe|perhaps|possibly|might|if|or)\s+|=\s*|\(\s*)([^.;\n()]+)(\))?")
        .expect("valid explicit definition regex")
});

/// Resolve a definition only when its matched source term is this indication and the
/// captured phrase verifies as exactly one of that term's supported meanings.
pub(crate) fn explicit_definition_outcome(
    text: &str,
    indication: &TemporalIndication,
    reference_timestamp_ns: Option<i64>,
) -> ExplicitDefinitionOutcome {
    if indication.kind != crate::TemporalIndicationKind::Recurrence
        || text.get(indication.start_byte..indication.end_byte)
            != Some(indication.evidence.as_str())
        || !matches!(
            indication.evidence.to_ascii_lowercase().as_str(),
            "biweekly" | "bimonthly"
        )
    {
        return ExplicitDefinitionOutcome::None;
    }
    let alternatives = if indication.evidence.eq_ignore_ascii_case("biweekly") {
        ["every two weeks", "twice weekly"]
    } else {
        ["every two months", "twice monthly"]
    };
    let mut local_end = indication.end_byte.saturating_add(512).min(text.len());
    while !text.is_char_boundary(local_end) {
        local_end -= 1;
    }
    let Some(local) = text.get(indication.start_byte..local_end) else {
        return ExplicitDefinitionOutcome::None;
    };
    let Some(captures) = DEFINITION_RE.captures(local) else {
        return ExplicitDefinitionOutcome::None;
    };
    let term = captures.get(1).unwrap();
    if term.end() != indication.end_byte - indication.start_byte {
        return ExplicitDefinitionOutcome::None;
    }
    let phrase = captures.get(2).unwrap();
    let whole = captures.get(0).unwrap();
    let tail = &local[phrase.end()..whole.end()];
    let connector = &local[term.end()..phrase.start()];
    let parenthetical = connector.contains('(');
    if connector.contains('\n') || connector.contains('\r') {
        return ExplicitDefinitionOutcome::None;
    }
    if parenthetical && !tail.starts_with(')') {
        return ExplicitDefinitionOutcome::Blocked;
    }
    // An exact definition is not authoritative if its same-clause continuation
    // qualifies it or states another cadence. This applies to comma definitions
    // as well as parentheses; checking a few leading words is not sufficient.
    if unsafe_definition_continuation(&local[whole.end()..])
        || (local_end < text.len() && !local[whole.end()..].contains(['.', ';', '\n', '\r']))
    {
        return ExplicitDefinitionOutcome::Blocked;
    }
    let lower = local[term.end()..phrase.end()].to_ascii_lowercase();
    let mut prefix_start = indication.start_byte.saturating_sub(128);
    while !text.is_char_boundary(prefix_start) {
        prefix_start += 1;
    }
    let before_term = &text[prefix_start..indication.start_byte];
    let clause_prefix = before_term
        .rsplit(['.', ';', '\n', '\r'])
        .next()
        .unwrap_or(before_term)
        .to_ascii_lowercase();
    if lower.starts_with("not ")
        || lower.contains(" not ")
        || lower.contains("never")
        || lower.contains(" maybe ")
        || lower.contains(" perhaps ")
        || lower.contains(" might ")
        || lower.contains(" or ")
        || lower.contains(" if ")
        || clause_prefix.split_whitespace().rev().take(3).any(|word| {
            matches!(
                word,
                "not" | "maybe" | "perhaps" | "might" | "possibly" | "if" | "hypothetically"
            )
        })
    {
        return ExplicitDefinitionOutcome::Blocked;
    }
    let phrase_text = phrase.as_str().trim();
    if crate::detect(phrase_text).indications.iter().any(|value| {
        matches!(
            value.evidence.to_ascii_lowercase().as_str(),
            "biweekly" | "bimonthly"
        )
    }) {
        return ExplicitDefinitionOutcome::Blocked;
    }
    let mut meaning = None;
    for candidate in alternatives {
        if let Ok(analysis) = crate::chronos_inference_verify::verify_canonical_expression(
            phrase_text,
            crate::TemporalIndicationKind::Recurrence,
            reference_timestamp_ns,
        ) {
            if let Ok(expected) = crate::chronos_inference_verify::verify_canonical_expression(
                candidate,
                crate::TemporalIndicationKind::Recurrence,
                reference_timestamp_ns,
            ) {
                if analysis
                    .patterns
                    .first()
                    .is_some_and(|pattern| pattern.evidence.eq_ignore_ascii_case(phrase_text))
                    && same_recurrence(&analysis, &expected)
                {
                    if meaning.is_some() {
                        return ExplicitDefinitionOutcome::Blocked;
                    }
                    meaning = Some(candidate.to_owned());
                }
            }
        }
    }
    match meaning {
        Some(canonical_expression) => ExplicitDefinitionOutcome::Proven {
            canonical_expression,
            phrase_span: (
                indication.start_byte + phrase.start(),
                indication.start_byte + phrase.end(),
            ),
        },
        None => ExplicitDefinitionOutcome::Blocked,
    }
}

static QUALIFIED_CONTINUATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:or|unless|except|otherwise|instead|rather|but|however|actually|alternatively|maybe|perhaps|possibly|might|if|hypothetically|not|never|meaning|known\s+as|that\s+is)\b",
    )
    .expect("valid definition qualification regex")
});

fn unsafe_definition_continuation(remainder: &str) -> bool {
    let same_clause = remainder
        .split([';', '\n', '\r', '.'])
        .next()
        .unwrap_or("")
        .trim();
    if same_clause.is_empty() {
        return false;
    }
    // A second parenthetical can redefine the same source term. Do not
    // guess that it is unrelated when no span-grounded grammar proves that.
    same_clause.contains(['(', ')'])
        || QUALIFIED_CONTINUATION.is_match(same_clause)
        || !crate::chronos_parser::parse_temporal(same_clause, None)
            .patterns
            .is_empty()
}

/// Prevent an unproven contradictory definition from being published through
/// `analyze` (which Dream uses without inspecting `assess` status). Mask only
/// the affected source clause, preserving UTF-8 byte offsets and unrelated
/// clauses; leave original text and detection untouched for resolution.
pub(crate) fn mask_blocked_definition_clauses(
    text: &str,
    reference_timestamp_ns: Option<i64>,
    detection: &TemporalDetection,
) -> Option<String> {
    let mut starts = Vec::new();
    for indication in &detection.indications {
        if explicit_definition_outcome(text, indication, reference_timestamp_ns)
            == ExplicitDefinitionOutcome::Blocked
        {
            starts.push(indication.start_byte);
        }
    }
    if starts.is_empty() {
        return None;
    }

    let mut masked = text.as_bytes().to_vec();
    for start in starts {
        let mut end = text.len();
        for (offset, ch) in text[start..].char_indices() {
            let pos = start + offset;
            // Bound masking to the same local window as definition recognition.
            if offset >= 512 {
                end = pos;
                break;
            }
            if matches!(ch, ';' | '\n' | '\r') || (ch == '.' && !abbreviation_dot(text, pos)) {
                end = pos;
                break;
            }
        }
        masked[start..end].fill(b' ');
    }
    Some(String::from_utf8(masked).expect("replacing complete UTF-8 character spans with ASCII"))
}

fn abbreviation_dot(text: &str, dot: usize) -> bool {
    text.get(dot.saturating_sub(1)..dot.saturating_add(3))
        .is_some_and(|v| v.eq_ignore_ascii_case("i.e."))
        || text
            .get(dot.saturating_sub(3)..dot.saturating_add(1))
            .is_some_and(|v| v.eq_ignore_ascii_case("i.e."))
}

fn same_recurrence(left: &TemporalAnalysis, right: &TemporalAnalysis) -> bool {
    has_only_one_recurrence(left) && has_only_one_recurrence(right) && {
        let a = &left.patterns[0];
        let b = &right.patterns[0];
        a.frequency == b.frequency
            && a.interval == b.interval
            && a.occurrences_per_period == b.occurrences_per_period
            && a.weekday == b.weekday
            && a.month_day == b.month_day
            && a.month == b.month
    }
}

fn has_only_one_recurrence(analysis: &TemporalAnalysis) -> bool {
    analysis.patterns.len() == 1
        && analysis.anchors.is_empty()
        && analysis.times_of_day.is_empty()
        && analysis.durations.is_empty()
        && analysis.duration_ranges.is_empty()
        && analysis.approximate_durations.is_empty()
        && analysis.event_relations.is_empty()
        && analysis.intervals.is_empty()
}

/// Merge deterministic, source-backed definitions into the shared analysis path.
pub(crate) fn apply_explicit_definitions(
    text: &str,
    reference_timestamp_ns: Option<i64>,
    detection: &TemporalDetection,
    analysis: &mut TemporalAnalysis,
) {
    for indication in &detection.indications {
        let ExplicitDefinitionOutcome::Proven {
            canonical_expression,
            ..
        } = explicit_definition_outcome(text, indication, reference_timestamp_ns)
        else {
            continue;
        };
        let Ok(mut inferred) = crate::chronos_inference_verify::verify_canonical_expression(
            &canonical_expression,
            indication.kind,
            reference_timestamp_ns,
        ) else {
            continue;
        };
        for pattern in &mut inferred.patterns {
            pattern.evidence = indication.evidence.clone();
        }
        crate::chronos_parser::merge_temporal_analysis(analysis, inferred);
    }
}
