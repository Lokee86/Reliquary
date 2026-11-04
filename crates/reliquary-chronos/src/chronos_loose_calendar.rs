use crate::TemporalAnchor;
use crate::chronos_parser::TextSpan;
use time::Month;

pub(crate) const MONTH_RE: &str =
    "january|february|march|april|may|june|july|august|september|october|november|december";
pub(crate) const MONTH_ABBR_RE: &str = "jan|feb|mar|apr|jun|jul|aug|sep|sept|oct|nov|dec";

pub(crate) fn extract_explicit(
    text: &str,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    crate::chronos_loose_explicit::extract(text, claimed_spans, anchors);
}

pub(crate) fn extract_relative(
    text: &str,
    reference_timestamp_ns: Option<i64>,
    claimed_spans: &mut Vec<TextSpan>,
    anchors: &mut Vec<TemporalAnchor>,
) {
    crate::chronos_loose_relative::extract(text, reference_timestamp_ns, claimed_spans, anchors);
}

pub(crate) fn parse_abbreviation(value: &str) -> Option<Month> {
    match value.trim_end_matches('.').to_ascii_lowercase().as_str() {
        "jan" => Some(Month::January),
        "feb" => Some(Month::February),
        "mar" => Some(Month::March),
        "apr" => Some(Month::April),
        "jun" => Some(Month::June),
        "jul" => Some(Month::July),
        "aug" => Some(Month::August),
        "sep" | "sept" => Some(Month::September),
        "oct" => Some(Month::October),
        "nov" => Some(Month::November),
        "dec" => Some(Month::December),
        _ => None,
    }
}
