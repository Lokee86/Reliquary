use crate::dream_candidate_test_support::{memory, memory_with_created_at, test_path};
use crate::{
    Cva, DreamEvidenceSide, DreamPairClassification, DreamPairEvidence, DreamPairVerification,
    DreamPublicationError, DreamPublicationOutcome, DreamRelationDirection, DreamRelationKind,
    DreamVerificationPolicy, DreamVerificationSignal, DreamVerificationVerdict, GraphRelationKind,
    MemoryDraft, MemoryId,
};
use std::collections::HashSet;

fn publish_duplicate(cva: &mut Cva, left: MemoryId, right: MemoryId) -> DreamPublicationOutcome {
    let (a, b) = canonical(left, right);
    let classification = duplicate_classification(a, b);
    let verification = accepted_verification(&classification);
    cva.publish_dream_pair(
        &classification,
        Some(&verification),
        DreamVerificationPolicy::default(),
        cva.graph_version(),
    )
    .unwrap()
}

fn canonical(left: MemoryId, right: MemoryId) -> (MemoryId, MemoryId) {
    if left.0 < right.0 {
        (left, right)
    } else {
        (right, left)
    }
}

fn duplicate_classification(a: MemoryId, b: MemoryId) -> DreamPairClassification {
    DreamPairClassification {
        model: "classifier".into(),
        a,
        b,
        relation: DreamRelationKind::DuplicateOf,
        direction: DreamRelationDirection::Undirected,
        evidence: vec![
            DreamPairEvidence {
                side: DreamEvidenceSide::A,
                quote: "A evidence".into(),
            },
            DreamPairEvidence {
                side: DreamEvidenceSide::B,
                quote: "B evidence".into(),
            },
        ],
    }
}

fn accepted_verification(classification: &DreamPairClassification) -> DreamPairVerification {
    DreamPairVerification {
        verifier_model: "verifier".into(),
        a: classification.a,
        b: classification.b,
        classification: classification.clone(),
        relation_supported: DreamVerificationSignal::Yes,
        direction_supported: DreamVerificationSignal::Yes,
        evidence_supported: DreamVerificationSignal::Yes,
        verdict: DreamVerificationVerdict::Accept,
    }
}

fn duplicate_edges(cva: &Cva) -> HashSet<(MemoryId, MemoryId)> {
    cva.graph_relations()
        .into_iter()
        .filter(|relation| relation.kind == GraphRelationKind::DuplicateOf)
        .map(|relation| (relation.source, relation.target))
        .collect()
}

fn expected_chain(mut values: Vec<(i64, MemoryId)>) -> HashSet<(MemoryId, MemoryId)> {
    values.sort_by_key(|(timestamp, id)| (*timestamp, id.0));
    values
        .windows(2)
        .map(|pair| (pair[1].1, pair[0].1))
        .collect()
}

#[test]
fn duplicate_insertion_order_does_not_change_final_chain() {
    let orders = permutations([0_usize, 1, 2, 3]);
    for order in orders {
        let mut cva = Cva::create(test_path("duplicate-permutation.cva")).unwrap();
        let ids = [
            memory(&mut cva, "a", "A", "same", 10, false),
            memory(&mut cva, "b", "B", "same", 20, false),
            memory(&mut cva, "c", "C", "same", 30, false),
            memory(&mut cva, "d", "D", "same", 40, false),
        ];
        let anchor = ids[order[0]];
        for index in order.iter().copied().skip(1) {
            publish_duplicate(&mut cva, ids[index], anchor);
        }
        assert_eq!(
            duplicate_edges(&cva),
            expected_chain(vec![(10, ids[0]), (20, ids[1]), (30, ids[2]), (40, ids[3])])
        );
    }
}

