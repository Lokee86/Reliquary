use reliquary_history::{self, HistoricalPayload as HistoryPayload};
use reliquary_memory::owner_lifecycle::{HistoricalPayload, PayloadUnavailable};

// This external consumer proves the old product path is the exact History type.
fn accept_root_payload(
    value: HistoricalPayload<&'static str, u64>,
) -> HistoryPayload<&'static str, u64> {
    value
}

#[test]
fn root_path_reexports_history_type_and_preserves_all_variants() {
    let active: HistoryPayload<_, u64> =
        reliquary_history::evaluate(Ok::<_, ()>("active"), |_| None, || Ok(false), || Ok(0))
            .unwrap();
    let tombstoned: HistoryPayload<_, u64> =
        reliquary_history::evaluate(Ok::<_, ()>("deleted"), |_| None, || Ok(true), || Ok(0))
            .unwrap();
    let pruned: HistoricalPayload<&'static str, u64> = reliquary_history::evaluate(
        Err::<&str, _>("pruned"),
        |_| Some(PayloadUnavailable::Pruned),
        || panic!("pruned reads skip lifecycle"),
        || Ok(9),
    )
    .unwrap();

    let values = [
        accept_root_payload(active),
        accept_root_payload(tombstoned),
        accept_root_payload(pruned),
    ];
    let mut seen = [false; 3];
    for value in values {
        match value {
            HistoricalPayload::Available(value) => seen[0] = value == "active",
            HistoricalPayload::Tombstoned(value) => seen[1] = value == "deleted",
            HistoricalPayload::Pruned {
                structural_history,
                reason: PayloadUnavailable::Pruned,
            } => seen[2] = structural_history == 9,
        }
    }
    assert_eq!(seen, [true, true, true]);
}
