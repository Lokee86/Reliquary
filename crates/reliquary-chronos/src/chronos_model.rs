#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemporalGranularity {
    Instant,
    Day,
    Week,
    Month,
    Quarter,
    Season,
    Year,
    Range,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemporalOrigin {
    Explicit,
    Relative,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemporalClockPrecision {
    Minute,
    Second,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemporalDurationUnit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemporalDurationApproximation {
    About,
    Around,
    Approximately,
    Roughly,
    Couple,
    Few,
    Several,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemporalEventDirection {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemporalFrequency {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Yearly,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemporalWeekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalAnchor {
    pub start_ns: i64,
    pub end_ns: i64,
    pub granularity: TemporalGranularity,
    pub origin: TemporalOrigin,
    pub evidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalPattern {
    pub frequency: TemporalFrequency,
    pub interval: u16,
    /// Number of occurrences in each recurrence period.
    pub occurrences_per_period: u16,
    pub weekday: Option<TemporalWeekday>,
    pub month_day: Option<u8>,
    pub month: Option<u8>,
    pub evidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalTimeOfDay {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub precision: TemporalClockPrecision,
    pub origin: TemporalOrigin,
    pub evidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalDuration {
    pub amount: u16,
    pub unit: TemporalDurationUnit,
    pub evidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalDurationRange {
    pub min_amount: u16,
    pub max_amount: u16,
    pub unit: TemporalDurationUnit,
    pub evidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalApproximateDuration {
    pub amount: Option<u16>,
    pub approximation: TemporalDurationApproximation,
    pub unit: TemporalDurationUnit,
    pub evidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalEventRelation {
    pub amount: u16,
    pub unit: TemporalDurationUnit,
    pub direction: TemporalEventDirection,
    pub reference_event: String,
    pub evidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalInterval {
    pub start_ns: Option<i64>,
    pub end_ns: Option<i64>,
    pub start_granularity: Option<TemporalGranularity>,
    pub end_granularity: Option<TemporalGranularity>,
    pub origin: TemporalOrigin,
    pub evidence: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TemporalAnalysis {
    pub source_timestamp_ns: Option<i64>,
    pub anchors: Vec<TemporalAnchor>,
    pub times_of_day: Vec<TemporalTimeOfDay>,
    pub durations: Vec<TemporalDuration>,
    pub duration_ranges: Vec<TemporalDurationRange>,
    pub approximate_durations: Vec<TemporalApproximateDuration>,
    pub event_relations: Vec<TemporalEventRelation>,
    pub intervals: Vec<TemporalInterval>,
    pub patterns: Vec<TemporalPattern>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemporalMatchKind {
    ExactInstant,
    ExactDay,
    ExactWeek,
    ExactMonth,
    ExactQuarter,
    ExactSeason,
    ExactYear,
    RangeOverlap,
    Recurrence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalMatch {
    pub kind: TemporalMatchKind,
    pub source_anchor: Option<TemporalAnchor>,
    pub candidate_anchor: Option<TemporalAnchor>,
    pub source_pattern: Option<TemporalPattern>,
    pub candidate_pattern: Option<TemporalPattern>,
}
