use crate::chronos_absolute::explicit_anchor;
use crate::chronos_calendar::{day_span, month_span, quarter_span, year_span};
use crate::chronos_parser::{TextSpan, claimed, push_claimed};
use crate::{TemporalAnchor, TemporalGranularity};
use regex::Regex;
use std::sync::LazyLock;
use time::{Date, Month};

static MONTH_DAY_YEAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(january|february|march|april|may|june|july|august|september|october|november|december)\s+(\d{1,2})(?:st|nd|rd|th)?(?:,)?\s+(\d{4})\b").unwrap()
});
static DAY_MONTH_YEAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(\d{1,2})(?:st|nd|rd|th)?\s+(january|february|march|april|may|june|july|august|september|october|november|december)\s+(\d{4})\b").unwrap()
});
static QUARTER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:q([1-4])\s+(\d{4})|(\d{4})\s+q([1-4]))\b").unwrap());
static MONTH_YEAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(january|february|march|april|may|june|july|august|september|october|november|december)\s+(\d{4})\b").unwrap()
});
static ISO_MONTH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d{4})-(\d{2})\b").unwrap());
static CONTEXT_YEAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:in|during|throughout|year)\s+(\d{4})\b").unwrap());

pub(crate) fn extract_natural_dates(
    text: &str,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    for captures in MONTH_DAY_YEAR_RE.captures_iter(text) {
        push_natural_date(
            captures.get(0).unwrap(),
            &captures[1],
            &captures[2],
            &captures[3],
            claimed_spans,
            anchors,
        );
    }
    for captures in DAY_MONTH_YEAR_RE.captures_iter(text) {
        push_natural_date(
            captures.get(0).unwrap(),
            &captures[2],
            &captures[1],
            &captures[3],
            claimed_spans,
            anchors,
        );
    }
}

pub(crate) fn extract_coarse(
    text: &str,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    extract_quarters(text, claimed_spans, anchors);
    extract_months(text, claimed_spans, anchors);
    extract_years(text, claimed_spans, anchors);
}

fn push_natural_date(
    found: regex::Match<'_>,
    month: &str,
    day: &str,
    year: &str,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    let span = TextSpan {
        start: found.start(),
        end: found.end(),
    };
    if claimed(span, claimed_spans) {
        return;
    }
    let (Some(month), Ok(day), Ok(year)) =
        (parse_month(month), day.parse::<u8>(), year.parse::<i32>())
    else {
        return;
    };
    let Ok(date) = Date::from_calendar_date(year, month, day) else {
        return;
    };
    let Some((start_ns, end_ns)) = day_span(date) else {
        return;
    };
    anchors.push(explicit_anchor(
        found.as_str(),
        start_ns,
        end_ns,
        TemporalGranularity::Day,
    ));
    push_claimed(claimed_spans, span);
}

fn extract_quarters(
    text: &str,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    for captures in QUARTER_RE.captures_iter(text) {
        let found = captures.get(0).unwrap();
        let span = TextSpan {
            start: found.start(),
            end: found.end(),
        };
        if claimed(span, claimed_spans) {
            continue;
        }
        let quarter = captures
            .get(1)
            .or_else(|| captures.get(4))
            .and_then(|value| value.as_str().parse::<u8>().ok());
        let year = captures
            .get(2)
            .or_else(|| captures.get(3))
            .and_then(|value| value.as_str().parse::<i32>().ok());
        let (Some(quarter), Some(year)) = (quarter, year) else {
            continue;
        };
        let Ok(month) = Month::try_from((quarter - 1) * 3 + 1) else {
            continue;
        };
        let Ok(date) = Date::from_calendar_date(year, month, 1) else {
            continue;
        };
        let Some((start_ns, end_ns)) = quarter_span(date) else {
            continue;
        };
        anchors.push(explicit_anchor(
            found.as_str(),
            start_ns,
            end_ns,
            TemporalGranularity::Quarter,
        ));
        push_claimed(claimed_spans, span);
    }
}

fn extract_months(
    text: &str,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    for captures in MONTH_YEAR_RE.captures_iter(text) {
        push_month(
            captures.get(0).unwrap(),
            parse_month(&captures[1]),
            captures[2].parse().ok(),
            claimed_spans,
            anchors,
        );
    }
    for captures in ISO_MONTH_RE.captures_iter(text) {
        let month = captures[2]
            .parse::<u8>()
            .ok()
            .and_then(|value| Month::try_from(value).ok());
        push_month(
            captures.get(0).unwrap(),
            month,
            captures[1].parse().ok(),
            claimed_spans,
            anchors,
        );
    }
}

fn push_month(
    found: regex::Match<'_>,
    month: Option<Month>,
    year: Option<i32>,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    let span = TextSpan {
        start: found.start(),
        end: found.end(),
    };
    if claimed(span, claimed_spans) {
        return;
    }
    let (Some(month), Some(year)) = (month, year) else {
        return;
    };
    let Ok(date) = Date::from_calendar_date(year, month, 1) else {
        return;
    };
    let Some((start_ns, end_ns)) = month_span(date) else {
        return;
    };
    anchors.push(explicit_anchor(
        found.as_str(),
        start_ns,
        end_ns,
        TemporalGranularity::Month,
    ));
    push_claimed(claimed_spans, span);
}

fn extract_years(text: &str, claimed_spans: &mut Vec<TextSpan>, anchors: &mut Vec<TemporalAnchor>) {
    for captures in CONTEXT_YEAR_RE.captures_iter(text) {
        let found = captures.get(0).unwrap();
        let span = TextSpan {
            start: found.start(),
            end: found.end(),
        };
        if claimed(span, claimed_spans) {
            continue;
        }
        let Ok(year) = captures[1].parse::<i32>() else {
            continue;
        };
        let Ok(date) = Date::from_calendar_date(year, Month::January, 1) else {
            continue;
        };
        let Some((start_ns, end_ns)) = year_span(date) else {
            continue;
        };
        anchors.push(explicit_anchor(
            found.as_str(),
            start_ns,
            end_ns,
            TemporalGranularity::Year,
        ));
        push_claimed(claimed_spans, span);
    }
}

pub(crate) fn parse_month(value: &str) -> Option<Month> {
    match value.to_ascii_lowercase().as_str() {
        "january" => Some(Month::January),
        "february" => Some(Month::February),
        "march" => Some(Month::March),
        "april" => Some(Month::April),
        "may" => Some(Month::May),
        "june" => Some(Month::June),
        "july" => Some(Month::July),
        "august" => Some(Month::August),
        "september" => Some(Month::September),
        "october" => Some(Month::October),
        "november" => Some(Month::November),
        "december" => Some(Month::December),
        _ => None,
    }
}
