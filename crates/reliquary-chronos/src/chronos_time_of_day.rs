use crate::{TemporalClockPrecision, TemporalOrigin, TemporalTimeOfDay};
use regex::Regex;
use std::sync::LazyLock;

static MERIDIEM_TIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?P<hour>\d{1,2}):(?P<minute>\d{2})(?::(?P<second>\d{2}))?\s*(?P<meridiem>am|pm)\b",
    )
    .expect("valid Chronos meridiem time regex")
});

pub(crate) fn extract_times_of_day(text: &str) -> Vec<TemporalTimeOfDay> {
    MERIDIEM_TIME_RE
        .captures_iter(text)
        .filter_map(|captures| {
            let matched = captures.get(0)?;
            let hour = captures.name("hour")?.as_str().parse::<u8>().ok()?;
            let minute = captures.name("minute")?.as_str().parse::<u8>().ok()?;
            let second_capture = captures.name("second");
            let second = second_capture
                .map(|value| value.as_str().parse::<u8>())
                .transpose()
                .ok()?
                .unwrap_or(0);
            if !(1..=12).contains(&hour) || minute > 59 || second > 59 {
                return None;
            }

            let meridiem = captures.name("meridiem")?.as_str();
            let hour = match (hour, meridiem.eq_ignore_ascii_case("pm")) {
                (12, false) => 0,
                (12, true) => 12,
                (value, false) => value,
                (value, true) => value + 12,
            };

            Some(TemporalTimeOfDay {
                hour,
                minute,
                second,
                precision: if second_capture.is_some() {
                    TemporalClockPrecision::Second
                } else {
                    TemporalClockPrecision::Minute
                },
                origin: TemporalOrigin::Explicit,
                evidence: matched.as_str().to_owned(),
            })
        })
        .collect()
}
