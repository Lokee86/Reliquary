use super::*;
use crate::historical_propagation::HistoricalPropagationKind;

#[test]
fn scores_older_recipients_and_preserves_future_and_excluded_memory() {
    let [root, future, excluded, older] = [1, 2, 3, 4].map(|id| MemoryId([id; 32]));
    let mut record = FreshnessRecord::created();
    record.admit(1).unwrap();
    record.score = 0;
    let initial = [root, future, excluded, older]
        .into_iter()
        .map(|id| (id, record))
        .collect();
    let adjacency = [
        (root, [future].into_iter().collect()),
        (future, [excluded].into_iter().collect()),
        (excluded, [older].into_iter().collect()),
    ]
    .into_iter()
    .collect();
    let memberships = [root, future, excluded, older]
        .into_iter()
        .map(|id| (id, reliquary_graph::CommunityId([1; 32])))
        .collect();
    let authorized = [root, future, excluded, older].into_iter().collect();
    let availability = [(root, 1), (future, 11), (excluded, 1), (older, 1)]
        .into_iter()
        .map(|(id, position)| (id, MemoryAvailability::Supported { position }))
        .collect();
    let graph = || HistoricalImportGraph {
        adjacency: &adjacency,
        memberships: &memberships,
        authorized: &authorized,
        availability: &availability,
    };
    let policy = FreshnessPolicy::default_v1();
    let event = (
        [5; 32],
        HistoricalPropagationEvent {
            kind: HistoricalPropagationKind::FirstCycleDream,
            position: 10,
            roots: [(root, policy.linkage_principal)].into_iter().collect(),
            credit_excluded: [excluded].into_iter().collect(),
        },
    );
    let retained = [HistoricalScoreEvent {
        recipient: older,
        position: 10,
        event_id: [6; 32],
        amount: 10,
    }];
    let result =
        score_historical_import(&initial, &[event.clone()], &retained, graph(), 10, &policy)
            .unwrap();
    assert_eq!(result[&root].score, 50);
    assert_eq!(result[&older].score, 45);
    assert_eq!(result[&excluded].score, result[&future].score);
    assert!(result[&future].score <= 0);
    assert!(
        score_historical_import(
            &initial,
            &[event.clone(), event],
            &retained,
            graph(),
            10,
            &policy
        )
        .is_err()
    );
}
