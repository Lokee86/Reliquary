use crate::*;
fn inference() -> TemporalInference {
    TemporalInference {
        model: "verified-fixture".into(),
        contract_version: CHRONOS_INFERENCE_CONTRACT_VERSION.into(),
        resolutions: vec![TemporalInferenceResolution {
            start_byte: 7,
            end_byte: 15,
            kind: TemporalIndicationKind::Recurrence,
            evidence: "biweekly".into(),
            canonical_expression: "every two weeks".into(),
        }],
    }
}
#[test]
fn persisted_resolution_preserves_source_evidence_and_rejects_fabricated_or_unresolved_inputs() {
    let source = "Review biweekly.";
    let valid = inference();
    let result = analyze_with_inference(source, None, &valid).unwrap();
    assert_eq!(result.patterns.len(), 1);
    assert_eq!(result.patterns[0].interval, 2);
    assert_eq!(result.patterns[0].evidence, "biweekly");
    for variant in 0..7 {
        let mut bad = valid.clone();
        match variant {
            0 => bad.contract_version = "future-contract".into(),
            1 => bad.resolutions[0].start_byte = 8,
            2 => bad.resolutions[0].evidence = "made up".into(),
            3 => bad.resolutions[0].canonical_expression = "biweekly".into(),
            4 => bad.resolutions.push(bad.resolutions[0].clone()),
            5 => bad.resolutions[0].kind = TemporalIndicationKind::Duration,
            _ => bad.model.clear(),
        };
        assert!(
            analyze_with_inference(source, None, &bad).is_err(),
            "variant {variant}"
        );
    }
}
#[test]
fn persisted_relative_inference_requires_authoritative_reference_time() {
    let mut proof = inference();
    proof.resolutions[0].kind = TemporalIndicationKind::Relative;
    proof.resolutions[0].start_byte = 7;
    proof.resolutions[0].end_byte = 15;
    proof.resolutions[0].canonical_expression = "tomorrow".into();
    assert!(analyze_with_inference("Review biweekly.", None, &proof).is_err());
}
