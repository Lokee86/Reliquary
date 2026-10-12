use super::*;
use crate::freshness::historical_planner::HistoricalSource;
fn key(c: &str, n: &str) -> SourceKey {
    SourceKey {
        conversation_id: c.into(),
        node_id: n.into(),
    }
}
fn id(n: u8) -> MemoryId {
    MemoryId([n; 32])
}
fn profile() -> SemanticVectorProfile {
    SemanticVectorProfile {
        profile_id: [9; 32],
        dimensions: 2,
        generation: 1,
    }
}
fn timeline() -> HistoricalTimeline {
    HistoricalTimeline::plan(vec![
        HistoricalSource {
            key: key("c", "u"),
            parent_id: None,
            role: "user".into(),
            timestamp_ns: 1,
        },
        HistoricalSource {
            key: key("c", "a"),
            parent_id: Some("u".into()),
            role: "assistant".into(),
            timestamp_ns: 2,
        },
    ])
    .unwrap()
}
fn source() -> HistoricalSemanticSource {
    HistoricalSemanticSource {
        key: key("c", "u"),
        owner_id: "owner".into(),
        profile: profile(),
        vectors: vec![HistoricalSemanticSourceVector {
            fragment_id: [1; 32],
            components: vec![1.0, 0.0],
        }],
        evidence_digest: [1; 32],
        credit_excluded: BTreeSet::new(),
    }
}
fn target(n: u8) -> HistoricalSemanticTarget {
    HistoricalSemanticTarget {
        memory_id: id(n),
        owner_id: "owner".into(),
        availability: MemoryAvailability::Supported { position: 1 },
        lifecycle_eligible: true,
        profile: profile(),
        vector: vec![1.0, 0.0],
        vector_binding_id: [n; 32],
        evidence_digest: [n; 32],
    }
}
fn plan(
    sources: Vec<HistoricalSemanticSource>,
    targets: Vec<HistoricalSemanticTarget>,
) -> HistoricalSemanticPlan {
    HistoricalSemanticPlan::plan(
        [1; 16],
        [2; 32],
        "owner",
        SemanticPolicy::default(),
        &timeline(),
        sources,
        targets,
    )
    .unwrap()
}
#[test]
fn historical_semantic_identity_matches_independent_h1_golden() {
    let actual: String = historical_semantic_event_id([1; 16], [2; 32], &key("c", "u"))
        .unwrap()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        actual,
        "113d383b96941b8b283477ef073a20eb6167e25c044b5221bba9607872cdf2de"
    );
    assert_ne!(
        historical_semantic_event_id([1; 16], [2; 32], &key("c", "u")).unwrap(),
        historical_semantic_event_id([1; 16], [2; 32], &key("c", "U")).unwrap()
    );
    assert_ne!(
        historical_semantic_event_id([1; 16], [2; 32], &key("c\0", "u")).unwrap(),
        historical_semantic_event_id([1; 16], [2; 32], &key("c", "\0u")).unwrap()
    );
}
#[test]
fn historical_semantic_changed_evidence_and_amount_keep_source_identity() {
    let a = plan(vec![source()], vec![target(1)]);
    let mut s = source();
    s.evidence_digest = [7; 32];
    let mut t = target(1);
    t.vector = vec![0.8, 0.6];
    t.evidence_digest = [8; 32];
    let b = plan(vec![s], vec![t]);
    assert_eq!(a.events[0].event_id, b.events[0].event_id);
    assert_ne!(a.events[0].effects, b.events[0].effects);
    assert_ne!(
        a.events[0].source_evidence_digest,
        b.events[0].source_evidence_digest
    );
    assert_ne!(
        a.events[0].event_id,
        historical_semantic_event_id([1; 16], [3; 32], &key("c", "u")).unwrap()
    );
    assert_ne!(
        a.events[0].event_id,
        historical_semantic_event_id([3; 16], [2; 32], &key("c", "u")).unwrap()
    );
}
#[test]
fn historical_semantic_one_source_many_fragments_reduces_to_one_max_award() {
    let mut s = source();
    s.vectors.push(HistoricalSemanticSourceVector {
        fragment_id: [2; 32],
        components: vec![0.8, 0.6],
    });
    let result = plan(vec![s], vec![target(1)]);
    assert_eq!(result.events.len(), 1);
    assert_eq!(result.events[0].effects.len(), 1);
    assert_eq!(result.events[0].effects[0].bonus, 10);
}
#[test]
fn historical_semantic_exclusions_future_lifecycle_profile_precede_top64() {
    let mut s = source();
    s.credit_excluded = BTreeSet::from([id(1), id(2), id(3), id(7)]);
    let mut targets: Vec<_> = (1..=72).map(target).collect();
    targets[3].availability = MemoryAvailability::Supported { position: 2 };
    targets[4].lifecycle_eligible = false;
    targets[5].profile.profile_id = [8; 32];
    let result = plan(vec![s], targets);
    let roots: BTreeSet<_> = result.events[0]
        .effects
        .iter()
        .map(|e| e.memory_id)
        .collect();
    assert_eq!(roots, (8..=71).map(id).collect());
    assert_eq!(
        result.events[0].credit_excluded,
        BTreeSet::from([id(1), id(2), id(3), id(7)])
    );
}
#[test]
fn historical_semantic_unavailable_is_distinct_from_completed_unusable() {
    let absent = plan(vec![], vec![target(1)]);
    let mut empty = source();
    empty.vectors.clear();
    let complete = plan(vec![empty], vec![target(1)]);
    assert_eq!(
        absent.coverage[&key("c", "u")],
        "source-evidence-unavailable"
    );
    assert_eq!(
        complete.coverage[&key("c", "u")],
        "complete-no-usable-source-rows"
    );
    assert!(absent.events.is_empty() && complete.events.is_empty());
    let mut bad = source();
    bad.vectors[0].components = vec![f32::NAN, 0.0];
    assert_eq!(
        plan(vec![bad], vec![target(1)]).coverage[&key("c", "u")],
        "complete-no-usable-source-rows"
    );
}
#[test]
fn historical_semantic_rejects_foreign_duplicate_and_changed_policy() {
    let mut foreign = target(1);
    foreign.owner_id = "foreign".into();
    assert!(
        HistoricalSemanticPlan::plan(
            [1; 16],
            [2; 32],
            "owner",
            SemanticPolicy::default(),
            &timeline(),
            [source()],
            [foreign]
        )
        .is_err()
    );
    assert!(
        HistoricalSemanticPlan::plan(
            [1; 16],
            [2; 32],
            "owner",
            SemanticPolicy::default(),
            &timeline(),
            [source()],
            [target(1), target(1)]
        )
        .is_err()
    );
    let mut duplicate = source();
    duplicate.vectors.push(duplicate.vectors[0].clone());
    assert!(
        HistoricalSemanticPlan::plan(
            [1; 16],
            [2; 32],
            "owner",
            SemanticPolicy::default(),
            &timeline(),
            [duplicate],
            [target(1)]
        )
        .is_err()
    );
    assert!(matches!(
        HistoricalSemanticPlan::plan(
            [1; 16],
            [2; 32],
            "owner",
            SemanticPolicy {
                candidate_limit: 1,
                ..SemanticPolicy::default()
            },
            &timeline(),
            [source()],
            [target(1)]
        ),
        Err(HistoricalSemanticError::UnsupportedPolicy)
    ));
}
