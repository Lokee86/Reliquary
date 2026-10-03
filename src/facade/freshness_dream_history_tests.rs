use super::{Fixture, publish, turns};

#[test]
fn freshness_direct_birth_gap_rebuilds_without_append_or_anchor_reset() {
    let fixture = Fixture::new();
    let path = fixture.path("direct-birth-gap.rel");
    let mut rel = Cva::create(&path).unwrap();
    turns(&mut rel, 0, 17);
    let memory = publish(
        &mut rel,
        "direct-gap-memory",
        "knowledge",
        "Accepted before score enrollment.",
    );
    let chunk = rel
        .container
        .chunks()
        .unwrap()
        .into_iter()
        .find(|chunk| rel.container.read(*chunk).unwrap().starts_with(b"CVAMEMV1"))
        .unwrap();
    let end = chunk.legacy_offset() + 8 + chunk.legacy_len();
    drop(rel);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(end)
        .unwrap();
    let before = std::fs::read(&path).unwrap();
    let reopened = Cva::open(&path).unwrap();
    let record = reopened.freshness_record(memory).unwrap();
    assert_eq!(record.score, 100);
    assert_eq!(record.admitted_at_turn, Some(17));
    assert!(reopened.freshness_is_publication_birth(memory));
    drop(reopened);
    assert_eq!(std::fs::read(&path).unwrap(), before);

    // Retrying the already accepted publication must first make the derived
    // birth durable, then consume the orphan intent. A second reopen must
    // retain the original clock and identity instead of losing the record.
    let mut retried = Cva::open(&path).unwrap();
    let (again, changed) = retried
        .publish_memory(
            None,
            0,
            super::draft(
                "direct-gap-memory",
                "knowledge",
                "Accepted before score enrollment.",
            ),
        )
        .unwrap();
    assert_eq!(again.id, memory);
    assert!(!changed);
    assert!(!retried.freshness_has_pending_publication(memory));
    drop(retried);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.freshness_record(memory), Some(record));
    assert!(reopened.freshness_is_publication_birth(memory));
}

#[test]
fn admission_committed_before_linkage_event_recovers_exactly_once() {
    let fixture = Fixture::new();
    let path = fixture.path("admission-before-event.rel");
    let mut rel = Cva::create(&path).unwrap();
    let root = publish(&mut rel, "admission-gap-root", "knowledge", "Root.");
    let source = publish(&mut rel, "admission-gap-source", "extracted", "Source.");
    turns(&mut rel, 0, 1010);
    let accepted_turn = rel.rel_turn_count();
    let pass_id = crate::dream_freshness::initial_pass_id(rel.owner_uuid().unwrap(), source);
    rel.begin_initial_dream_pass(
        pass_id.clone(),
        source,
        1,
        accepted_turn,
        BTreeSet::from([root]),
        rel.graph_version(),
    )
    .unwrap();
    rel.set_memory_relation_with_origin(
        source,
        root,
        GraphRelationKind::Factual,
        true,
        GraphRelationOrigin::Dream,
        rel.graph_version(),
    )
    .unwrap();
    rel.record_initial_dream_roots(&pass_id, BTreeSet::from([root]))
        .unwrap();
    rel.accept_initial_dream_settlement(&pass_id, accepted_turn)
        .unwrap();

    // Simulate a crash after the admission append but before the indivisible
    // score event. The accepted first settlement clock must not move on retry.
    assert!(rel.freshness_admit(source, accepted_turn).unwrap());
    let original_source = rel.freshness_record(source).unwrap();
    assert_eq!(original_source.admitted_at_turn, Some(accepted_turn));
    assert_eq!(
        rel.freshness_record(root)
            .unwrap()
            .score_at(accepted_turn)
            .unwrap(),
        -1,
    );
    rel.sync().unwrap();
    drop(rel);

    let mut rel = Cva::open(&path).unwrap();
    assert_eq!(rel.freshness_record(source).unwrap(), original_source);
    crate::dream_freshness::settle_initial_pass(&mut rel, source, true, 2).unwrap();
    let reinforced = rel.freshness_record(root).unwrap();
    assert_eq!(reinforced.score_at(accepted_turn).unwrap(), 49);
    assert_eq!(rel.freshness_record(source).unwrap(), original_source);
    assert!(rel.pending_initial_dream_pass(source).is_none());
    rel.sync().unwrap();
    drop(rel);

    let mut reopened = Cva::open(&path).unwrap();
    crate::dream_freshness::settle_initial_pass(&mut reopened, source, true, 2).unwrap();
    assert_eq!(reopened.freshness_record(root).unwrap(), reinforced);
    assert_eq!(reopened.freshness_record(source).unwrap(), original_source);
}

use crate::{Cva, GraphRelationKind, GraphRelationOrigin};
use std::collections::BTreeSet;

#[test]
fn freshness_recovers_inactivated_link_and_uses_accepted_graph_snapshot() {
    let fixture = Fixture::new();
    let path = fixture.path("historical-dream.rel");
    let mut rel = Cva::create(&path).unwrap();
    let root = publish(&mut rel, "historical-root", "knowledge", "Root.");
    let untouched = publish(&mut rel, "historical-unrelated", "knowledge", "Unrelated.");
    let source = publish(&mut rel, "historical-source", "extracted", "Source.");
    turns(&mut rel, 0, 1000);
    let pass_id = crate::dream_freshness::initial_pass_id(rel.owner_uuid().unwrap(), source);
    rel.begin_initial_dream_pass(
        pass_id.clone(),
        source,
        1,
        1000,
        BTreeSet::from([root, untouched]),
        rel.graph_version(),
    )
    .unwrap();
    for active in [true, false] {
        rel.set_memory_relation_with_origin(
            source,
            root,
            GraphRelationKind::Factual,
            active,
            GraphRelationOrigin::Dream,
            rel.graph_version(),
        )
        .unwrap();
    }
    rel.accept_initial_dream_settlement(&pass_id, 1000).unwrap();
    rel.sync().unwrap();
    drop(rel);
    let mut rel = Cva::open(&path).unwrap();
    rel.set_memory_relation_with_origin(
        root,
        untouched,
        GraphRelationKind::Topical,
        true,
        GraphRelationOrigin::User,
        rel.graph_version(),
    )
    .unwrap();
    crate::dream_freshness::settle_initial_pass(&mut rel, source, true, 2).unwrap();
    assert_eq!(
        rel.freshness_record(root).unwrap().score_at(1000).unwrap(),
        50
    );
    assert_eq!(
        rel.freshness_record(untouched)
            .unwrap()
            .score_at(1000)
            .unwrap(),
        0
    );
    drop(rel);
    super::truncate_final_settled_marker(&path);
    let mut rel = Cva::open(&path).unwrap();
    let before_root = rel.freshness_record(root).unwrap();
    let before_untouched = rel.freshness_record(untouched).unwrap();
    turns(&mut rel, 1000, 1010);
    crate::dream_freshness::settle_initial_pass(&mut rel, source, true, 2).unwrap();
    assert_eq!(rel.freshness_record(root).unwrap(), before_root);
    assert_eq!(rel.freshness_record(untouched).unwrap(), before_untouched);
    assert!(rel.pending_initial_dream_pass(source).is_none());
}
