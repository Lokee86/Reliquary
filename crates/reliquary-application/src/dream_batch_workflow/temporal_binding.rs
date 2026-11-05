use super::*;
#[test]
fn stored_inference_requires_exact_body_clock_contract_and_source_span() {
    use reliquary_owner_session::model::{
        MemoryBody, MemoryTemporalInference, TemporalIndicationKind, TemporalInference,
        TemporalInferenceResolution, memory_body_id,
    };
    let (path, owner, source, _) = fixture(false);
    let session = PhySession::open_path(&path.0, &graph(owner), []).unwrap();
    let mut record = session.current_memory(source.id).unwrap().record;
    let body = MemoryBody {
        title: "Review".into(),
        content: "Meet biweekly.".into(),
    };
    record.body_id = memory_body_id(&body.title, &body.content);
    let text = format!("{}\n{}", body.title, body.content);
    let start = text.find("biweekly").unwrap();
    record.temporal_inference = Some(MemoryTemporalInference {
        body_id: record.body_id,
        source_time_ns: record.source_time_ns,
        inference: TemporalInference {
            model: "verified-memory".into(),
            contract_version: reliquary_chronos::CHRONOS_INFERENCE_CONTRACT_VERSION.into(),
            resolutions: vec![TemporalInferenceResolution {
                start_byte: start,
                end_byte: start + 8,
                kind: TemporalIndicationKind::Recurrence,
                evidence: "biweekly".into(),
                canonical_expression: "every two weeks".into(),
            }],
        },
    });
    assert_eq!(
        crate::dream_temporal::analyze(&record, &body)
            .unwrap()
            .patterns[0]
            .interval,
        2
    );
    for variant in 0..5 {
        let mut bad = record.clone();
        let proof = bad.temporal_inference.as_mut().unwrap();
        match variant {
            0 => proof.body_id.0 = [99; 32],
            1 => proof.source_time_ns = Some(999),
            2 => proof.inference.contract_version = "future".into(),
            3 => proof.inference.resolutions[0].start_byte += 1,
            _ => proof.inference.resolutions[0].canonical_expression = "biweekly".into(),
        };
        assert!(
            crate::dream_temporal::analyze(&bad, &body).is_err(),
            "variant {variant}"
        );
    }
    let mut wrong = body;
    wrong.content.push_str(" changed");
    assert!(crate::dream_temporal::analyze(&record, &wrong).is_err());
}
