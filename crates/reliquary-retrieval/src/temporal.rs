//! Shared half-open temporal search windows and grounded evidence.
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemporalBasis {
    Source,
    Mentioned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct TemporalFilter {
    pub basis: TemporalBasis,
    pub start_ns: i64,
    pub end_exclusive_ns: i64,
}

impl TemporalFilter {
    pub fn new(basis: TemporalBasis, start_ns: i64, end_exclusive_ns: i64) -> Option<Self> {
        (start_ns < end_exclusive_ns).then_some(Self {
            basis,
            start_ns,
            end_exclusive_ns,
        })
    }
    /// Parse exact offset-bearing RFC3339 bounds and reject unrepresentable nanoseconds.
    pub fn from_rfc3339(basis: TemporalBasis, from: &str, before: &str) -> Option<Self> {
        use time::{OffsetDateTime, format_description::well_known::Rfc3339};
        let parse = |s: &str| -> Option<i64> {
            let parsed = OffsetDateTime::parse(s, &Rfc3339).ok()?;
            // The parser accepts leap seconds and truncates excess precision; neither
            // can be represented exactly as a Unix nanosecond bound.
            if s.as_bytes().get(17..19) == Some(b"60") {
                return None;
            }
            if let Some((_, fraction)) = s.split_once('.') {
                if fraction
                    .bytes()
                    .take_while(u8::is_ascii_digit)
                    .skip(9)
                    .any(|b| b != b'0')
                {
                    return None;
                }
            }
            i64::try_from(parsed.unix_timestamp_nanos()).ok()
        };
        Self::new(basis, parse(from)?, parse(before)?)
    }
    pub fn contains(&self, instant_ns: i64) -> bool {
        self.start_ns <= instant_ns && instant_ns < self.end_exclusive_ns
    }
    pub fn overlaps(&self, start_ns: i64, end_exclusive_ns: i64) -> bool {
        start_ns < end_exclusive_ns
            && self.start_ns < end_exclusive_ns
            && start_ns < self.end_exclusive_ns
    }
}

impl<'de> Deserialize<'de> for TemporalFilter {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Fields {
            basis: TemporalBasis,
            start_ns: i64,
            end_exclusive_ns: i64,
        }
        let fields = Fields::deserialize(deserializer)?;
        Self::new(fields.basis, fields.start_ns, fields.end_exclusive_ns)
            .ok_or_else(|| serde::de::Error::custom("temporal window must be nonempty"))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "basis", rename_all = "snake_case")]
pub enum TemporalEvidence {
    Source {
        source_time_ns: i64,
    },
    Mentioned {
        phrase: String,
        start_ns: i64,
        end_exclusive_ns: i64,
        precision: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offset_bounds_normalize_and_reject_invalid_values() {
        let f = TemporalFilter::from_rfc3339(
            TemporalBasis::Source,
            "2026-11-20T00:00:00-08:00",
            "2026-11-21T00:00:00-08:00",
        )
        .unwrap();
        let utc = TemporalFilter::from_rfc3339(
            TemporalBasis::Source,
            "2026-11-20T08:00:00Z",
            "2026-11-21T08:00:00Z",
        )
        .unwrap();
        assert_eq!(f, utc);
        assert!(
            TemporalFilter::from_rfc3339(
                TemporalBasis::Source,
                "3000-01-01T00:00:00Z",
                "3000-01-02T00:00:00Z"
            )
            .is_none()
        );
        assert!(
            TemporalFilter::from_rfc3339(
                TemporalBasis::Source,
                "2026-11-21T00:00:00Z",
                "2026-11-20T00:00:00Z"
            )
            .is_none()
        );
    }
    #[test]
    fn vancouver_fall_back_day_and_exact_precision() {
        let filter = TemporalFilter::from_rfc3339(
            TemporalBasis::Source,
            "2026-11-01T00:00:00-07:00",
            "2026-11-02T00:00:00-08:00",
        )
        .unwrap();
        assert_eq!(
            filter.end_exclusive_ns - filter.start_ns,
            25 * 3_600_000_000_000
        );
        let utc = TemporalFilter::from_rfc3339(
            TemporalBasis::Source,
            "2026-11-01T07:00:00Z",
            "2026-11-02T08:00:00Z",
        )
        .unwrap();
        assert_eq!(filter, utc);
        assert!(filter.contains(filter.start_ns));
        assert!(filter.contains(filter.end_exclusive_ns - 1));
        assert!(!filter.contains(filter.end_exclusive_ns));
        for invalid in ["2026-11-01T00:00:00.0000000001Z", "2016-12-31T23:59:60Z"] {
            assert!(
                TemporalFilter::from_rfc3339(
                    TemporalBasis::Source,
                    invalid,
                    "2027-01-01T00:00:00Z"
                )
                .is_none()
            );
            assert!(
                TemporalFilter::from_rfc3339(
                    TemporalBasis::Source,
                    "2016-01-01T00:00:00Z",
                    invalid
                )
                .is_none()
            );
        }
        assert!(
            TemporalFilter::from_rfc3339(
                TemporalBasis::Source,
                "2026-11-01T00:00:00.0000000000Z",
                "2026-11-02T00:00:00Z"
            )
            .is_some()
        );
    }

    #[test]
    fn half_open_windows() {
        let f = TemporalFilter::new(TemporalBasis::Source, -10, 10).unwrap();
        assert!(!f.contains(-11));
        assert!(f.contains(-10));
        assert!(f.contains(9));
        assert!(!f.contains(10));
        assert!(!f.overlaps(-20, -10));
        assert!(f.overlaps(9, 20));
        assert!(!f.overlaps(10, 20));
        assert!(TemporalFilter::new(TemporalBasis::Source, 1, 1).is_none());
    }
}