#[test]
fn middle_insertion_rewires_one_chain_transaction() {
    let mut cva = Cva::create(test_path("duplicate-middle.cva")).unwrap();
    let old = memory(&mut cva, "old", "Old", "same", 10, false);
    let middle = memory(&mut cva, "middle", "Middle", "same", 20, false);
    let new = memory(&mut cva, "new", "New", "same", 30, false);
    publish_duplicate(&mut cva, new, old);

    let DreamPublicationOutcome::Published(changes) = publish_duplicate(&mut cva, middle, old)
    else {
        panic!("middle insertion must rewire the chain");
    };
    assert_eq!(changes.len(), 3);
    assert!(changes.iter().all(|change| change.graph_version == 2));
    assert!(
        changes
            .windows(2)
            .all(|pair| pair[0].global_version == pair[1].global_version)
    );
    assert_eq!(
        duplicate_edges(&cva),
        HashSet::from([(new, middle), (middle, old)])
    );
}

#[test]
fn merging_interleaved_chains_rebuilds_one_ordered_chain() {
    let mut cva = Cva::create(test_path("duplicate-merge.cva")).unwrap();
    let a = memory(&mut cva, "merge-a", "A", "same", 10, false);
    let b = memory(&mut cva, "merge-b", "B", "same", 20, false);
    let c = memory(&mut cva, "merge-c", "C", "same", 30, false);
    let d = memory(&mut cva, "merge-d", "D", "same", 40, false);
    publish_duplicate(&mut cva, c, a);
    publish_duplicate(&mut cva, d, b);

    let DreamPublicationOutcome::Published(changes) = publish_duplicate(&mut cva, c, b) else {
        panic!("component merge must rewire the chains");
    };
    assert_eq!(changes.len(), 5);
    assert!(changes.iter().all(|change| change.graph_version == 3));
    assert_eq!(
        duplicate_edges(&cva),
        HashSet::from([(d, c), (c, b), (b, a)])
    );
}

#[test]
fn equal_source_timestamps_use_memory_id_only_as_tie_breaker() {
    let mut cva = Cva::create(test_path("duplicate-tie.cva")).unwrap();
    let a = memory(&mut cva, "tie-a", "A", "same", 10, false);
    let b = memory(&mut cva, "tie-b", "B", "same", 10, false);
    let c = memory(&mut cva, "tie-c", "C", "same", 10, false);
    publish_duplicate(&mut cva, c, a);
    publish_duplicate(&mut cva, b, a);
    assert_eq!(
        duplicate_edges(&cva),
        expected_chain(vec![(10, a), (10, b), (10, c)])
    );
}

#[test]
fn reopened_chain_rebuilds_derived_index_before_next_insertion() {
    let file = test_path("duplicate-reopen.cva");
    let mut cva = Cva::create(&file).unwrap();
    let old = memory(&mut cva, "reopen-old", "Old", "same", 10, false);
    let middle = memory(&mut cva, "reopen-middle", "Middle", "same", 20, false);
    let new = memory(&mut cva, "reopen-new", "New", "same", 30, false);
    publish_duplicate(&mut cva, new, old);
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(&file).unwrap();
    publish_duplicate(&mut reopened, middle, old);
    assert_eq!(
        duplicate_edges(&reopened),
        HashSet::from([(new, middle), (middle, old)])
    );
}

#[test]
fn duplicate_assertion_preserves_existing_nonduplicate_pair_state() {
    let mut cva = Cva::create(test_path("duplicate-reclassify.cva")).unwrap();
    let old = memory(&mut cva, "reclassify-old", "Old", "same", 10, false);
    let new = memory(&mut cva, "reclassify-new", "New", "same", 20, false);
    let (a, b) = canonical(old, new);
    let topical = DreamPairClassification {
        model: "classifier".into(),
        a,
        b,
        relation: DreamRelationKind::Topical,
        direction: DreamRelationDirection::Undirected,
        evidence: duplicate_classification(a, b).evidence,
    };
    cva.publish_dream_pair(&topical, None, DreamVerificationPolicy::default(), 0)
        .unwrap();

    let DreamPublicationOutcome::Published(changes) = publish_duplicate(&mut cva, old, new) else {
        panic!("duplicate assertion must publish");
    };
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].graph_version, 2);
    let relations = cva.graph_relations();
    assert_eq!(relations.len(), 3);
    assert_eq!(
        relations
            .iter()
            .filter(|relation| relation.kind == GraphRelationKind::Topical)
            .count(),
        2
    );
    let duplicate = relations
        .iter()
        .find(|relation| relation.kind == GraphRelationKind::DuplicateOf)
        .unwrap();
    assert_eq!((duplicate.source, duplicate.target), (new, old));
}

