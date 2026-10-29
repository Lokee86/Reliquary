use reliquary_dream::{DuplicateIndex, DuplicateTemporalKey};
use reliquary_graph::{GraphRelation, GraphRelationKind, GraphRelationOrigin};
use std::collections::HashMap;

#[test]
fn duplicate_union_rewires_middle_insertion() {
    let a = [1; 32];
    let b = [2; 32];
    let c = [3; 32];
    let relation = GraphRelation {
        source: c,
        target: a,
        kind: GraphRelationKind::DuplicateOf,
        active: true,
        origin: GraphRelationOrigin::Dream,
        global_version: 1,
        graph_version: 1,
    };
    let keys = HashMap::from([
        (a, DuplicateTemporalKey::new(10, a)),
        (c, DuplicateTemporalKey::new(30, c)),
    ]);
    let mut index = DuplicateIndex::rebuild(1, &[relation], &keys);
    let plan = index.plan_union(
        a,
        DuplicateTemporalKey::new(10, a),
        b,
        DuplicateTemporalKey::new(20, b),
        &[relation],
    );
    assert_eq!(plan.len(), 3);
    assert!(
        plan.iter()
            .any(|r| r.source == c && r.target == a && !r.active)
    );
    assert!(
        plan.iter()
            .any(|r| r.source == b && r.target == a && r.active)
    );
    assert!(
        plan.iter()
            .any(|r| r.source == c && r.target == b && r.active)
    );
}
