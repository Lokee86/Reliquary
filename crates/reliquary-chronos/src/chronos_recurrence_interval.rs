use crate::chronos_number::{WORD_NUMBER_RE, parse_number};
use crate::chronos_parser::{TextSpan, claimed};
use crate::chronos_relative::parse_weekday;
use crate::{TemporalFrequency, TemporalPattern};
use regex::Regex;
use std::sync::LazyLock;

static OTHER_WEEKDAY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\bevery\s+other\s+(monday|tuesday|wednesday|thursday|friday|saturday|sunday)\b",
    )
    .expect("valid Chronos alternate-weekday recurrence regex")
});
static WEEKDAY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:every|each)\s+(monday|tuesday|wednesday|thursday|friday|saturday|sunday)\b",
    )
    .expect("valid Chronos weekday recurrence regex")
});
static PLURAL_WEEKDAY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(monday|tuesday|wednesday|thursday|friday|saturday|sunday)s\b")
        .expect("valid Chronos plural-weekday recurrence regex")
});
static OTHER_UNIT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bevery\s+other\s+(day|week|month|quarter|year)\b")
        .expect("valid Chronos alternate-unit recurrence regex")
});
static INTERVAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b(?:every|each)\s+({number}|\d{{1,4}})\s+(day|week|month|quarter|year)s?\b",
        number = WORD_NUMBER_RE
    ))
    .expect("valid Chronos interval recurrence regex")
});
static UNIT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:every|each)\s+(day|week|month|quarter|year)\b")
        .expect("valid Chronos unit recurrence regex")
});

pub(crate) fn extract_interval_recurrence(
    text: &str,
    claimed_spans: &[TextSpan],
) -> Vec<TemporalPattern> {
    let mut result = Vec::new();
    for captures in OTHER_WEEKDAY_RE.captures_iter(text) {
        if is_claimed(&captures, claimed_spans) {
            continue;
        }
        if let Some(weekday) = parse_weekday(&captures[1]) {
            result.push(pattern(
                captures.get(0).unwrap().as_str(),
                TemporalFrequency::Weekly,
                2,
                Some(weekday),
            ));
        }
    }
    for captures in WEEKDAY_RE.captures_iter(text) {
        if is_claimed(&captures, claimed_spans) {
            continue;
        }
        if let Some(weekday) = parse_weekday(&captures[1]) {
            result.push(pattern(
                captures.get(0).unwrap().as_str(),
                TemporalFrequency::Weekly,
                1,
                Some(weekday),
            ));
        }
    }
    for captures in PLURAL_WEEKDAY_RE.captures_iter(text) {
        if is_claimed(&captures, claimed_spans) {
            continue;
        }
        if let Some(weekday) = parse_weekday(&captures[1]) {
            result.push(pattern(
                captures.get(0).unwrap().as_str(),
                TemporalFrequency::Weekly,
                1,
                Some(weekday),
            ));
        }
    }
    for captures in OTHER_UNIT_RE.captures_iter(text) {
        if is_claimed(&captures, claimed_spans) {
            continue;
        }
        if let Some(frequency) = unit_frequency(&captures[1]) {
            result.push(pattern(
                captures.get(0).unwrap().as_str(),
                frequency,
                2,
                None,
            ));
        }
    }
    for captures in INTERVAL_RE.captures_iter(text) {
        if is_claimed(&captures, claimed_spans) {
            continue;
        }
        let Some(amount) = parse_number(&captures[1]) else {
            continue;
        };
        let Ok(interval) = u16::try_from(amount) else {
            continue;
        };
        if interval == 0 {
            continue;
        }
        let Some(frequency) = unit_frequency(&captures[2]) else {
            continue;
        };
        result.push(pattern(
            captures.get(0).unwrap().as_str(),
            frequency,
            interval,
            None,
        ));
    }
    for captures in UNIT_RE.captures_iter(text) {
        if is_claimed(&captures, claimed_spans) {
            continue;
        }
        if let Some(frequency) = unit_frequency(&captures[1]) {
            result.push(pattern(
                captures.get(0).unwrap().as_str(),
                frequency,
                1,
                None,
            ));
        }
    }
    result
}

fn is_claimed(captures: &regex::Captures<'_>, claimed_spans: &[TextSpan]) -> bool {
    let found = captures.get(0).unwrap();
    claimed(
        TextSpan {
            start: found.start(),
            end: found.end(),
        },
        claimed_spans,
    )
}

fn pattern(
    evidence: &str,
    frequency: TemporalFrequency,
    interval: u16,
    weekday: Option<crate::TemporalWeekday>,
) -> TemporalPattern {
    TemporalPattern {
        frequency,
        interval,
        occurrences_per_period: 1,
        weekday,
        month_day: None,
        month: None,
        evidence: evidence.to_owned(),
    }
}

fn unit_frequency(value: &str) -> Option<TemporalFrequency> {
    Some(match value.to_ascii_lowercase().as_str() {
        "day" => TemporalFrequency::Daily,
        "week" => TemporalFrequency::Weekly,
        "month" => TemporalFrequency::Monthly,
        "quarter" => TemporalFrequency::Quarterly,
        "year" => TemporalFrequency::Yearly,
        _ => return None,
    })
}
