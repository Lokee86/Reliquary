use crate::chronos_absolute_calendar::parse_month;
use crate::chronos_calendar::{
    date_from_ns, day_span, month_span, weekday_on_or_after, weekday_on_or_before, year_span,
};
use crate::chronos_relative::parse_weekday;
use crate::{TemporalAnchor, TemporalGranularity, TemporalOrigin};
use time::{Date, Month};

#[derive(Clone, Copy)]
pub(crate) enum EndpointDirection {
    Past,
    Future,
    Neutral,
}

pub(crate) fn parse_endpoint_exact(
    text: &str,
    reference_timestamp_ns: Option<i64>,
    direction: EndpointDirection,
) -> Option<TemporalAnchor> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    let mut claimed = Vec::new();
    let mut anchors = Vec::new();
    crate::chronos_absolute::extract_absolute(text, &mut claimed, &mut anchors);
    crate::chronos_relative::extract_relative(
        text,
        reference_timestamp_ns,
        &mut claimed,
        &mut anchors,
    );
    if let Some(anchor) = anchors
        .into_iter()
        .find(|anchor| anchor.evidence.eq_ignore_ascii_case(text))
    {
        return Some(anchor);
    }

    parse_loose_year(text)
        .or_else(|| parse_loose_month(text, reference_timestamp_ns, direction))
        .or_else(|| parse_loose_weekday(text, reference_timestamp_ns, direction))
}

pub(crate) fn has_endpoint_suffix(text: &str, reference_timestamp_ns: Option<i64>) -> bool {
    let segment_start = text
        .char_indices()
        .rev()
        .find_map(|(index, value)| {
            matches!(value, ',' | ';' | '.').then_some(index + value.len_utf8())
        })
        .unwrap_or(0);
    let segment = text[segment_start..].trim();
    if segment.is_empty() {
        return false;
    }

    let mut starts = vec![0];
    for (index, value) in segment.char_indices() {
        if value.is_whitespace() {
            starts.push(index + value.len_utf8());
        }
    }
    starts.into_iter().rev().take(7).any(|start| {
        parse_endpoint_exact(
            &segment[start..],
            reference_timestamp_ns,
            EndpointDirection::Neutral,
        )
        .is_some()
    })
}

pub(crate) fn parse_endpoint_prefix(
    text: &str,
    reference_timestamp_ns: Option<i64>,
    direction: EndpointDirection,
) -> Option<(TemporalAnchor, usize)> {
    let mut ends = Vec::new();
    for (index, found) in text.match_indices(char::is_whitespace) {
        if index > 0 {
            ends.push(index);
        }
        if ends.len() >= 7 {
            break;
        }
        let _ = found;
    }
    let terminal = text
        .char_indices()
        .find_map(|(index, value)| matches!(value, ',' | ';' | '.').then_some(index))
        .unwrap_or(text.len());
    ends.push(terminal);
    ends.sort_unstable();
    ends.dedup();

    for end in ends.into_iter().rev() {
        let candidate = text[..end].trim();
        if let Some(anchor) = parse_endpoint_exact(candidate, reference_timestamp_ns, direction) {
            let consumed = candidate.len();
            return Some((anchor, consumed));
        }
    }
    None
}

fn parse_loose_year(text: &str) -> Option<TemporalAnchor> {
    if text.len() != 4 || !text.bytes().all(|value| value.is_ascii_digit()) {
        return None;
    }
    let year = text.parse::<i32>().ok()?;
    let date = Date::from_calendar_date(year, Month::January, 1).ok()?;
    let (start_ns, end_ns) = year_span(date)?;
    Some(anchor(
        text,
        start_ns,
        end_ns,
        TemporalGranularity::Year,
        TemporalOrigin::Explicit,
    ))
}

fn parse_loose_month(
    text: &str,
    reference_timestamp_ns: Option<i64>,
    direction: EndpointDirection,
) -> Option<TemporalAnchor> {
    let month = parse_month(text)?;
    let reference = reference_timestamp_ns.and_then(date_from_ns)?;
    let mut year = reference.year();
    let reference_month = u8::from(reference.month());
    let target_month = u8::from(month);
    match direction {
        EndpointDirection::Past if target_month > reference_month => year = year.checked_sub(1)?,
        EndpointDirection::Future if target_month < reference_month => {
            year = year.checked_add(1)?
        }
        _ => {}
    }
    let date = Date::from_calendar_date(year, month, 1).ok()?;
    let (start_ns, end_ns) = month_span(date)?;
    Some(anchor(
        text,
        start_ns,
        end_ns,
        TemporalGranularity::Month,
        TemporalOrigin::Relative,
    ))
}

fn parse_loose_weekday(
    text: &str,
    reference_timestamp_ns: Option<i64>,
    direction: EndpointDirection,
) -> Option<TemporalAnchor> {
    let weekday = parse_weekday(text)?;
    let reference = reference_timestamp_ns.and_then(date_from_ns)?;
    let date = match direction {
        EndpointDirection::Past => weekday_on_or_before(reference, weekday)?,
        EndpointDirection::Future | EndpointDirection::Neutral => {
            weekday_on_or_after(reference, weekday)?
        }
    };
    let (start_ns, end_ns) = day_span(date)?;
    Some(anchor(
        text,
        start_ns,
        end_ns,
        TemporalGranularity::Day,
        TemporalOrigin::Relative,
    ))
}

fn anchor(
    evidence: &str,
    start_ns: i64,
    end_ns: i64,
    granularity: TemporalGranularity,
    origin: TemporalOrigin,
) -> TemporalAnchor {
    TemporalAnchor {
        start_ns,
        end_ns,
        granularity,
        origin,
        evidence: evidence.to_owned(),
    }
}
