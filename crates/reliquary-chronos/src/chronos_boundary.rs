use crate::chronos_boundary_endpoint::{
    EndpointDirection, has_endpoint_suffix, parse_endpoint_exact, parse_endpoint_prefix,
};
use crate::chronos_parser::{TextSpan, claimed, push_claimed};
use crate::{TemporalAnchor, TemporalGranularity, TemporalInterval, TemporalOrigin};
use regex::Regex;
use std::sync::LazyLock;

static FROM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bfrom\b").expect("valid Chronos from regex"));
static RANGE_SEPARATOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:to|through|until)\b|[–—]").expect("valid Chronos range separator regex")
});
static BOUNDARY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(since|until|before|after|starting|ending)\b")
        .expect("valid Chronos boundary regex")
});

pub(crate) fn extract_boundaries(
    text: &str,
    reference_timestamp_ns: Option<i64>,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
    intervals: &mut Vec<TemporalInterval>,
) {
    extract_from_ranges(
        text,
        reference_timestamp_ns,
        claimed_spans,
        anchors,
        intervals,
    );
    extract_open_boundaries(text, reference_timestamp_ns, claimed_spans, intervals);
}

fn extract_from_ranges(
    text: &str,
    reference_timestamp_ns: Option<i64>,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
    intervals: &mut Vec<TemporalInterval>,
) {
    for from in FROM_RE.find_iter(text) {
        let tail = &text[from.end()..];
        let Some(separator) = RANGE_SEPARATOR_RE.find(tail) else {
            continue;
        };
        let left_text = tail[..separator.start()].trim();
        let Some(left) = parse_endpoint_exact(
            left_text,
            reference_timestamp_ns,
            EndpointDirection::Neutral,
        ) else {
            continue;
        };
        let raw_right = &tail[separator.end()..];
        let right_leading = raw_right.len() - raw_right.trim_start().len();
        let right_tail = &raw_right[right_leading..];
        let Some((mut right, right_len)) = parse_endpoint_prefix(
            right_tail,
            reference_timestamp_ns,
            EndpointDirection::Neutral,
        ) else {
            continue;
        };
        adjust_cross_period_range(&left, &mut right);
        if left.start_ns > right.end_ns {
            continue;
        }

        let right_start = from.end() + separator.end() + right_leading;
        let span = TextSpan {
            start: from.start(),
            end: right_start + right_len,
        };
        if claimed(span, claimed_spans) {
            continue;
        }
        let evidence = &text[span.start..span.end];
        let origin = combined_origin(left.origin, right.origin);
        intervals.push(TemporalInterval {
            start_ns: Some(left.start_ns),
            end_ns: Some(right.end_ns),
            start_granularity: Some(left.granularity),
            end_granularity: Some(right.granularity),
            origin,
            evidence: evidence.to_owned(),
        });
        anchors.push(TemporalAnchor {
            start_ns: left.start_ns,
            end_ns: right.end_ns,
            granularity: TemporalGranularity::Range,
            origin,
            evidence: evidence.to_owned(),
        });
        push_claimed(claimed_spans, span);
    }
}

fn extract_open_boundaries(
    text: &str,
    reference_timestamp_ns: Option<i64>,
    claimed_spans: &mut Vec<TextSpan>,
    intervals: &mut Vec<TemporalInterval>,
) {
    for captures in BOUNDARY_RE.captures_iter(text) {
        let operator = captures.get(1).unwrap();
        let operator_text = operator.as_str().to_ascii_lowercase();
        if operator_text == "until"
            && has_endpoint_suffix(&text[..operator.start()], reference_timestamp_ns)
        {
            continue;
        }
        let direction = match operator_text.as_str() {
            "since" | "after" => EndpointDirection::Past,
            "until" | "before" | "ending" => EndpointDirection::Future,
            "starting" => EndpointDirection::Neutral,
            _ => continue,
        };
        let tail = &text[operator.end()..];
        let leading = tail.len() - tail.trim_start().len();
        let endpoint_tail = &tail[leading..];
        let Some((anchor, consumed)) =
            parse_endpoint_prefix(endpoint_tail, reference_timestamp_ns, direction)
        else {
            continue;
        };
        let endpoint_start = operator.end() + leading;
        let span = TextSpan {
            start: operator.start(),
            end: endpoint_start + consumed,
        };
        if claimed(span, claimed_spans) {
            continue;
        }
        let evidence = &text[span.start..span.end];
        let (start_ns, end_ns, start_granularity, end_granularity) = match operator_text.as_str() {
            "since" | "starting" => (Some(anchor.start_ns), None, Some(anchor.granularity), None),
            "after" => (Some(anchor.end_ns), None, Some(anchor.granularity), None),
            "until" | "ending" => (None, Some(anchor.end_ns), None, Some(anchor.granularity)),
            "before" => (None, Some(anchor.start_ns), None, Some(anchor.granularity)),
            _ => continue,
        };
        intervals.push(TemporalInterval {
            start_ns,
            end_ns,
            start_granularity,
            end_granularity,
            origin: anchor.origin,
            evidence: evidence.to_owned(),
        });
        push_claimed(claimed_spans, span);
    }
}

fn adjust_cross_period_range(left: &TemporalAnchor, right: &mut TemporalAnchor) {
    if right.end_ns >= left.start_ns || right.origin != TemporalOrigin::Relative {
        return;
    }
    let Some(date) = crate::chronos_calendar::date_from_ns(right.start_ns) else {
        return;
    };
    let shifted = match right.granularity {
        TemporalGranularity::Month => crate::chronos_calendar::shift_months_clamped(date, 12),
        TemporalGranularity::Day => crate::chronos_calendar::shift_days(date, 7),
        _ => None,
    };
    let Some(shifted) = shifted else {
        return;
    };
    let span = match right.granularity {
        TemporalGranularity::Month => crate::chronos_calendar::month_span(shifted),
        TemporalGranularity::Day => crate::chronos_calendar::day_span(shifted),
        _ => None,
    };
    let Some((start_ns, end_ns)) = span else {
        return;
    };
    right.start_ns = start_ns;
    right.end_ns = end_ns;
}

fn combined_origin(left: TemporalOrigin, right: TemporalOrigin) -> TemporalOrigin {
    if left == TemporalOrigin::Explicit && right == TemporalOrigin::Explicit {
        TemporalOrigin::Explicit
    } else {
        TemporalOrigin::Relative
    }
}
