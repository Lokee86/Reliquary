use crate::chronos_calendar::{
    date_from_ns, day_span, month_span, quarter_span, shift_days, shift_month_start,
    shift_week_start, shift_year_start, week_span, weekday_date, year_span,
};
use crate::chronos_parser::{TextSpan, claimed, push_claimed};
use crate::chronos_relative_offset::extract_relative_offsets;
use crate::{TemporalAnchor, TemporalGranularity, TemporalOrigin, TemporalWeekday};
use regex::Regex;
use std::sync::LazyLock;

static WEEKDAY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(next|last)\s+(monday|tuesday|wednesday|thursday|friday|saturday|sunday)\b")
        .unwrap()
});
static PERIOD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(this|next|last)\s+(week|month|quarter|year)\b").unwrap());
static SIMPLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(today|yesterday|tomorrow)\b").unwrap());

pub(crate) fn extract_relative(
    text: &str,
    source_timestamp_ns: Option<i64>,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    let Some(source_date) = source_timestamp_ns.and_then(date_from_ns) else {
        return;
    };
    for captures in WEEKDAY_RE.captures_iter(text) {
        let found = captures.get(0).unwrap();
        let span = TextSpan {
            start: found.start(),
            end: found.end(),
        };
        if claimed(span, claimed_spans) {
            continue;
        }
        let Some(weekday) = parse_weekday(&captures[2]) else {
            continue;
        };
        let forward = captures[1].eq_ignore_ascii_case("next");
        let Some(date) = weekday_date(source_date, weekday, forward) else {
            continue;
        };
        push_day(found.as_str(), date, span, claimed_spans, anchors);
    }
    extract_periods(text, source_date, claimed_spans, anchors);
    extract_relative_offsets(text, source_date, claimed_spans, anchors);
    extract_simple(text, source_date, claimed_spans, anchors);
}

fn extract_periods(
    text: &str,
    source_date: time::Date,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    for captures in PERIOD_RE.captures_iter(text) {
        let found = captures.get(0).unwrap();
        let span = TextSpan {
            start: found.start(),
            end: found.end(),
        };
        if claimed(span, claimed_spans) {
            continue;
        }
        let offset = match captures[1].to_ascii_lowercase().as_str() {
            "this" => 0,
            "next" => 1,
            "last" => -1,
            _ => continue,
        };
        let unit = captures[2].to_ascii_lowercase();
        let value = match unit.as_str() {
            "week" => shift_week_start(source_date, offset)
                .and_then(week_span)
                .map(|bounds| (bounds, TemporalGranularity::Week)),
            "month" => shift_month_start(source_date, offset as i32)
                .and_then(month_span)
                .map(|bounds| (bounds, TemporalGranularity::Month)),
            "quarter" => shift_month_start(source_date, offset as i32 * 3)
                .and_then(quarter_span)
                .map(|bounds| (bounds, TemporalGranularity::Quarter)),
            "year" => shift_year_start(source_date, offset as i32)
                .and_then(year_span)
                .map(|bounds| (bounds, TemporalGranularity::Year)),
            _ => None,
        };
        let Some(((start_ns, end_ns), granularity)) = value else {
            continue;
        };
        anchors.push(relative_anchor(
            found.as_str(),
            start_ns,
            end_ns,
            granularity,
        ));
        push_claimed(claimed_spans, span);
    }
}

fn extract_simple(
    text: &str,
    source_date: time::Date,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    for found in SIMPLE_RE.find_iter(text) {
        let span = TextSpan {
            start: found.start(),
            end: found.end(),
        };
        if claimed(span, claimed_spans) {
            continue;
        }
        let offset = match found.as_str().to_ascii_lowercase().as_str() {
            "today" => 0,
            "yesterday" => -1,
            "tomorrow" => 1,
            _ => continue,
        };
        let Some(date) = shift_days(source_date, offset) else {
            continue;
        };
        push_day(found.as_str(), date, span, claimed_spans, anchors);
    }
}

fn push_day(
    evidence: &str,
    date: time::Date,
    span: TextSpan,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    let Some((start_ns, end_ns)) = day_span(date) else {
        return;
    };
    anchors.push(relative_anchor(
        evidence,
        start_ns,
        end_ns,
        TemporalGranularity::Day,
    ));
    push_claimed(claimed_spans, span);
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

pub(crate) fn parse_weekday(value: &str) -> Option<TemporalWeekday> {
    match value.to_ascii_lowercase().as_str() {
        "monday" => Some(TemporalWeekday::Monday),
        "tuesday" => Some(TemporalWeekday::Tuesday),
        "wednesday" => Some(TemporalWeekday::Wednesday),
        "thursday" => Some(TemporalWeekday::Thursday),
        "friday" => Some(TemporalWeekday::Friday),
        "saturday" => Some(TemporalWeekday::Saturday),
        "sunday" => Some(TemporalWeekday::Sunday),
        _ => None,
    }
}
