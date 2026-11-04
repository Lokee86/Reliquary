use crate::chronos_vocabulary::{
    is_ascii_number, is_calendar_word, is_month_abbreviation, is_unit, is_word_number,
};

pub(crate) fn boundary_context(previous: Option<&str>, next: Option<&str>) -> bool {
    previous.is_some_and(is_temporal_neighbor) || next.is_some_and(is_temporal_neighbor)
}

pub(crate) fn recurrence_context(word: &str, next: Option<&str>) -> bool {
    word != "every" || next.is_some_and(is_temporal_neighbor)
}

pub(crate) fn fuzzy_calendar_context(previous: Option<&str>, next: Option<&str>) -> bool {
    previous.is_some_and(is_calendar_cue) || next.is_some_and(is_ascii_number)
}

pub(crate) fn fuzzy_weekday_context(previous: Option<&str>) -> bool {
    previous.is_some_and(|value| matches!(value, "next" | "last" | "this" | "every" | "on" | "by"))
}

pub(crate) fn fuzzy_unit_context(
    previous: Option<&str>,
    next: Option<&str>,
    previous_adjacent: bool,
) -> bool {
    (previous_adjacent
        && previous.is_some_and(|value| is_ascii_number(value) || is_word_number(value)))
        || next.is_some_and(|value| matches!(value, "ago" | "later" | "earlier"))
}

pub(crate) fn standalone_year_context(
    value: &str,
    previous: Option<&str>,
    next: Option<&str>,
) -> bool {
    let Ok(_year) = value.parse::<i32>() else {
        return false;
    };
    previous.is_some_and(is_calendar_cue)
        || next.is_some_and(|value| matches!(value, "ad" | "bc" | "bce" | "ce"))
}

fn is_temporal_neighbor(value: &str) -> bool {
    is_unit(value)
        || is_calendar_word(value)
        || is_month_abbreviation(value)
        || matches!(value, "today" | "tomorrow" | "yesterday")
}

fn is_calendar_cue(value: &str) -> bool {
    matches!(
        value,
        "in" | "during"
            | "by"
            | "until"
            | "since"
            | "from"
            | "next"
            | "last"
            | "before"
            | "after"
            | "through"
            | "year"
    )
}
