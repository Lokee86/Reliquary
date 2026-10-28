use super::*;
fn id(n: u8) -> MemoryId {
    MemoryId([n; 32])
}
fn member(n: u8, birth: u64) -> HistoricalDreamMemory {
    HistoricalDreamMemory {
        id: id(n),
        cohort: format!("c{n}"),
        birth_position: Some(birth),
        credit_eligible: true,
    }
}
fn link(a: u8, b: u8) -> HistoricalDreamLink {
    HistoricalDreamLink {
        source: id(a),
        target: id(b),
        kind: GraphRelationKind::Topical,
        origin: GraphRelationOrigin::Dream,
        completed: true,
        direct: true,
    }
}
fn proof(roots: &[u8], duplicate_origin: bool) -> HistoricalFirstCycleProof {
    HistoricalFirstCycleProof::Completed {
        roots: roots.iter().map(|&n| id(n)).collect(),
        duplicate_origin,
        evidence_digest: [7; 32],
    }
}
fn plan(
    m: &[HistoricalDreamMemory],
    p: &BTreeMap<MemoryId, HistoricalFirstCycleProof>,
    l: &[HistoricalDreamLink],
    r: &BTreeMap<MemoryId, MemoryId>,
) -> HistoricalDreamPlan {
    plan_historical_dream_events([1; 16], [2; 32], m, p, l, r).unwrap()
}

#[test]
fn historical_dream_one_event_many_roots_strictly_older_and_no_use() {
    let m = vec![
        member(1, 10),
        member(2, 1),
        member(3, 2),
        member(4, 10),
        member(5, 11),
    ];
    let out = plan(
        &m,
        &BTreeMap::new(),
        &[link(1, 2), link(1, 3), link(1, 2), link(1, 4), link(1, 5)],
        &BTreeMap::new(),
    );
    let events: Vec<_> = out.events.iter().filter(|e| e.source_id == id(1)).collect();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].propagation.roots,
        [(id(2), 50), (id(3), 50)].into_iter().collect()
    );
    assert!(
        out.events
            .iter()
            .all(|e| e.propagation.roots.values().all(|&v| v == 50))
    );
}
#[test]
fn historical_dream_actual_proof_and_no_link_precede_final_graph() {
    let m = vec![member(1, 10), member(2, 1), member(3, 2)];
    let links = vec![link(1, 2), link(1, 3)];
    let out = plan(
        &m,
        &[(id(1), proof(&[], false))].into_iter().collect(),
        &links,
        &BTreeMap::new(),
    );
    assert!(!out.events.iter().any(|e| e.source_id == id(1)));
    assert_eq!(
        out.coverage[&id(1)],
        HistoricalDreamCoverage::VerifiedNoLink
    );
    let out = plan(
        &m,
        &[(id(1), proof(&[2], false))].into_iter().collect(),
        &links,
        &BTreeMap::new(),
    );
    let e = out.events.iter().find(|e| e.source_id == id(1)).unwrap();
    assert_eq!(e.propagation.roots, [(id(2), 50)].into_iter().collect());
    assert_eq!(e.coverage, HistoricalDreamCoverage::ActualFirstCycle);
}
#[test]
fn historical_dream_rewires_non_dream_and_uncompleted_links_create_no_event() {
    let m = vec![member(1, 10), member(2, 1)];
    let mut links = vec![link(1, 2); 3];
    links[0].direct = false;
    links[0].kind = GraphRelationKind::DuplicateOf;
    links[1].origin = GraphRelationOrigin::User;
    links[2].completed = false;
    assert!(
        plan(&m, &BTreeMap::new(), &links, &BTreeMap::new())
            .events
            .is_empty()
    );
    let out = plan(
        &m,
        &[(id(1), HistoricalFirstCycleProof::Unsupported)]
            .into_iter()
            .collect(),
        &[link(1, 2)],
        &BTreeMap::new(),
    );
    assert_eq!(out.coverage[&id(1)], HistoricalDreamCoverage::Unsupported);
    assert!(out.events.is_empty());
}
#[test]
fn historical_dream_duplicate_authority_archived_source_and_cohort_exclusions() {
    let mut m = vec![member(1, 10), member(2, 1), member(3, 2), member(4, 10)];
    m[0].credit_eligible = false;
    m[1].credit_eligible = false;
    m[3].cohort = m[0].cohort.clone();
    let r = [(id(2), id(3))].into_iter().collect();
    let out = plan(
        &m,
        &[(id(1), proof(&[2, 3, 4], true))].into_iter().collect(),
        &[],
        &r,
    );
    let e = &out.events[0];
    assert_eq!(e.source_id, id(1));
    assert_eq!(e.propagation.roots, [(id(3), 50)].into_iter().collect());
    assert_eq!(
        e.propagation.credit_excluded,
        [id(1), id(4)].into_iter().collect()
    );
    assert!(
        !plan(&m, &BTreeMap::new(), &[link(1, 3)], &r)
            .events
            .iter()
            .any(|e| e.source_id == id(1))
    );
    assert!(
        !plan(
            &m,
            &[(id(1), proof(&[3], false))].into_iter().collect(),
            &[],
            &r
        )
        .events
        .iter()
        .any(|e| e.source_id == id(1))
    );
}
#[test]
fn historical_dream_identity_excludes_reconstruction_positions_and_roots() {
    let p = [(id(1), proof(&[2], false))].into_iter().collect();
    let a = plan(&[member(1, 10), member(2, 1)], &p, &[], &BTreeMap::new());
    let b = plan(&[member(1, 20), member(2, 1)], &p, &[], &BTreeMap::new());
    assert_eq!(a.events[0].event_id, b.events[0].event_id);
    assert_ne!(
        historical_dream_event_id([1; 16], [2; 32], id(1)),
        historical_dream_event_id([1; 16], [3; 32], id(1))
    );
}
#[test]
fn historical_dream_rejects_foreign_missing_and_conflicting_cohort_inputs() {
    let m = vec![member(1, 10), member(2, 1)];
    assert!(matches!(
        plan_historical_dream_events(
            [1; 16],
            [2; 32],
            &m,
            &BTreeMap::new(),
            &[link(1, 99)],
            &BTreeMap::new()
        ),
        Err(HistoricalDreamError::UnknownMemory(_))
    ));
    let mut m = m;
    m[1].cohort = m[0].cohort.clone();
    assert!(matches!(
        plan_historical_dream_events(
            [1; 16],
            [2; 32],
            &m,
            &BTreeMap::new(),
            &[],
            &BTreeMap::new()
        ),
        Err(HistoricalDreamError::InvalidCohort(_))
    ));
}

#[test]
fn historical_dream_identity_matches_independent_h1_typed_encoding_golden() {
    let hex: String = historical_dream_event_id([1; 16], [2; 32], id(1))
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        hex,
        "4934d474875b806cf6577542b76a697b616c4126f66e10e404e91e04556801bc"
    );
}

#[test]
fn historical_dream_archived_verified_no_link_precedes_final_links() {
    let mut source = member(1, 10);
    source.credit_eligible = false;
    let out = plan(
        &[source, member(2, 1)],
        &BTreeMap::from([(id(1), proof(&[], false))]),
        &[link(1, 2)],
        &BTreeMap::new(),
    );
    assert_eq!(
        out.coverage[&id(1)],
        HistoricalDreamCoverage::VerifiedNoLink
    );
    assert!(!out.events.iter().any(|event| event.source_id == id(1)));
}