#[test]
fn duplicate_order_ignores_memory_creation_bookkeeping() {
    let mut cva = Cva::create(test_path("duplicate-created-at.cva")).unwrap();
    let old = memory_with_created_at(&mut cva, "created-old", "Old", "same", 10, false, 9_000);
    let new = memory_with_created_at(&mut cva, "created-new", "New", "same", 20, false, 1);
    publish_duplicate(&mut cva, old, new);
    assert_eq!(duplicate_edges(&cva), HashSet::from([(new, old)]));
}

#[test]
fn duplicate_chain_requires_authoritative_source_time() {
    let mut cva = Cva::create(test_path("duplicate-source-time.cva")).unwrap();
    let timed = memory(&mut cva, "timed", "Timed", "same", 10, false);
    let untimed = cva
        .publish_memory(None, 0, unprovenanced_draft("untimed"))
        .unwrap()
        .0
        .id;
    let (a, b) = canonical(timed, untimed);
    let classification = duplicate_classification(a, b);
    let verification = accepted_verification(&classification);
    assert!(matches!(
        cva.publish_dream_pair(
            &classification,
            Some(&verification),
            DreamVerificationPolicy::default(),
            0,
        ),
        Err(DreamPublicationError::MissingSourceTimestamp(id)) if id == untimed
    ));
    assert_eq!(cva.graph_version(), 0);
}

#[test]
fn duplicate_pair_requests_a_before_b_when_both_sources_are_untimed() {
    let mut cva = Cva::create(test_path("duplicate-source-time-order.cva")).unwrap();
    let left = cva
        .publish_memory(None, 0, unprovenanced_draft("untimed-left"))
        .unwrap()
        .0
        .id;
    let right = cva
        .publish_memory(None, 0, unprovenanced_draft("untimed-right"))
        .unwrap()
        .0
        .id;
    let (a, b) = canonical(left, right);
    let classification = duplicate_classification(a, b);
    let verification = accepted_verification(&classification);

    assert!(matches!(
        cva.publish_dream_pair(
            &classification,
            Some(&verification),
            DreamVerificationPolicy::default(),
            0,
        ),
        Err(DreamPublicationError::MissingSourceTimestamp(id)) if id == a
    ));
    assert_eq!(cva.graph_version(), 0);
}

#[test]
fn duplicate_verification_mismatch_precedes_source_time_requests() {
    let mut cva = Cva::create(test_path("duplicate-verification-before-source-time.cva")).unwrap();
    let left = cva
        .publish_memory(None, 0, unprovenanced_draft("mismatch-left"))
        .unwrap()
        .0
        .id;
    let right = cva
        .publish_memory(None, 0, unprovenanced_draft("mismatch-right"))
        .unwrap()
        .0
        .id;
    let (a, b) = canonical(left, right);
    let classification = duplicate_classification(a, b);
    let mut verification = accepted_verification(&classification);
    verification.b = a;

    assert!(matches!(
        cva.publish_dream_pair(
            &classification,
            Some(&verification),
            DreamVerificationPolicy::default(),
            0,
        ),
        Err(DreamPublicationError::VerificationMismatch)
    ));
    assert_eq!(cva.graph_version(), 0);
}

