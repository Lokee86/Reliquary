use crate::chronos_parser::{TextSpan, claimed, push_claimed};
use crate::chronos_recurrence_interval::extract_interval_recurrence;
use crate::{TemporalFrequency, TemporalPattern};
use regex::Regex;
use std::sync::LazyLock;

static CADENCE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(daily|weekly|monthly|quarterly|yearly|annually)\b").unwrap()
});
static TWICE_WEEKLY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:twice\s+weekly|semiweekly)\b").unwrap());
static TWICE_MONTHLY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:twice\s+monthly|semimonthly)\b").unwrap());
static YEARLY_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:every year on|annually on|yearly on)\s+(january|february|march|april|may|june|july|august|september|october|november|december)\s+(\d{1,2})(?:st|nd|rd|th)?\b")
        .unwrap()
});
static MONTHLY_DAY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:every month on|monthly on)\s+(?:the\s+)?(\d{1,2})(?:st|nd|rd|th)?\b")
        .unwrap()
});

pub(crate) fn extract_recurrence(text: &str) -> Vec<TemporalPattern> {
    let mut result = Vec::new();
    let mut claimed_spans = Vec::new();
    for found in TWICE_MONTHLY_RE.find_iter(text) {
        let mut value = pattern(found.as_str(), TemporalFrequency::Monthly, None);
        value.occurrences_per_period = 2;
        result.push(value);
        push_claimed(
            &mut claimed_spans,
            TextSpan {
                start: found.start(),
                end: found.end(),
            },
        );
    }
    for found in TWICE_WEEKLY_RE.find_iter(text) {
        let mut value = pattern(found.as_str(), TemporalFrequency::Weekly, None);
        value.occurrences_per_period = 2;
        result.push(value);
        push_claimed(
            &mut claimed_spans,
            TextSpan {
                start: found.start(),
                end: found.end(),
            },
        );
    }
    for captures in YEARLY_DATE_RE.captures_iter(text) {
        let found = captures.get(0).unwrap();
        let Some(month) = month_number(&captures[1]) else {
            continue;
        };
        let Ok(day) = captures[2].parse::<u8>() else {
            continue;
        };
        if !valid_month_day(month, day) {
            continue;
        }
        result.push(TemporalPattern {
            frequency: TemporalFrequency::Yearly,
            interval: 1,
            occurrences_per_period: 1,
            weekday: None,
            month_day: Some(day),
            month: Some(month),
            evidence: found.as_str().to_owned(),
        });
        push_claimed(
            &mut claimed_spans,
            TextSpan {
                start: found.start(),
                end: found.end(),
            },
        );
    }
    for captures in MONTHLY_DAY_RE.captures_iter(text) {
        let Ok(day) = captures[1].parse::<u8>() else {
            continue;
        };
        if !(1..=31).contains(&day) {
            continue;
        }
        let found = captures.get(0).unwrap();
        result.push(pattern(
            found.as_str(),
            TemporalFrequency::Monthly,
            Some(day),
        ));
        push_claimed(
            &mut claimed_spans,
            TextSpan {
                start: found.start(),
                end: found.end(),
            },
        );
    }
    result.extend(extract_interval_recurrence(text, &claimed_spans));
    for captures in CADENCE_RE.captures_iter(text) {
        let found = captures.get(0).unwrap();
        let span = TextSpan {
            start: found.start(),
            end: found.end(),
        };
        if claimed(span, &claimed_spans) {
            continue;
        }
        let Some(frequency) = cadence_frequency(&captures[1]) else {
            continue;
        };
        result.push(pattern(found.as_str(), frequency, None));
    }
    result
}

fn pattern(evidence: &str, frequency: TemporalFrequency, month_day: Option<u8>) -> TemporalPattern {
    TemporalPattern {
        frequency,
        interval: 1,
        occurrences_per_period: 1,
        weekday: None,
        month_day,
        month: None,
        evidence: evidence.to_owned(),
    }
}

fn month_number(value: &str) -> Option<u8> {
    Some(match value.to_ascii_lowercase().as_str() {
        "january" => 1,
        "february" => 2,
        "march" => 3,
        "april" => 4,
        "may" => 5,
        "june" => 6,
        "july" => 7,
        "august" => 8,
        "september" => 9,
        "october" => 10,
        "november" => 11,
        "december" => 12,
        _ => return None,
    })
}

fn valid_month_day(month: u8, day: u8) -> bool {
    let max = match month {
        2 => 29,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return false,
    };
    (1..=max).contains(&day)
}

fn cadence_frequency(value: &str) -> Option<TemporalFrequency> {
    match value.to_ascii_lowercase().as_str() {
        "daily" => Some(TemporalFrequency::Daily),
        "weekly" => Some(TemporalFrequency::Weekly),
        "monthly" => Some(TemporalFrequency::Monthly),
        "quarterly" => Some(TemporalFrequency::Quarterly),
        "yearly" | "annually" => Some(TemporalFrequency::Yearly),
        _ => None,
    }
}
