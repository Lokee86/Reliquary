use crate::chronos_absolute::explicit_anchor;
use crate::chronos_calendar::month_number;
use crate::chronos_parser::{TextSpan, claimed, push_claimed};
use crate::{TemporalAnchor, TemporalGranularity};
use regex::Regex;
use std::sync::LazyLock;
use time::{Date, Month};

static PREFIX_SEASON_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(northern|southern)(?:\s+hemisphere)?\s+(spring|summer|autumn|fall|winter)\s+(\d{4})\b",
    )
    .expect("valid Chronos hemisphere-season regex")
});
static SUFFIX_SEASON_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(spring|summer|autumn|fall|winter)\s+(\d{4})\s+in\s+the\s+(northern|southern)\s+hemisphere\b",
    )
    .expect("valid Chronos season-hemisphere regex")
});

pub(crate) fn extract_seasons(
    text: &str,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    for captures in PREFIX_SEASON_RE.captures_iter(text) {
        push_season(
            captures.get(0).unwrap(),
            &captures[1],
            &captures[2],
            &captures[3],
            claimed_spans,
            anchors,
        );
    }
    for captures in SUFFIX_SEASON_RE.captures_iter(text) {
        push_season(
            captures.get(0).unwrap(),
            &captures[3],
            &captures[1],
            &captures[2],
            claimed_spans,
            anchors,
        );
    }
}

fn push_season(
    found: regex::Match<'_>,
    hemisphere: &str,
    season: &str,
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
    let Ok(year) = year.parse::<i32>() else {
        return;
    };
    let Some((start, end)) = season_dates(hemisphere, season, year) else {
        return;
    };
    let (Some(start_ns), Some(end_ns)) = (date_ns(start), date_ns(end)) else {
        return;
    };
    anchors.push(explicit_anchor(
        found.as_str(),
        start_ns,
        end_ns,
        TemporalGranularity::Season,
    ));
    push_claimed(claimed_spans, span);
}

fn season_dates(hemisphere: &str, season: &str, year: i32) -> Option<(Date, Date)> {
    let northern = hemisphere.eq_ignore_ascii_case("northern");
    let season = season.to_ascii_lowercase();
    let (start_month, crosses_year) = match (northern, season.as_str()) {
        (true, "spring") | (false, "autumn" | "fall") => (Month::March, false),
        (true, "summer") | (false, "winter") => (Month::June, false),
        (true, "autumn" | "fall") | (false, "spring") => (Month::September, false),
        (true, "winter") | (false, "summer") => (Month::December, true),
        _ => return None,
    };
    let start = Date::from_calendar_date(year, start_month, 1).ok()?;
    let end_month = ((month_number(start_month) - 1 + 3) % 12) + 1;
    let end_year = if crosses_year {
        year.checked_add(1)?
    } else {
        year
    };
    let end = Date::from_calendar_date(end_year, Month::try_from(end_month).ok()?, 1).ok()?;
    Some((start, end))
}

fn date_ns(date: Date) -> Option<i64> {
    i64::try_from(date.midnight().assume_utc().unix_timestamp_nanos()).ok()
}
