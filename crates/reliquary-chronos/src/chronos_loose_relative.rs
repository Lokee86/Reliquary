use crate::chronos_absolute_calendar::parse_month;
use crate::chronos_calendar::{date_from_ns, day_span, month_number, month_span};
use crate::chronos_loose_calendar::MONTH_RE;
use crate::chronos_parser::{TextSpan, claimed, push_claimed};
use crate::{TemporalAnchor, TemporalGranularity, TemporalOrigin};
use regex::Regex;
use std::sync::LazyLock;
use time::{Date, Month};

static RELATIVE_MONTH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?i)\b(this|next|last)\s+({MONTH_RE})\b"))
        .expect("valid Chronos relative named-month regex")
});
static MONTH_DAY_RELATIVE_YEAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b({MONTH_RE})\s+(\d{{1,2}})(?:st|nd|rd|th)?(?:,)?\s+(this|next|last)\s+year\b"
    ))
    .expect("valid Chronos month-day relative-year regex")
});

pub(crate) fn extract(
    text: &str,
    reference_timestamp_ns: Option<i64>,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    let Some(reference) = reference_timestamp_ns.and_then(date_from_ns) else {
        return;
    };
    extract_relative_months(text, reference, claimed_spans, anchors);
    extract_relative_year_dates(text, reference, claimed_spans, anchors);
}

fn extract_relative_months(
    text: &str,
    reference: Date,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    for captures in RELATIVE_MONTH_RE.captures_iter(text) {
        let found = captures.get(0).unwrap();
        let span = span(found);
        if claimed(span, claimed_spans) {
            continue;
        }
        let Some(month) = parse_month(&captures[2]) else {
            continue;
        };
        let year = relative_month_year(reference, month, &captures[1]);
        let Ok(date) = Date::from_calendar_date(year, month, 1) else {
            continue;
        };
        let Some((start_ns, end_ns)) = month_span(date) else {
            continue;
        };
        anchors.push(relative_anchor(
            found.as_str(),
            start_ns,
            end_ns,
            TemporalGranularity::Month,
        ));
        push_claimed(claimed_spans, span);
    }
}

fn extract_relative_year_dates(
    text: &str,
    reference: Date,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    for captures in MONTH_DAY_RELATIVE_YEAR_RE.captures_iter(text) {
        let offset = match captures[3].to_ascii_lowercase().as_str() {
            "this" => 0,
            "next" => 1,
            "last" => -1,
            _ => continue,
        };
        let Some(year) = reference.year().checked_add(offset) else {
            continue;
        };
        push_day(
            captures.get(0).unwrap(),
            parse_month(&captures[1]),
            captures[2].parse().ok(),
            year,
            claimed_spans,
            anchors,
        );
    }
}

fn push_day(
    found: regex::Match<'_>,
    month: Option<Month>,
    day: Option<u8>,
    year: i32,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    let span = span(found);
    if claimed(span, claimed_spans) {
        return;
    }
    let (Some(month), Some(day)) = (month, day) else {
        return;
    };
    let Ok(date) = Date::from_calendar_date(year, month, day) else {
        return;
    };
    let Some((start_ns, end_ns)) = day_span(date) else {
        return;
    };
    anchors.push(relative_anchor(
        found.as_str(),
        start_ns,
        end_ns,
        TemporalGranularity::Day,
    ));
    push_claimed(claimed_spans, span);
}

fn relative_month_year(reference: Date, month: Month, relation: &str) -> i32 {
    let current = month_number(reference.month());
    let target = month_number(month);
    match relation.to_ascii_lowercase().as_str() {
        "this" => reference.year(),
        "next" if target > current => reference.year(),
        "next" => reference.year().saturating_add(1),
        "last" if target < current => reference.year(),
        "last" => reference.year().saturating_sub(1),
        _ => reference.year(),
    }
}

fn relative_anchor(
    evidence: &str,
    start_ns: i64,
    end_ns: i64,
    granularity: TemporalGranularity,
) -> TemporalAnchor {
    TemporalAnchor {
        start_ns,
        end_ns,
        granularity,
        origin: TemporalOrigin::Relative,
        evidence: evidence.to_owned(),
    }
}

fn span(found: regex::Match<'_>) -> TextSpan {
    TextSpan {
        start: found.start(),
        end: found.end(),
    }
}
