use crate::chronos_absolute::explicit_anchor;
use crate::chronos_absolute_calendar::parse_month;
use crate::chronos_calendar::{day_span, month_span};
use crate::chronos_loose_calendar::{MONTH_ABBR_RE, MONTH_RE, parse_abbreviation};
use crate::chronos_parser::{TextSpan, claimed, push_claimed};
use crate::{TemporalAnchor, TemporalGranularity, TemporalOrigin};
use regex::Regex;
use std::sync::LazyLock;
use time::Date;

static DAY_OF_MONTH_YEAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b(\d{{1,2}})(?:st|nd|rd|th)?\s+of\s+({MONTH_RE})\s+(\d{{4}})\b"
    ))
    .expect("valid Chronos day-of-month-year regex")
});
static ABBR_DAY_YEAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b({MONTH_ABBR_RE})\.?\s+(\d{{1,2}})(?:st|nd|rd|th)?(?:,)?\s+(\d{{4}})\b"
    ))
    .expect("valid Chronos abbreviated date regex")
});
static ABBR_MONTH_YEAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?i)\b({MONTH_ABBR_RE})\.?\s+(\d{{4}})\b"))
        .expect("valid Chronos abbreviated month regex")
});

pub(crate) fn extract(
    text: &str,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    for captures in DAY_OF_MONTH_YEAR_RE.captures_iter(text) {
        push_day(
            captures.get(0).unwrap(),
            parse_month(&captures[2]),
            captures[1].parse().ok(),
            captures[3].parse().ok(),
            claimed_spans,
            anchors,
        );
    }
    for captures in ABBR_DAY_YEAR_RE.captures_iter(text) {
        push_day(
            captures.get(0).unwrap(),
            parse_abbreviation(&captures[1]),
            captures[2].parse().ok(),
            captures[3].parse().ok(),
            claimed_spans,
            anchors,
        );
    }
    extract_abbreviated_months(text, claimed_spans, anchors);
}

fn extract_abbreviated_months(
    text: &str,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    for captures in ABBR_MONTH_YEAR_RE.captures_iter(text) {
        let found = captures.get(0).unwrap();
        let span = span(found);
        if claimed(span, claimed_spans) {
            continue;
        }
        let (Some(month), Ok(year)) = (parse_abbreviation(&captures[1]), captures[2].parse())
        else {
            continue;
        };
        let Ok(date) = Date::from_calendar_date(year, month, 1) else {
            continue;
        };
        let Some((start_ns, end_ns)) = month_span(date) else {
            continue;
        };
        anchors.push(explicit_anchor(
            found.as_str(),
            start_ns,
            end_ns,
            TemporalGranularity::Month,
        ));
        push_claimed(claimed_spans, span);
    }
}

fn push_day(
    found: regex::Match<'_>,
    month: Option<time::Month>,
    day: Option<u8>,
    year: Option<i32>,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    let span = span(found);
    if claimed(span, claimed_spans) {
        return;
    }
    let (Some(month), Some(day), Some(year)) = (month, day, year) else {
        return;
    };
    let Ok(date) = Date::from_calendar_date(year, month, day) else {
        return;
    };
    let Some((start_ns, end_ns)) = day_span(date) else {
        return;
    };
    anchors.push(TemporalAnchor {
        start_ns,
        end_ns,
        granularity: TemporalGranularity::Day,
        origin: TemporalOrigin::Explicit,
        evidence: found.as_str().to_owned(),
    });
    push_claimed(claimed_spans, span);
}

fn span(found: regex::Match<'_>) -> TextSpan {
    TextSpan {
        start: found.start(),
        end: found.end(),
    }
}
