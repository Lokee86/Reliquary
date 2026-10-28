use crate::TemporalIndicationKind;
use crate::chronos::detect;

#[test]
fn detector_rejects_common_non_temporal_boundary_words() {
    let detection = detect(
        "Route through the audioflow seam before the other changes, then continue after auditing.",
    );
    assert!(!detection.indications.iter().any(|indication| {
        matches!(indication.evidence.as_str(), "through" | "before" | "after")
    }));
}

#[test]
fn detector_keeps_boundary_words_with_temporal_endpoints() {
    let detection = detect("Run until Friday, after 2026-10-01, and starting next week.");
    assert!(
        detection
            .indications
            .iter()
            .any(|value| value.evidence == "until")
    );
    assert!(
        detection
            .indications
            .iter()
            .any(|value| value.evidence == "after")
    );
    assert!(
        detection
            .indications
            .iter()
            .any(|value| value.evidence == "next")
    );
}

#[test]
fn fuzzy_normalization_does_not_rewrite_common_words() {
    let detection = detect(
        "Match rules should be established early; guided tours are available; Frida Kahlo painted; 3. Weeds with seeds spread.",
    );
    assert!(!detection.has_corrections());
    assert!(!detection.indications.iter().any(|value| {
        matches!(
            value.normalized.as_deref(),
            Some("march" | "yearly" | "friday" | "hours")
        )
    }));
}

#[test]
fn ambiguous_calendar_words_require_calendar_context() {
    let detection = detect(
        "Emily May Photography adjusted spring rates while August Comte and Jan van Eyck were discussed.",
    );
    assert!(!detection.indications.iter().any(|value| {
        value.kind == TemporalIndicationKind::Calendar
            && matches!(value.evidence.as_str(), "May" | "spring" | "August" | "Jan")
    }));

    let contextual = detect("Travel in May, return March 2, and work during spring.");
    assert!(
        contextual
            .indications
            .iter()
            .any(|value| value.evidence == "May")
    );
    assert!(
        contextual
            .indications
            .iter()
            .any(|value| value.evidence == "March")
    );
    assert!(
        contextual
            .indications
            .iter()
            .any(|value| value.evidence == "spring")
    );
}

#[test]
fn detector_rejects_deictic_non_temporal_boundaries() {
    let detection =
        detect("Through this training we iterate through this list after this incident.");
    assert!(
        !detection
            .indications
            .iter()
            .any(|value| { value.kind == TemporalIndicationKind::Boundary })
    );
}

#[test]
fn detector_rejects_uncontextualized_four_digit_dimensions() {
    let detection = detect("Viewport dimensions are 1280 by 720 and world height is 9200.0.");
    assert!(!detection.indications.iter().any(|value| {
        value.kind == TemporalIndicationKind::Explicit
            && matches!(value.evidence.as_str(), "1280" | "9200")
    }));
    assert!(detect("During 1800 the archive changed.").has_indications());
}

#[test]
fn duration_detection_does_not_treat_ordinal_second_as_elapsed_time() {
    let detection = detect("During the second game, the player sprite remained hidden.");
    assert!(!detection.indications.iter().any(|value| {
        value.kind == TemporalIndicationKind::Duration && value.evidence == "second"
    }));
}