#[test]
fn duplicate_graph_version_conflict_precedes_source_time_requests() {
    let mut cva = Cva::create(test_path("duplicate-graph-version-before-source-time.cva")).unwrap();
    let left = cva
        .publish_memory(None, 0, unprovenanced_draft("conflict-left"))
        .unwrap()
        .0
        .id;
    let right = cva
        .publish_memory(None, 0, unprovenanced_draft("conflict-right"))
        .unwrap()
        .0
        .id;
    let (a, b) = canonical(left, right);
    let classification = duplicate_classification(a, b);
    let verification = accepted_verification(&classification);

    assert!(matches!(
        cva.publish_dream_pair(
            &classification,
            Some(&verification),
            DreamVerificationPolicy::default(),
            1,
        ),
        Err(DreamPublicationError::Graph(
            crate::GraphError::RevisionConflict { .. }
        ))
    ));
    assert_eq!(cva.graph_version(), 0);
}

#[test]
fn authority_filtered_rebuild_error_keeps_published_duplicate_graph() {
    let mut cva = Cva::create(test_path("duplicate-post-publish-rebuild-error.cva")).unwrap();
    let left = memory(&mut cva, "post-rebuild-left", "Left", "same", 10, false);
    let middle = memory(&mut cva, "post-rebuild-middle", "Middle", "same", 20, false);
    let right = cva
        .publish_memory(None, 0, unprovenanced_draft("post-rebuild-untimed"))
        .unwrap()
        .0
        .id;
    cva.set_memory_relation_with_origin(
        right,
        left,
        GraphRelationKind::DuplicateOf,
        true,
        crate::GraphRelationOrigin::User,
        cva.graph_version(),
    )
    .unwrap();

    // Model a current runtime index containing the established user edge. Its
    // timestamp is immaterial to the pair plan; the source becomes necessary
    // only when post-publication authority filtering rebuilds the index.
    let keys = std::collections::HashMap::from([
        (
            left,
            crate::dream_duplicate_index::DuplicateTemporalKey::new(10, left),
        ),
        (
            right,
            crate::dream_duplicate_index::DuplicateTemporalKey::new(30, right),
        ),
    ]);
    cva.duplicate_index = crate::dream_duplicate_index::DuplicateIndex::rebuild(
        cva.memory_graph_version(),
        &cva.graph_relations(),
        &keys,
    );
    let version_before_publication = cva.graph_version();
    let (a, b) = canonical(left, middle);
    let classification = duplicate_classification(a, b);
    let verification = accepted_verification(&classification);

    assert!(matches!(
        cva.publish_dream_pair(
            &classification,
            Some(&verification),
            DreamVerificationPolicy::default(),
            version_before_publication,
        ),
        Err(DreamPublicationError::MissingSourceTimestamp(id)) if id == right
    ));
    assert_eq!(cva.graph_version(), version_before_publication + 1);
    let relations = cva.graph_relations();
    assert!(relations.iter().any(|relation| {
        relation.source == right
            && relation.target == left
            && relation.kind == GraphRelationKind::DuplicateOf
            && relation.origin == crate::GraphRelationOrigin::User
    }));
    assert!(relations.iter().any(|relation| {
        relation.source == right
            && relation.target == middle
            && relation.kind == GraphRelationKind::DuplicateOf
    }));
    assert!(relations.iter().any(|relation| {
        relation.source == middle
            && relation.target == left
            && relation.kind == GraphRelationKind::DuplicateOf
    }));
}

fn unprovenanced_draft(mutation_id: &str) -> MemoryDraft {
    MemoryDraft {
        category: "project".into(),
        memory_type: "fact".into(),
        authority_kind: "unknown".into(),
        temporal_status: "unknown".into(),
        title: format!("Untimed {mutation_id}"),
        content: format!("same {mutation_id}"),
        scope: "private".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: mutation_id.into(),
        created_at_ns: 999_999,
        updated_at_ns: 999_999,
    }
}

fn permutations(values: [usize; 4]) -> Vec<[usize; 4]> {
    let mut output = Vec::new();
    let mut values = values;
    permute_from(&mut values, 0, &mut output);
    output
}

fn permute_from(values: &mut [usize; 4], start: usize, output: &mut Vec<[usize; 4]>) {
    if start == values.len() {
        output.push(*values);
        return;
    }
    for index in start..values.len() {
        values.swap(start, index);
        permute_from(values, start + 1, output);
        values.swap(start, index);
    }
}
