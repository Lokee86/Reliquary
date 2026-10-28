use crate::chronos_calendar::{day_span, shift_days, shift_months_clamped};
use crate::chronos_number::{WORD_NUMBER_RE, parse_number};
use crate::chronos_parser::{TextSpan, claimed, push_claimed};
use crate::{TemporalAnchor, TemporalGranularity, TemporalOrigin};
use regex::Regex;
use std::sync::LazyLock;
use time::Date;

static OFFSET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b(?:in\s+({number}|\d{{1,4}})\s+(day|week|month|year)s?|({number}|\d{{1,4}})\s+(day|week|month|year)s?\s+ago)\b",
        number = WORD_NUMBER_RE
    ))
    .expect("valid Chronos relative-offset regex")
});

pub(crate) fn extract_relative_offsets(
    text: &str,
    source_date: Date,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    for captures in OFFSET_RE.captures_iter(text) {
        let found = captures.get(0).unwrap();
        let span = TextSpan {
            start: found.start(),
            end: found.end(),
        };
        if claimed(span, claimed_spans) {
            continue;
        }

        let future = captures.get(1).is_some();
        let number = captures.get(1).or_else(|| captures.get(3));
        let unit = captures.get(2).or_else(|| captures.get(4));
        let (Some(number), Some(unit)) = (number, unit) else {
            continue;
        };
        let Some(number) = parse_number(number.as_str()) else {
            continue;
        };
        let direction = if future { 1_i64 } else { -1_i64 };
        let Some(date) = resolve_offset(source_date, number, unit.as_str(), direction) else {
            continue;
        };
        let Some((start_ns, end_ns)) = day_span(date) else {
            continue;
        };
        anchors.push(TemporalAnchor {
            start_ns,
            end_ns,
            granularity: TemporalGranularity::Day,
            origin: TemporalOrigin::Relative,
            evidence: found.as_str().to_owned(),
        });
        push_claimed(claimed_spans, span);
    }
}

fn resolve_offset(source: Date, number: i64, unit: &str, direction: i64) -> Option<Date> {
    let signed = number.checked_mul(direction)?;
    match unit.to_ascii_lowercase().as_str() {
        "day" => shift_days(source, signed),
        "week" => shift_days(source, signed.checked_mul(7)?),
        "month" => shift_months_clamped(source, i32::try_from(signed).ok()?),
        "year" => shift_months_clamped(source, i32::try_from(signed.checked_mul(12)?).ok()?),
        _ => None,
    }
}
