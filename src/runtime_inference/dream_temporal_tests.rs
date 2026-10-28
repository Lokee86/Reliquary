use crate::chronos::analyze as parse_temporal;
use crate::dream_candidate_test_support::{
    install_vectors, memory, memory_with_created_at, memory_with_source_time, test_path,
};
use crate::{
    Cva, DreamCandidateConfig, DreamTemporalFrequency, DreamTemporalGranularity,
    DreamTemporalMatchKind, DreamTemporalOrigin, DreamTemporalWeekday,
};
use time::{Date, Month};

#[test]
fn deterministic_parser_extracts_absolute_dates_ranges_quarters_and_years() {
    let analysis = parse_temporal(
        "Deadline 2026-09-15. Phase runs 2026-10-01 to 2026-10-03. Review Q4 2026 during 2027.",
        None,
    );
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == DreamTemporalGranularity::Day
            && anchor.start_ns == day_start_ns(2026, Month::September, 15)
    }));
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.granularity == DreamTemporalGranularity::Range
            && anchor.start_ns == day_start_ns(2026, Month::October, 1)
            && anchor.end_ns == day_start_ns(2026, Month::October, 4)
    }));
    assert!(
        analysis
            .anchors
            .iter()
            .any(|anchor| anchor.granularity == DreamTemporalGranularity::Quarter)
    );
    assert!(
        analysis
            .anchors
            .iter()
            .any(|anchor| anchor.granularity == DreamTemporalGranularity::Year)
    );
    assert!(
        analysis
            .anchors
            .iter()
            .all(|anchor| anchor.origin == DreamTemporalOrigin::Explicit)
    );
}

#[test]
fn relative_dates_resolve_against_source_turn_time_not_memory_bookkeeping() {
    let source_time = timestamp_ns(2026, Month::August, 24, 12);
    let mut cva = Cva::create(test_path("relative-source-time.cva")).unwrap();
    let early_created = memory_with_created_at(
        &mut cva,
        "early-created",
        "A",
        "Inspection is tomorrow and the crew met last week.",
        source_time,
        false,
        1,
    );
    let late_created = memory_with_created_at(
        &mut cva,
        "late-created",
        "B",
        "Inspection is tomorrow and the crew met last week.",
        source_time,
        false,
        9_999_999,
    );
    let left = cva.dream_temporal_analysis(early_created).unwrap();
    let right = cva.dream_temporal_analysis(late_created).unwrap();
    assert_eq!(left.source_timestamp_ns, Some(source_time));
    assert_eq!(left.anchors, right.anchors);
    assert!(left.anchors.iter().any(|anchor| {
        anchor.granularity == DreamTemporalGranularity::Day
            && anchor.origin == DreamTemporalOrigin::Relative
            && anchor.start_ns == day_start_ns(2026, Month::August, 25)
    }));
    assert!(left.anchors.iter().any(|anchor| {
        anchor.granularity == DreamTemporalGranularity::Week
            && anchor.start_ns == day_start_ns(2026, Month::August, 17)
            && anchor.end_ns == day_start_ns(2026, Month::August, 24)
    }));
}

#[test]
fn recoverable_archive_provenance_overrides_persisted_source_time_and_creation_bookkeeping() {
    let archive_time = timestamp_ns(2026, Month::August, 10, 12);
    let source_time = timestamp_ns(2026, Month::August, 24, 12);
    let mut cva = Cva::create(test_path("explicit-source-time.cva")).unwrap();
    let id = memory_with_source_time(
        &mut cva,
        "explicit-source-time",
        "Inspection",
        "Inspection is tomorrow.",
        archive_time,
        source_time,
        1,
    );
    let analysis = cva.dream_temporal_analysis(id).unwrap();
    assert_eq!(analysis.source_timestamp_ns, Some(archive_time));
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.origin == DreamTemporalOrigin::Relative
            && anchor.start_ns == day_start_ns(2026, Month::August, 11)
    }));
}

#[test]
fn next_weekday_is_strictly_after_the_source_day() {
    let source_time = timestamp_ns(2026, Month::August, 24, 12);
    let analysis = parse_temporal("Review next Tuesday and last Monday.", Some(source_time));
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.start_ns == day_start_ns(2026, Month::August, 25)
            && anchor.origin == DreamTemporalOrigin::Relative
    }));
    assert!(analysis.anchors.iter().any(|anchor| {
        anchor.start_ns == day_start_ns(2026, Month::August, 17)
            && anchor.origin == DreamTemporalOrigin::Relative
    }));
}

#[test]
fn relative_language_without_source_time_is_not_guessed() {
    let analysis = parse_temporal("Do it tomorrow; hard deadline 2026-08-25.", None);
    assert_eq!(analysis.anchors.len(), 1);
    assert_eq!(analysis.anchors[0].origin, DreamTemporalOrigin::Explicit);
    assert_eq!(
        analysis.anchors[0].start_ns,
        day_start_ns(2026, Month::August, 25)
    );
}

#[test]
fn recurrence_parser_preserves_weekday_and_interval_identity() {
    let analysis = parse_temporal("Crew reports every Monday and audits every 2 weeks.", None);
    assert!(analysis.patterns.iter().any(|pattern| {
        pattern.frequency == DreamTemporalFrequency::Weekly
            && pattern.interval == 1
            && pattern.weekday == Some(DreamTemporalWeekday::Monday)
    }));
    assert!(analysis.patterns.iter().any(|pattern| {
        pattern.frequency == DreamTemporalFrequency::Weekly
            && pattern.interval == 2
            && pattern.weekday.is_none()
    }));
}

