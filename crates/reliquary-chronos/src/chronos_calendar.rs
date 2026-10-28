use crate::TemporalWeekday;
use time::{Date, Duration, Month, OffsetDateTime, Weekday};

pub(crate) fn date_from_ns(timestamp_ns: i64) -> Option<Date> {
    OffsetDateTime::from_unix_timestamp_nanos(timestamp_ns as i128)
        .ok()
        .map(|value| value.date())
}

pub(crate) fn instant_to_ns(value: OffsetDateTime) -> Option<i64> {
    i64::try_from(value.unix_timestamp_nanos()).ok()
}

pub(crate) fn day_span(date: Date) -> Option<(i64, i64)> {
    bounds(date, date.next_day()?)
}

pub(crate) fn week_span(date: Date) -> Option<(i64, i64)> {
    let offset = weekday_number(date.weekday()) as i64;
    let start = date.checked_sub(Duration::days(offset))?;
    bounds(start, start.checked_add(Duration::days(7))?)
}

pub(crate) fn month_span(date: Date) -> Option<(i64, i64)> {
    let start = Date::from_calendar_date(date.year(), date.month(), 1).ok()?;
    bounds(start, shift_month_start(start, 1)?)
}

pub(crate) fn quarter_span(date: Date) -> Option<(i64, i64)> {
    let month = month_number(date.month());
    let quarter_month = ((month - 1) / 3) * 3 + 1;
    let start =
        Date::from_calendar_date(date.year(), Month::try_from(quarter_month).ok()?, 1).ok()?;
    bounds(start, shift_month_start(start, 3)?)
}

pub(crate) fn year_span(date: Date) -> Option<(i64, i64)> {
    let start = Date::from_calendar_date(date.year(), Month::January, 1).ok()?;
    let end = Date::from_calendar_date(date.year().checked_add(1)?, Month::January, 1).ok()?;
    bounds(start, end)
}

pub(crate) fn shift_days(date: Date, days: i64) -> Option<Date> {
    if days >= 0 {
        date.checked_add(Duration::days(days))
    } else {
        date.checked_sub(Duration::days(days.unsigned_abs() as i64))
    }
}

pub(crate) fn shift_week_start(date: Date, weeks: i64) -> Option<Date> {
    let offset = weekday_number(date.weekday()) as i64;
    let start = date.checked_sub(Duration::days(offset))?;
    shift_days(start, weeks.checked_mul(7)?)
}

pub(crate) fn shift_month_start(date: Date, delta: i32) -> Option<Date> {
    let month_index =
        date.year().checked_mul(12)? + i32::from(month_number(date.month())) - 1 + delta;
    let year = month_index.div_euclid(12);
    let month = u8::try_from(month_index.rem_euclid(12) + 1).ok()?;
    Date::from_calendar_date(year, Month::try_from(month).ok()?, 1).ok()
}

pub(crate) fn shift_months_clamped(date: Date, delta: i32) -> Option<Date> {
    let target = shift_month_start(date, delta)?;
    let next = shift_month_start(target, 1)?;
    let last_day = next.previous_day()?.day();
    Date::from_calendar_date(target.year(), target.month(), date.day().min(last_day)).ok()
}

pub(crate) fn shift_year_start(date: Date, delta: i32) -> Option<Date> {
    Date::from_calendar_date(date.year().checked_add(delta)?, Month::January, 1).ok()
}

pub(crate) fn weekday_date(source: Date, target: TemporalWeekday, forward: bool) -> Option<Date> {
    let current = weekday_number(source.weekday()) as i64;
    let target = temporal_weekday_number(target) as i64;
    let distance = if forward {
        let delta = (target - current).rem_euclid(7);
        if delta == 0 { 7 } else { delta }
    } else {
        let delta = (current - target).rem_euclid(7);
        -(if delta == 0 { 7 } else { delta })
    };
    shift_days(source, distance)
}

pub(crate) fn weekday_on_or_after(source: Date, target: TemporalWeekday) -> Option<Date> {
    let current = weekday_number(source.weekday()) as i64;
    let target = temporal_weekday_number(target) as i64;
    shift_days(source, (target - current).rem_euclid(7))
}

pub(crate) fn weekday_on_or_before(source: Date, target: TemporalWeekday) -> Option<Date> {
    let current = weekday_number(source.weekday()) as i64;
    let target = temporal_weekday_number(target) as i64;
    shift_days(source, -((current - target).rem_euclid(7)))
}

pub(crate) fn month_number(month: Month) -> u8 {
    match month {
        Month::January => 1,
        Month::February => 2,
        Month::March => 3,
        Month::April => 4,
        Month::May => 5,
        Month::June => 6,
        Month::July => 7,
        Month::August => 8,
        Month::September => 9,
        Month::October => 10,
        Month::November => 11,
        Month::December => 12,
    }
}

fn bounds(start: Date, end: Date) -> Option<(i64, i64)> {
    let start = i64::try_from(start.midnight().assume_utc().unix_timestamp_nanos()).ok()?;
    let end = i64::try_from(end.midnight().assume_utc().unix_timestamp_nanos()).ok()?;
    (start < end).then_some((start, end))
}

fn weekday_number(weekday: Weekday) -> u8 {
    match weekday {
        Weekday::Monday => 0,
        Weekday::Tuesday => 1,
        Weekday::Wednesday => 2,
        Weekday::Thursday => 3,
        Weekday::Friday => 4,
        Weekday::Saturday => 5,
        Weekday::Sunday => 6,
    }
}

fn temporal_weekday_number(weekday: TemporalWeekday) -> u8 {
    match weekday {
        TemporalWeekday::Monday => 0,
        TemporalWeekday::Tuesday => 1,
        TemporalWeekday::Wednesday => 2,
        TemporalWeekday::Thursday => 3,
        TemporalWeekday::Friday => 4,
        TemporalWeekday::Saturday => 5,
        TemporalWeekday::Sunday => 6,
    }
}
