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
        fragment_id: [1; 32],
        source_keys: BTreeSet::from([key("c", "u"), key("c", "a")]),
        evidence_available: true,
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
        "owner",
        SemanticPolicy::default(),
        &timeline(),
        sources,
        targets,
    )
    .unwrap()
}
#[test]
fn historical_semantic_identity_is_owner_domain_fragment_only() {
    let event = historical_semantic_event_id([1; 16], [1; 32]);
    assert_eq!(
        format!(
            "{:x}",
            sha2::Sha256::digest(
                [
                    b"reliquary:historical-semantic-fragment:v1\0".as_slice(),
                    &[1; 16],
                    &[1; 32]
                ]
                .concat()
            )
        ),
        event.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    assert_eq!(
        event.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        "758faa5a61bdab3516a19b133b3be7af74c5e4786854abc1524a68455839e61f"
    );
    assert_ne!(event, historical_semantic_event_id([2; 16], [1; 32]));
    assert_ne!(event, historical_semantic_event_id([1; 16], [2; 32]));
    let a = plan(vec![source()], vec![target(1)]);
    let mut changed = source();
    changed.evidence_digest = [8; 32];
    changed.profile.generation = 2;
    let mut t = target(1);
    t.vector = vec![0.8, 0.6];
    let b = plan(vec![changed], vec![t]);
    assert_eq!(a.events[0].event_id, b.events[0].event_id);
    assert_ne!(a.events[0].effects, b.events[0].effects);
    assert_ne!(
        a.events[0].source_evidence_digest,
        b.events[0].source_evidence_digest
    );
}
#[test]
fn historical_semantic_mixed_fragment_is_one_latest_position_event() {
    let result = plan(
        vec![source()],
        vec![
            target(1),
            HistoricalSemanticTarget {
                availability: MemoryAvailability::Supported { position: 2 },
                ..target(2)
            },
        ],
    );
    assert_eq!(result.events.len(), 1);
    assert_eq!(result.events[0].source_position, 2);
    assert_eq!(
        result.events[0].source_keys,
        BTreeSet::from([key("c", "u"), key("c", "a")])
    );
    assert_eq!(result.events[0].effects.len(), 2); // Later context is never replayed at position 1.
    assert!(result.events[0].effects.iter().all(|e| e.bonus == 10));
    let mut overlapping = source();
    overlapping.fragment_id = [2; 32];
    overlapping.vectors[0].fragment_id = [2; 32];
    overlapping.source_keys = BTreeSet::from([key("c", "a")]);
    let two = plan(vec![overlapping.clone(), source()], vec![target(1)]);
    assert_eq!(two.events.len(), 2); // Distinct retained fragment observations.
    assert_ne!(two.events[0].event_id, two.events[1].event_id);
    assert_eq!(two, plan(vec![source(), overlapping], vec![target(1)]));
}
#[test]
fn historical_semantic_exclusions_future_lifecycle_profile_precede_top64() {
    let mut s = source();
    s.credit_excluded = BTreeSet::from([id(1), id(2), id(3), id(7)]);
    let mut targets: Vec<_> = (1..=72).map(target).collect();
    targets[3].availability = MemoryAvailability::Supported { position: 3 };
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
    let mut unavailable = source();
    unavailable.evidence_available = false;
    unavailable.vectors.clear();
    let absent = plan(vec![unavailable], vec![target(1)]);
    let mut empty = source();
    empty.vectors.clear();
    let complete = plan(vec![empty], vec![target(1)]);
    assert_eq!(absent.coverage[&[1; 32]], "fragment-evidence-unavailable");
    assert_eq!(
        complete.coverage[&[1; 32]],
        "complete-no-usable-source-rows"
    );
    assert!(absent.events.is_empty() && complete.events.is_empty());
    let mut bad = source();
    bad.vectors[0].components = vec![f32::NAN, 0.0];
    assert_eq!(
        plan(vec![bad], vec![target(1)]).coverage[&[1; 32]],
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

#[test]
fn historical_semantic_same_fragment_cannot_be_recredited_after_reembedding() {
    let mut regenerated = source();
    regenerated.evidence_digest = [8; 32];
    regenerated.profile.generation = 2;
    assert!(matches!(
        HistoricalSemanticPlan::plan(
            [1; 16],
            "owner",
            SemanticPolicy::default(),
            &timeline(),
            [source(), regenerated],
            [target(1)]
        ),
        Err(HistoricalSemanticError::DuplicateSource(_))
    ));
}
