use crate::chronos_number::{WORD_NUMBER_RE, parse_number};
use crate::chronos_vocabulary::{is_calendar_word, is_month_abbreviation, is_unit};
use crate::{TemporalDurationUnit, TemporalEventDirection, TemporalEventRelation};
use regex::Regex;
use std::sync::LazyLock;

static EVENT_RELATIVE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b(?P<amount>{number}|\d{{1,4}})\s+(?P<unit>second|minute|hour|day|week|month|quarter|year)s?\s+(?P<direction>before|after)\s+",
        number = WORD_NUMBER_RE
    ))
    .expect("valid Chronos event-relative regex")
});

static EVENT_WORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[A-Za-z][A-Za-z0-9_-]*").expect("valid Chronos event word regex")
});

pub(crate) fn extract_event_relations(text: &str) -> Vec<TemporalEventRelation> {
    EVENT_RELATIVE_RE
        .captures_iter(text)
        .filter_map(|captures| {
            let prefix = captures.get(0)?;
            let amount = positive_u16(captures.name("amount")?.as_str())?;
            let unit = duration_unit(captures.name("unit")?.as_str())?;
            let direction = match captures
                .name("direction")?
                .as_str()
                .to_ascii_lowercase()
                .as_str()
            {
                "before" => TemporalEventDirection::Before,
                "after" => TemporalEventDirection::After,
                _ => return None,
            };
            let (reference_event, event_len) = event_phrase(&text[prefix.end()..])?;
            let evidence = text[prefix.start()..prefix.end() + event_len].to_owned();
            Some(TemporalEventRelation {
                amount,
                unit,
                direction,
                reference_event,
                evidence,
            })
        })
        .collect()
}

fn event_phrase(tail: &str) -> Option<(String, usize)> {
    let first = EVENT_WORD_RE.find(tail)?;
    if first.start() != 0 || temporal_target(first.as_str()) {
        return None;
    }
    let first_lower = first.as_str().to_ascii_lowercase();
    let determiner = matches!(first_lower.as_str(), "the" | "a" | "an" | "this" | "that");
    if !determiner {
        return Some((first.as_str().to_owned(), first.end()));
    }

    let mut end = first.end();
    let mut content_words = 0usize;
    for word in EVENT_WORD_RE.find_iter(&tail[first.end()..]) {
        let start = first.end() + word.start();
        let word_end = first.end() + word.end();
        if !tail[end..start].chars().all(char::is_whitespace) {
            break;
        }
        let lower = word.as_str().to_ascii_lowercase();
        if event_stop_word(&lower) || temporal_target(&lower) {
            break;
        }
        end = word_end;
        content_words += 1;
        if content_words == 3 {
            break;
        }
    }
    (content_words > 0).then(|| (tail[..end].to_owned(), end))
}

fn temporal_target(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    is_unit(&lower)
        || is_calendar_word(&lower)
        || is_month_abbreviation(&lower)
        || matches!(lower.as_str(), "today" | "tomorrow" | "yesterday")
}

fn event_stop_word(value: &str) -> bool {
    matches!(
        value,
        "and"
            | "but"
            | "then"
            | "on"
            | "at"
            | "in"
            | "for"
            | "with"
            | "without"
            | "by"
            | "from"
            | "until"
            | "before"
            | "after"
            | "during"
            | "while"
            | "when"
    )
}

fn positive_u16(value: &str) -> Option<u16> {
    u16::try_from(parse_number(value)?)
        .ok()
        .filter(|value| *value > 0)
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
