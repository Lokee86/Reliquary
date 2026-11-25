//! Resolve natural-language Archive queries before any retrieval work begins.

use chrono::{DateTime, LocalResult, Offset, TimeZone, Utc};
use chrono_tz::Tz;
use reliquary_retrieval::{TemporalBasis, TemporalFilter};
use std::{error::Error, fmt};

pub const TEMPORAL_QUERY_REQUIRED_MESSAGE: &str = "Archive search requires a resolvable time reference. If temporal information is unavailable, try memory_query.";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedArchiveTemporalQuery {
    pub topic: String,
    pub filter: TemporalFilter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalQueryError {
    TemporalQueryRequired,
}

impl TemporalQueryError {
    pub const fn kind(self) -> &'static str {
        "temporal_query_required"
    }
}

impl fmt::Display for TemporalQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(TEMPORAL_QUERY_REQUIRED_MESSAGE)
    }
}

impl Error for TemporalQueryError {}

/// Split a natural-language request into a topical query and a frozen temporal filter.
/// `request_time_ns` and `calendar_zone` must come from the trusted request context.
pub fn resolve_archive_temporal_query(
    query: &str,
    basis: TemporalBasis,
    request_time_ns: i64,
    calendar_zone: &str,
) -> Result<ResolvedArchiveTemporalQuery, TemporalQueryError> {
    let zone: Tz = calendar_zone
        .parse()
        .map_err(|_| TemporalQueryError::TemporalQueryRequired)?;
    let request_time = DateTime::<Utc>::from_timestamp_nanos(request_time_ns);
    let local_reference = request_time.with_timezone(&zone);
    // Chronos currently resolves calendar language against UTC dates. Present it the same local
    // wall-clock value, then map the resulting calendar dates back through the IANA zone below.
    let local_wallclock_ns = request_time_ns
        .checked_add(i64::from(local_reference.offset().fix().local_minus_utc()) * 1_000_000_000)
        .ok_or(TemporalQueryError::TemporalQueryRequired)?;
    let analysis = reliquary_chronos::analyze(query, Some(local_wallclock_ns));
    let mut evidence = analysis
        .anchors
        .iter()
        .map(|anchor| anchor.evidence.clone())
        .collect::<Vec<_>>();
    evidence.extend(
        analysis
            .intervals
            .iter()
            .map(|interval| interval.evidence.clone()),
    );
    let mut ranges = Vec::new();
    for anchor in analysis.anchors {
        let start = local_timestamp_ns(zone, anchor.start_ns)?;
        let end = local_timestamp_ns(zone, anchor.end_ns)?;
        if start < end {
            ranges.push((start, end));
        }
    }
    let mut earliest = None;
    let mut latest = None;
    for interval in analysis.intervals {
        match (interval.start_ns, interval.end_ns) {
            (Some(start), Some(end)) => {
                let start = local_timestamp_ns(zone, start)?;
                let end = local_timestamp_ns(zone, end)?;
                if start < end {
                    ranges.push((start, end));
                }
            }
            (Some(start), None) => {
                let start = local_timestamp_ns(zone, start)?;
                earliest = Some(earliest.map_or(start, |prior: i64| prior.max(start)));
            }
            (None, Some(end)) => {
                let end = local_timestamp_ns(zone, end)?;
                latest = Some(latest.map_or(end, |prior: i64| prior.min(end)));
            }
            (None, None) => {}
        }
    }
    if ranges.is_empty() && earliest.is_none() && latest.is_none() {
        return Err(TemporalQueryError::TemporalQueryRequired);
    }
    let earliest = earliest.unwrap_or(i64::MIN);
    let latest = latest.unwrap_or(i64::MAX);
    ranges.sort_unstable();
    let (start_ns, mut end_ns) = ranges.first().copied().unwrap_or((earliest, latest));
    for (start, end) in ranges.into_iter().skip(1) {
        if start > end_ns {
            return Err(TemporalQueryError::TemporalQueryRequired);
        }
        end_ns = end_ns.max(end);
    }
    let filter = TemporalFilter::new(basis, start_ns.max(earliest), end_ns.min(latest))
        .ok_or(TemporalQueryError::TemporalQueryRequired)?;
    let topic = remove_temporal_evidence(query, &evidence);
    Ok(ResolvedArchiveTemporalQuery { topic, filter })
}

