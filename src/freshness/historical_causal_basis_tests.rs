use super::*;
fn id(n: u8) -> MemoryId {
    MemoryId([n; 32])
}
fn initial() -> BTreeMap<MemoryId, FreshnessRecord> {
    let mut record = FreshnessRecord::created();
    record.admit(0).unwrap();
    record.score = 0;
    [(id(1), record), (id(2), record)].into_iter().collect()
}
fn event(recipient: u8, position: u64, source: u8, amount: i16) -> HistoricalScoreEvent {
    HistoricalScoreEvent {
        recipient: id(recipient),
        position,
        event_id: [source; 32],
        amount,
    }
}
#[test]
fn ranked_basis_preserves_same_turn_caps_and_shared_source_attribution() {
    let events = [
        event(1, 1, 9, 50),
        event(2, 1, 3, -100),
        event(1, 1, 3, -100),
        event(1, 1, 6, 100),
    ];
    let initial = initial();
    let basis = HistoricalCausalBasis::from_events(&initial, &events, 11).unwrap();
    assert_eq!(
        basis.reconstruct(11).unwrap(),
        score_historical_records(&initial, &events, 11, &FreshnessPolicy::default_v1()).unwrap()
    );
    assert_eq!(basis.contributions().len(), 3);
    assert_eq!(basis.contributions()[0].effects.len(), 2);
    let reversed: Vec<_> = events.iter().cloned().rev().collect();
    assert_eq!(
        basis,
        HistoricalCausalBasis::from_events(&initial, &reversed, 11).unwrap()
    );
    assert_eq!(
        basis,
        HistoricalCausalBasis::from_canonical(
            basis.initial().clone(),
            basis.contributions().to_vec(),
            basis.corrections().clone(),
            11
        )
        .unwrap()
    );
}
#[test]
fn conflicting_source_positions_and_duplicate_recipients_refuse() {
    assert!(
        HistoricalCausalBasis::from_events(
            &initial(),
            &[event(1, 1, 3, 10), event(2, 2, 3, 10)],
            2
        )
        .is_err()
    );
    assert!(
        HistoricalCausalBasis::from_events(
            &initial(),
            &[event(1, 1, 3, 10), event(1, 1, 3, 10)],
            2
        )
        .is_err()
    );
}
#[test]
fn corrections_preserve_exact_accepted_scores_and_unchanged_anchors() {
    let initial = initial();
    let baseline = HistoricalCausalBasis::from_events(&initial, &[event(1, 1, 2, 10)], 11).unwrap();
    let corrected = HistoricalCausalBasis::from_canonical(
        initial,
        baseline.contributions().to_vec(),
        [(id(1), -17)].into_iter().collect(),
        11,
    )
    .unwrap();
    let before = baseline.reconstruct(11).unwrap();
    let after = corrected.reconstruct(11).unwrap();
    assert_eq!(after[&id(1)].score, before[&id(1)].score - 17);
    assert_eq!(
        after[&id(1)].admitted_at_turn,
        before[&id(1)].admitted_at_turn
    );
    assert_eq!(after[&id(1)].accounted_turn, before[&id(1)].accounted_turn);
    assert_eq!(after[&id(2)], before[&id(2)]);
}
#[test]
fn malformed_canonical_basis_and_invalid_scalar_authority_refuse() {
    for (recipient, correction) in [(id(1), 0), (id(3), 1), (id(1), 201), (id(1), -200)] {
        assert!(
            HistoricalCausalBasis::from_canonical(
                initial(),
                vec![],
                [(recipient, correction)].into_iter().collect(),
                11
            )
            .is_err()
        );
    }
    for rows in [
        vec![CausalContribution {
            position: 1,
            effects: BTreeMap::new(),
        }],
        vec![CausalContribution {
            position: 0,
            effects: [(id(1), 1)].into_iter().collect(),
        }],
        vec![CausalContribution {
            position: 12,
            effects: [(id(1), 1)].into_iter().collect(),
        }],
        vec![
            CausalContribution {
                position: 2,
                effects: [(id(1), 1)].into_iter().collect(),
            },
            CausalContribution {
                position: 1,
                effects: [(id(1), 1)].into_iter().collect(),
            },
        ],
        vec![CausalContribution {
            position: 1,
            effects: [(id(3), 1)].into_iter().collect(),
        }],
    ] {
        assert!(
            HistoricalCausalBasis::from_canonical(initial(), rows, BTreeMap::new(), 11).is_err()
        );
    }
    assert!(
        HistoricalCausalBasis::from_events(&BTreeMap::new(), &[], 0)
            .unwrap()
            .reconstruct(0)
            .unwrap()
            .is_empty()
    );
}
