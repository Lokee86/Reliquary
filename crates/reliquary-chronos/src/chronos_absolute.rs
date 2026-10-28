use crate::chronos_calendar::{day_span, instant_to_ns};
use crate::chronos_parser::{TextSpan, claimed, push_claimed};
use crate::{TemporalAnchor, TemporalGranularity, TemporalOrigin};
use regex::Regex;
use std::sync::LazyLock;
use time::format_description::well_known::Rfc3339;
use time::{Date, Month, OffsetDateTime};

static TIMESTAMP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?(?:Z|[+-]\d{2}:\d{2})\b")
        .unwrap()
});
static DATE_RANGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(\d{4}-\d{2}-\d{2})\s+(?:to|through|until|–|—|--)\s+(\d{4}-\d{2}-\d{2})\b")
        .unwrap()
});
static ISO_DATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{4}-\d{2}-\d{2}\b").unwrap());

pub(crate) fn extract_absolute(
    text: &str,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    extract_timestamps(text, claimed_spans, anchors);
    extract_date_ranges(text, claimed_spans, anchors);
    crate::chronos_absolute_calendar::extract_natural_dates(text, claimed_spans, anchors);
    crate::chronos_loose_calendar::extract_explicit(text, claimed_spans, anchors);
    crate::chronos_season::extract_seasons(text, claimed_spans, anchors);
    extract_iso_dates(text, claimed_spans, anchors);
    crate::chronos_absolute_calendar::extract_coarse(text, claimed_spans, anchors);
}

fn extract_timestamps(
    text: &str,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    for found in TIMESTAMP_RE.find_iter(text) {
        let span = TextSpan {
            start: found.start(),
            end: found.end(),
        };
        if claimed(span, claimed_spans) {
            continue;
        }
        let Ok(value) = OffsetDateTime::parse(found.as_str(), &Rfc3339) else {
            continue;
        };
        let Some(start_ns) = instant_to_ns(value) else {
            continue;
        };
        anchors.push(explicit_anchor(
            found.as_str(),
            start_ns,
            start_ns.saturating_add(1),
            TemporalGranularity::Instant,
        ));
        push_claimed(claimed_spans, span);
    }
}

fn extract_date_ranges(
    text: &str,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    for captures in DATE_RANGE_RE.captures_iter(text) {
        let found = captures.get(0).unwrap();
        let span = TextSpan {
            start: found.start(),
            end: found.end(),
        };
        if claimed(span, claimed_spans) {
            continue;
        }
        let (Some(start), Some(end)) = (parse_iso_date(&captures[1]), parse_iso_date(&captures[2]))
        else {
            continue;
        };
        if start > end {
            continue;
        }
        let (Some((start_ns, _)), Some((_, end_ns))) = (day_span(start), day_span(end)) else {
            continue;
        };
        anchors.push(explicit_anchor(
            found.as_str(),
            start_ns,
            end_ns,
            TemporalGranularity::Range,
        ));
        push_claimed(claimed_spans, span);
    }
}

fn extract_iso_dates(
    text: &str,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    for found in ISO_DATE_RE.find_iter(text) {
        let span = TextSpan {
            start: found.start(),
            end: found.end(),
        };
        if claimed(span, claimed_spans) {
            continue;
        }
        let Some(date) = parse_iso_date(found.as_str()) else {
            continue;
        };
        let Some((start_ns, end_ns)) = day_span(date) else {
            continue;
        };
        anchors.push(explicit_anchor(
            found.as_str(),
            start_ns,
            end_ns,
            TemporalGranularity::Day,
        ));
        push_claimed(claimed_spans, span);
    }
}

pub(crate) fn explicit_anchor(
    evidence: &str,
    start_ns: i64,
    end_ns: i64,
    granularity: TemporalGranularity,
) -> TemporalAnchor {
    TemporalAnchor {
        start_ns,
        end_ns,
        granularity,
        origin: TemporalOrigin::Explicit,
        evidence: evidence.to_owned(),
    }
}

fn parse_iso_date(value: &str) -> Option<Date> {
    let mut fields = value.split('-');
    let year = fields.next()?.parse::<i32>().ok()?;
    let month = fields.next()?.parse::<u8>().ok()?;
    let day = fields.next()?.parse::<u8>().ok()?;
    if fields.next().is_some() {
        return None;
    }
    Date::from_calendar_date(year, Month::try_from(month).ok()?, day).ok()
}
