use reliquary_history::{HistoricalPayload, PayloadUnavailable, evaluate};

#[test]
fn external_consumer_uses_public_result_and_lazy_evaluator() {
    let view: HistoricalPayload<&str, u64> = evaluate(
        Ok::<_, &'static str>("record"),
        |_| None,
        || Ok(false),
        || panic!("retained structure is lazy"),
    )
    .unwrap();
    assert_eq!(view, HistoricalPayload::Available("record"));

    let pruned: HistoricalPayload<(), u64> = evaluate(
        Err::<(), _>("body unavailable"),
        |error| (*error == "body unavailable").then_some(PayloadUnavailable::Pruned),
        || panic!("lifecycle is lazy"),
        || Ok(4),
    )
    .unwrap();
    assert_eq!(
        pruned,
        HistoricalPayload::Pruned {
            structural_history: 4,
            reason: PayloadUnavailable::Pruned,
        }
    );
}