#[test]
fn dream_analysis_uses_explicit_recurrence_definition_from_shared_parser() {
    let source_time = timestamp_ns(2026, Month::August, 24, 12);
    let mut cva = Cva::create(test_path("dream-explicit-recurrence-definition.cva")).unwrap();
    let id = memory(
        &mut cva,
        "explicit-recurrence-definition",
        "Temporal test",
        "Review biweekly, meaning every two weeks.",
        source_time,
        false,
    );

    let dream = cva.dream_temporal_analysis(id).unwrap();
    let deterministic = parse_temporal(
        "Temporal test\nReview biweekly, meaning every two weeks.",
        Some(source_time),
    );
    assert_eq!(dream, deterministic);
    assert!(dream.patterns.iter().any(|pattern| {
        pattern.frequency == DreamTemporalFrequency::Weekly
            && pattern.interval == 2
            && pattern.occurrences_per_period == 1
            && pattern.weekday.is_none()
            && pattern.month_day.is_none()
    }));
}

#[test]
fn temporal_lane_recovers_unembedded_candidate_from_relative_and_explicit_dates() {
    let source_time = timestamp_ns(2026, Month::August, 24, 12);
    let mut cva = Cva::create(test_path("temporal-candidate.cva")).unwrap();
    let source = memory(
        &mut cva,
        "source",
        "Upcoming inspection",
        "Inspection is tomorrow.",
        source_time,
        false,
    );
    let match_id = memory(
        &mut cva,
        "match",
        "Field event",
        "Scheduled for 2026-08-25.",
        timestamp_ns(2026, Month::July, 1, 12),
        false,
    );
    let miss = memory(
        &mut cva,
        "miss",
        "Other event",
        "Scheduled for 2026-08-26.",
        timestamp_ns(2026, Month::July, 2, 12),
        false,
    );
    let profile = install_vectors(&mut cva, &[source], &[&[1.0, 0.0]]);
    let result = cva
        .dream_candidates(
            profile,
            source,
            DreamCandidateConfig {
                limit: 2,
                semantic_limit: 0,
                prior_semantic_quota: 0,
                lexical_limit: 0,
                temporal_limit: 2,
            },
        )
        .unwrap();
    assert_eq!(result.candidates.len(), 1);
    let candidate = &result.candidates[0];
    assert_eq!(candidate.context.memory.id, match_id);
    assert_eq!(candidate.semantic_score, None);
    assert_eq!(candidate.temporal_rank, Some(1));
    assert!(candidate.temporal_score > 0.9);
    assert!(
        candidate
            .temporal_matches
            .iter()
            .any(|matched| matched.kind == DreamTemporalMatchKind::ExactDay)
    );
    assert!(
        !result
            .candidates
            .iter()
            .any(|candidate| candidate.context.memory.id == miss)
    );
}

#[test]
fn recurrence_identity_can_supply_a_temporal_candidate() {
    let mut cva = Cva::create(test_path("recurrence-candidate.cva")).unwrap();
    let source = memory(
        &mut cva,
        "source",
        "Crew report",
        "Crew report happens every Monday.",
        timestamp_ns(2026, Month::August, 24, 12),
        false,
    );
    let candidate = memory(
        &mut cva,
        "candidate",
        "Safety sync",
        "Safety sync happens Mondays.",
        timestamp_ns(2026, Month::August, 10, 12),
        false,
    );
    let profile = install_vectors(&mut cva, &[source], &[&[1.0, 0.0]]);
    let result = cva
        .dream_candidates(
            profile,
            source,
            DreamCandidateConfig {
                limit: 1,
                semantic_limit: 0,
                prior_semantic_quota: 0,
                lexical_limit: 0,
                temporal_limit: 1,
            },
        )
        .unwrap();
    assert_eq!(result.candidates[0].context.memory.id, candidate);
    assert!(
        result.candidates[0]
            .temporal_matches
            .iter()
            .any(|matched| matched.kind == DreamTemporalMatchKind::Recurrence)
    );
}

#[test]
fn recurrence_specific_dates_do_not_collapse_to_generic_cadence() {
    let monthly_15 = parse_temporal("Inspection every month on the 15th.", None);
    let monthly_20 = parse_temporal("Inspection every month on the 20th.", None);
    assert_eq!(monthly_15.patterns.len(), 1);
    assert_eq!(monthly_20.patterns.len(), 1);
    assert_eq!(monthly_15.patterns[0].month_day, Some(15));
    assert_eq!(monthly_20.patterns[0].month_day, Some(20));
    assert_ne!(monthly_15.patterns[0], monthly_20.patterns[0]);

    let yearly = parse_temporal("Renew every year on September 15th.", None);
    assert_eq!(yearly.patterns.len(), 1);
    assert_eq!(yearly.patterns[0].frequency, DreamTemporalFrequency::Yearly);
    assert_eq!(yearly.patterns[0].month, Some(9));
    assert_eq!(yearly.patterns[0].month_day, Some(15));
}

fn timestamp_ns(year: i32, month: Month, day: u8, hour: u8) -> i64 {
    let date = Date::from_calendar_date(year, month, day).unwrap();
    let value = date.with_hms(hour, 0, 0).unwrap().assume_utc();
    i64::try_from(value.unix_timestamp_nanos()).unwrap()
}

fn day_start_ns(year: i32, month: Month, day: u8) -> i64 {
    let date = Date::from_calendar_date(year, month, day).unwrap();
    i64::try_from(date.midnight().assume_utc().unix_timestamp_nanos()).unwrap()
}
