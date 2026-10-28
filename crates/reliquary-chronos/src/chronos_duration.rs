use crate::chronos_number::{WORD_NUMBER_RE, parse_number};
use crate::{
    TemporalApproximateDuration, TemporalDuration, TemporalDurationApproximation,
    TemporalDurationRange, TemporalDurationUnit,
};
use regex::Regex;
use std::sync::LazyLock;

const DURATION_CUE: &str = r"(?:for|lasting|lasted|duration(?:\s+of)?|takes?|took)";
const DURATION_UNIT: &str = r"(second|minute|hour|day|week|month|quarter|year)s?";

static DURATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b{cue}\s+({number}|\d{{1,4}})\s+{unit}\b",
        cue = DURATION_CUE,
        number = WORD_NUMBER_RE,
        unit = DURATION_UNIT
    ))
    .expect("valid Chronos duration regex")
});

static DURATION_RANGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b{cue}\s+({number}|\d{{1,4}})\s*(?:-|–|—|to)\s*({number}|\d{{1,4}})\s+{unit}\b",
        cue = DURATION_CUE,
        number = WORD_NUMBER_RE,
        unit = DURATION_UNIT
    ))
    .expect("valid Chronos duration range regex")
});

static APPROXIMATE_NUMERIC_DURATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b{cue}\s+(about|around|approximately|roughly)\s+({number}|\d{{1,4}})\s+{unit}\b",
        cue = DURATION_CUE,
        number = WORD_NUMBER_RE,
        unit = DURATION_UNIT
    ))
    .expect("valid Chronos approximate numeric duration regex")
});

static APPROXIMATE_QUALITATIVE_DURATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b{cue}\s+(?:a\s+)?(few|several|couple(?:\s+of)?)\s+{unit}\b",
        cue = DURATION_CUE,
        unit = DURATION_UNIT
    ))
    .expect("valid Chronos approximate qualitative duration regex")
});

pub(crate) fn extract_durations(text: &str) -> Vec<TemporalDuration> {
    DURATION_RE
        .captures_iter(text)
        .filter_map(|captures| {
            let found = captures.get(0)?;
            let amount = positive_u16(captures.get(1)?.as_str())?;
            let unit = duration_unit(captures.get(2)?.as_str())?;
            Some(TemporalDuration {
                amount,
                unit,
                evidence: found.as_str().to_owned(),
            })
        })
        .collect()
}

pub(crate) fn extract_duration_ranges(text: &str) -> Vec<TemporalDurationRange> {
    DURATION_RANGE_RE
        .captures_iter(text)
        .filter_map(|captures| {
            let found = captures.get(0)?;
            let min_amount = positive_u16(captures.get(1)?.as_str())?;
            let max_amount = positive_u16(captures.get(2)?.as_str())?;
            if min_amount >= max_amount {
                return None;
            }
            Some(TemporalDurationRange {
                min_amount,
                max_amount,
                unit: duration_unit(captures.get(3)?.as_str())?,
                evidence: found.as_str().to_owned(),
            })
        })
        .collect()
}

pub(crate) fn extract_approximate_durations(text: &str) -> Vec<TemporalApproximateDuration> {
    let numeric = APPROXIMATE_NUMERIC_DURATION_RE
        .captures_iter(text)
        .filter_map(|captures| {
            let found = captures.get(0)?;
            Some(TemporalApproximateDuration {
                amount: Some(positive_u16(captures.get(2)?.as_str())?),
                approximation: approximation(captures.get(1)?.as_str())?,
                unit: duration_unit(captures.get(3)?.as_str())?,
                evidence: found.as_str().to_owned(),
            })
        });
    let qualitative = APPROXIMATE_QUALITATIVE_DURATION_RE
        .captures_iter(text)
        .filter_map(|captures| {
            let found = captures.get(0)?;
            Some(TemporalApproximateDuration {
                amount: None,
                approximation: approximation(captures.get(1)?.as_str())?,
                unit: duration_unit(captures.get(2)?.as_str())?,
                evidence: found.as_str().to_owned(),
            })
        });
    numeric.chain(qualitative).collect()
}

fn positive_u16(value: &str) -> Option<u16> {
    u16::try_from(parse_number(value)?)
        .ok()
        .filter(|value| *value > 0)
}

fn approximation(value: &str) -> Option<TemporalDurationApproximation> {
    Some(match value.to_ascii_lowercase().as_str() {
        "about" => TemporalDurationApproximation::About,
        "around" => TemporalDurationApproximation::Around,
        "approximately" => TemporalDurationApproximation::Approximately,
        "roughly" => TemporalDurationApproximation::Roughly,
        "couple" | "couple of" => TemporalDurationApproximation::Couple,
        "few" => TemporalDurationApproximation::Few,
        "several" => TemporalDurationApproximation::Several,
        _ => return None,
    })
}

fn duration_unit(value: &str) -> Option<TemporalDurationUnit> {
    Some(match value.to_ascii_lowercase().as_str() {
        "second" => TemporalDurationUnit::Second,
        "minute" => TemporalDurationUnit::Minute,
        "hour" => TemporalDurationUnit::Hour,
        "day" => TemporalDurationUnit::Day,
        "week" => TemporalDurationUnit::Week,
        "month" => TemporalDurationUnit::Month,
        "quarter" => TemporalDurationUnit::Quarter,
        "year" => TemporalDurationUnit::Year,
        _ => return None,
    })
}