fn remove_temporal_evidence(query: &str, evidence: &[String]) -> String {
    let mut topic = query.to_owned();
    for phrase in evidence {
        if phrase.is_empty() {
            continue;
        }
        if let Some(index) = topic
            .to_ascii_lowercase()
            .find(&phrase.to_ascii_lowercase())
        {
            topic.replace_range(index..index + phrase.len(), " ");
        }
    }
    let topic = topic
        .trim_matches(|character: char| !character.is_alphanumeric())
        .trim();
    let topic = strip_request_framing(topic);
    let topic = topic
        .trim_end_matches(|character: char| !character.is_alphanumeric())
        .trim();
    let topic = strip_temporal_connector(topic);
    if topic.is_empty() {
        query.trim()
    } else {
        topic
    }
    .to_owned()
}

fn strip_temporal_connector(mut value: &str) -> &str {
    for connector in [" on", " at", " during", " in", " from", " by"] {
        if value.to_ascii_lowercase().ends_with(connector) {
            value = value[..value.len() - connector.len()].trim_end();
            break;
        }
    }
    value
}

fn strip_request_framing(mut value: &str) -> &str {
    const PREFIXES: &[&str] = &[
        "what did i say about ",
        "what did i write about ",
        "what did we say about ",
        "find references to ",
        "find mentions of ",
        "search for ",
        "find ",
        "show me ",
        "what about ",
    ];
    loop {
        let lower = value.to_ascii_lowercase();
        if let Some(prefix) = PREFIXES.iter().find(|prefix| lower.starts_with(**prefix)) {
            value = value[prefix.len()..].trim_start();
        } else {
            break;
        }
    }
    value
}

fn local_timestamp_ns(zone: Tz, timestamp_ns: i64) -> Result<i64, TemporalQueryError> {
    let wallclock = DateTime::<Utc>::from_timestamp_nanos(timestamp_ns).naive_utc();
    let local = match zone.from_local_datetime(&wallclock) {
        LocalResult::Single(value) => value,
        LocalResult::Ambiguous(first, second) => first.min(second),
        LocalResult::None => return Err(TemporalQueryError::TemporalQueryRequired),
    };
    local
        .timestamp_nanos_opt()
        .ok_or(TemporalQueryError::TemporalQueryRequired)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ns(value: &str) -> i64 {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .timestamp_nanos_opt()
            .unwrap()
    }

    #[test]
    fn resolves_local_calendar_day_across_dst_and_freezes_basis() {
        let resolved = resolve_archive_temporal_query(
            "what did I say about gateway last Sunday",
            TemporalBasis::Source,
            ns("2026-11-02T12:00:00Z"),
            "America/Vancouver",
        )
        .unwrap();
        assert_eq!(resolved.topic, "gateway");
        assert_eq!(resolved.filter.basis, TemporalBasis::Source);
        assert_eq!(resolved.filter.start_ns, ns("2026-11-01T07:00:00Z"));
        assert_eq!(resolved.filter.end_exclusive_ns, ns("2026-11-02T08:00:00Z"));
    }

    #[test]
    fn missing_or_unresolvable_temporal_input_fails_closed() {
        for query in [
            "gateway rollback",
            "what did I say about gateway the day before launch",
        ] {
            assert_eq!(
                resolve_archive_temporal_query(
                    query,
                    TemporalBasis::Mentioned,
                    ns("2026-11-24T12:00:00Z"),
                    "America/Vancouver",
                ),
                Err(TemporalQueryError::TemporalQueryRequired)
            );
        }
        let error = TemporalQueryError::TemporalQueryRequired;
        assert_eq!(error.kind(), "temporal_query_required");
        assert_eq!(error.to_string(), TEMPORAL_QUERY_REQUIRED_MESSAGE);
    }

    #[test]
    fn resolves_one_sided_queries_and_preserves_unicode_topics() {
        let request = ns("2026-11-24T12:00:00Z");
        let resolved = resolve_archive_temporal_query(
            "İstanbul since 2026-11-20",
            TemporalBasis::Source,
            request,
            "UTC",
        )
        .unwrap();
        assert_eq!(resolved.topic, "İstanbul");
        assert_eq!(resolved.filter.start_ns, ns("2026-11-20T00:00:00Z"));
        assert_eq!(resolved.filter.end_exclusive_ns, i64::MAX);
        assert!(
            resolve_archive_temporal_query("today", TemporalBasis::Source, request, "UTC").is_ok()
        );
    }
}
