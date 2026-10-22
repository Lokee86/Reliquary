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
        .find(|chunk| {
            let payload = rel.container.read(*chunk).unwrap();
            crate::memory_compact_codec::publication::coordinates(&payload)
                .unwrap()
                .is_some()
        })
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
    let pass_id = crate::dream_pass_identity::initial_pass_id(rel.owner_uuid().unwrap(), source);
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

use crate::freshness_dream_journal::{DirectBirthPrefix, DreamFreshnessJournal};
use crate::{Cva, GraphRelationKind, GraphRelationOrigin};
use std::collections::BTreeSet;

#[test]
fn direct_birth_prefix_correlates_intent_and_replay_with_memory_first_identity_cut() {
    let fixture = Fixture::new();
    let path = fixture.path("direct-birth-memory-prefix.rel");
    let mut rel = Cva::create(&path).unwrap();
    let first = publish(&mut rel, "prefix-first", "knowledge", "First.");
    let later = publish(&mut rel, "prefix-later", "knowledge", "Later.");
    assert_eq!(rel.memories.first_mutation_id(first), Some("prefix-first"));
    let owner = rel.owner_uuid().unwrap();
    let mutation = rel.memories.first_mutation_id(first).unwrap().to_owned();

    let mut source = DreamFreshnessJournal::new(Some(owner));
    let intent = source
        .publication_intent_payload(first, &mutation, 0, false)
        .unwrap();
    source.ingest(&intent).unwrap();
    let consume = source
        .consume_publication_intent_payload(first)
        .unwrap()
        .unwrap();

    let mut pending = DirectBirthPrefix::new(owner);
    pending.ingest(&intent).unwrap();
    assert_eq!(
        rel.direct_birth_ids_at_memory_prefix(&pending, 1).unwrap(),
        BTreeSet::from([first]),
        "a matching pending intent is sufficient once its first Memory identity is in the cut"
    );
    assert_eq!(
        rel.direct_birth_ids_at_memory_prefix(&pending, 0).unwrap(),
        BTreeSet::new(),
        "an intent alone cannot introduce a Memory outside the queried first-identity prefix"
    );

    let mut mismatched = DirectBirthPrefix::new(owner);
    let wrong_intent = DreamFreshnessJournal::new(Some(owner))
        .publication_intent_payload(first, "different-first-mutation", 0, false)
        .unwrap();
    mismatched.ingest(&wrong_intent).unwrap();
    assert!(
        rel.direct_birth_ids_at_memory_prefix(&mismatched, 1)
            .unwrap()
            .is_empty()
    );
    let other_owner = [owner[0].wrapping_add(1); 16];
    let foreign_intent = DreamFreshnessJournal::new(Some(other_owner))
        .publication_intent_payload(first, &mutation, 0, false)
        .unwrap();
    let mut foreign_owner = DirectBirthPrefix::new(other_owner);
    foreign_owner.ingest(&foreign_intent).unwrap();
    assert!(
        rel.direct_birth_ids_at_memory_prefix(&foreign_owner, 1)
            .is_err()
    );

    let mut retries = DreamFreshnessJournal::new(Some(owner));
    let accepted = retries
        .publication_intent_payload(first, &mutation, 0, false)
        .unwrap();
    retries.ingest(&accepted).unwrap();
    let accepted_receipt = retries
        .consume_publication_intent_payload(first)
        .unwrap()
        .unwrap();
    retries.ingest(&accepted_receipt).unwrap();
    let later_retry = retries
        .publication_intent_payload(first, "later-retry", 1, false)
        .unwrap();
    retries.ingest(&later_retry).unwrap();
    let later_receipt = retries
        .consume_publication_intent_payload(first)
        .unwrap()
        .unwrap();
    let mut retry_prefix = DirectBirthPrefix::new(owner);
    for payload in [&accepted, &accepted_receipt, &later_retry, &later_receipt] {
        retry_prefix.ingest(payload).unwrap();
    }
    assert_eq!(
        rel.direct_birth_ids_at_memory_prefix(&retry_prefix, 1)
            .unwrap(),
        BTreeSet::from([first]),
        "a later admitted retry cannot replace the accepted first mutation evidence"
    );

    // A delayed Consume does not move the birth later than the matching
    // Intent plus first Memory facts already visible at the cut.
    pending.ingest(&consume).unwrap();
    assert_eq!(
        rel.direct_birth_ids_at_memory_prefix(&pending, 1).unwrap(),
        BTreeSet::from([first])
    );

    let mut replay = DirectBirthPrefix::new(owner);
    let replay_first = DreamFreshnessJournal::new(Some(owner))
        .replay_publication_birth_payload(first)
        .unwrap();
    let replay_later = DreamFreshnessJournal::new(Some(owner))
        .replay_publication_birth_payload(later)
        .unwrap();
    let missing = crate::MemoryId([241; 32]);
    let replay_missing = DreamFreshnessJournal::new(Some(owner))
        .replay_publication_birth_payload(missing)
        .unwrap();
    replay.ingest(&replay_first).unwrap();
    replay.ingest(&replay_first).unwrap();
    replay.ingest(&replay_later).unwrap();
    replay.ingest(&replay_missing).unwrap();
    assert_eq!(
        rel.direct_birth_ids_at_memory_prefix(&replay, 1).unwrap(),
        BTreeSet::from([first]),
        "ReplayBirth is gated by the queried first-identity prefix and deduplicated"
    );
    assert!(
        rel.direct_birth_ids_at_memory_prefix(&replay, 0)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        rel.direct_birth_ids_at_memory_prefix(&replay, 2).unwrap(),
        BTreeSet::from([first, later])
    );
    assert!(
        !rel.direct_birth_ids_at_memory_prefix(&replay, 2)
            .unwrap()
            .contains(&missing)
    );
    assert!(rel.direct_birth_ids_at_memory_prefix(&replay, 3).is_err());
}

fn append_ordered_memory_creation(
    rel: &mut Cva,
    id: crate::MemoryId,
    mutation_id: &str,
    local_version: u64,
    global_version: u64,
    lifecycle_state: &str,
) {
    let title = mutation_id;
    let content = format!("Synthetic first revision for {mutation_id}.");
    let body = crate::memory_model::memory_body_bytes(title, &content);
    let body_id = crate::memory_model::memory_body_id(title, &content);
    rel.container
        .append(&crate::memory_codec::encode_body(body_id, &body).unwrap())
        .unwrap();
    let record = crate::memory_model::MemoryRecord {
        id,
        revision: 1,
        body_id,
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
        lifecycle_state: lifecycle_state.into(),
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
        source_ref: None,
        temporal_inference: None,
        mutation_id: mutation_id.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
        global_version,
        memory_version: local_version,
    };
    let body = crate::memory_compact_codec::publication::encode(
        &crate::memory_compact_codec::publication::MemoryPublication::Creation(
            crate::memory_compact_contract::MemoryCreation::new(record).unwrap(),
        ),
    )
    .unwrap();
    let envelope = crate::container::publication_envelope::encode(
        crate::container::publication_envelope::PublicationEnvelope {
            global_version,
            clock: crate::container::publication_envelope::PublicationClock::BindReserved,
            body: &body,
        },
    )
    .unwrap();
    rel.container.append(&envelope).unwrap();
}

#[test]
fn first_dream_population_keeps_older_archived_and_tombstoned_births() {
    let fixture = Fixture::new();
    let path = fixture.path("dream-population-archived-tombstoned.rel");
    let mut rel = Cva::create(&path).unwrap();

    let tombstoned = publish(
        &mut rel,
        "population-tombstoned",
        "knowledge",
        "Older tombstone.",
    );
    assert!(matches!(
        rel.tombstone_memory(tombstoned, 1).unwrap(),
        crate::owner_lifecycle::TombstoneOutcome::Published(_)
    ));

    let mut archived_draft =
        super::draft("population-archived", "knowledge", "Older archived birth.");
    archived_draft.archived = true;
    let (archived_memory, changed) = rel.publish_memory(None, 0, archived_draft).unwrap();
    assert!(changed);
    let archived = archived_memory.id;

    let source = publish(&mut rel, "population-source", "extracted", "New source.");
    let pass = crate::dream_freshness::begin_initial_pass(&mut rel, source, [].into_iter())
        .unwrap()
        .unwrap();

    assert_eq!(
        pass.preexisting_ids,
        BTreeSet::from([tombstoned, archived]),
        "the legacy cut is based on durable birth and strictly earlier global first revision, not current liveness"
    );
    assert!(matches!(
        rel.memory_lifecycle_state(tombstoned).unwrap(),
        crate::owner_lifecycle::LifecycleState::Tombstoned(_)
    ));
    assert!(!rel.memory_revision(tombstoned, 1).unwrap().archived);
    assert!(rel.memory_revision(archived, 1).unwrap().archived);
}

#[test]
fn first_dream_population_uses_first_global_order_when_memory_local_order_reverses() {
    let fixture = Fixture::new();
    let path = fixture.path("dream-population-reversed-global-local.rel");
    let mut rel = Cva::create(&path).unwrap();

    // Reserve global order 1, 2, 3, then bind accepted Memory first revisions
    // in physical/local order 3, 1, 2. The oldest local Memory is therefore
    // newer than the source by global order, while the next local Memory is
    // older by global order.
    for time in [10, 20, 30] {
        rel.container
            .allocate_version_with_transaction_time(Some(time))
            .unwrap();
    }
    let local_first_but_globally_later = crate::MemoryId([61; 32]);
    let globally_earlier = crate::MemoryId([62; 32]);
    let source = crate::MemoryId([63; 32]);
    append_ordered_memory_creation(
        &mut rel,
        local_first_but_globally_later,
        "local-first-global-later",
        1,
        3,
        "knowledge",
    );
    append_ordered_memory_creation(
        &mut rel,
        globally_earlier,
        "local-second-global-first",
        2,
        1,
        "knowledge",
    );
    append_ordered_memory_creation(&mut rel, source, "local-third-source", 3, 2, "extracted");
    rel.sync().unwrap();
    drop(rel);

    let mut rel = Cva::open(&path).unwrap();
    for id in [local_first_but_globally_later, globally_earlier, source] {
        rel.freshness_replay_publication_birth(id).unwrap();
    }
    assert_eq!(
        rel.memory_revision(local_first_but_globally_later, 1)
            .unwrap()
            .global_version,
        3
    );
    assert_eq!(
        rel.memory_revision(globally_earlier, 1)
            .unwrap()
            .global_version,
        1
    );
    assert_eq!(rel.memory_revision(source, 1).unwrap().global_version, 2);

    let pass = crate::dream_freshness::begin_initial_pass(&mut rel, source, [].into_iter())
        .unwrap()
        .unwrap();
    assert_eq!(pass.preexisting_ids, BTreeSet::from([globally_earlier]));
}

#[test]
fn first_dream_population_includes_later_local_birth_with_earlier_global_version() {
    let fixture = Fixture::new();
    let path = fixture.path("dream-population-later-local-earlier-global.rel");
    let mut rel = Cva::create(&path).unwrap();

    // Physical/local order puts the source first, but its reserved global
    // coordinate is later. The candidate is admitted afterward with the
    // earlier global coordinate and therefore belongs to the legacy set.
    for time in [10, 20] {
        rel.container
            .allocate_version_with_transaction_time(Some(time))
            .unwrap();
    }
    let source = crate::MemoryId([64; 32]);
    let later_local_candidate = crate::MemoryId([65; 32]);
    append_ordered_memory_creation(
        &mut rel,
        source,
        "source-local-one-global-two",
        1,
        2,
        "extracted",
    );
    append_ordered_memory_creation(
        &mut rel,
        later_local_candidate,
        "candidate-local-two-global-one",
        2,
        1,
        "knowledge",
    );
    rel.sync().unwrap();
    drop(rel);

    let mut rel = Cva::open(&path).unwrap();
    rel.freshness_replay_publication_birth(source).unwrap();
    rel.freshness_replay_publication_birth(later_local_candidate)
        .unwrap();
    assert_eq!(rel.memory_revision(source, 1).unwrap().global_version, 2);
    assert_eq!(
        rel.memory_revision(later_local_candidate, 1)
            .unwrap()
            .global_version,
        1
    );

    let pass = crate::dream_freshness::begin_initial_pass(&mut rel, source, [].into_iter())
        .unwrap()
        .unwrap();
    assert_eq!(
        pass.preexisting_ids,
        BTreeSet::from([later_local_candidate])
    );
}

#[test]
fn first_dream_population_excludes_sibling_from_grouped_birth_cohort() {
    let (path, mut rel, _episode) =
        crate::insomnia::completion_tests::setup("dream-population-grouped-cohort.rel");
    let prior = publish(&mut rel, "grouped-prior-root", "knowledge", "Prior root.");
    let result = crate::insomnia::completion_tests::process(&mut rel, 2, 110);
    let ids = result
        .created
        .iter()
        .map(|memory| memory.id)
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    assert!(
        rel.freshness_same_publication_cohort(ids[0], ids[1])
            .unwrap()
    );

    let pass = crate::dream_freshness::begin_initial_pass(&mut rel, ids[0], [].into_iter())
        .unwrap()
        .unwrap();
    assert_eq!(pass.preexisting_ids, BTreeSet::from([prior]));
    assert!(!pass.preexisting_ids.contains(&ids[1]));
    rel.sync().unwrap();
    drop(rel);

    let reopened = Cva::open(&path).unwrap();
    assert!(
        reopened
            .freshness_same_publication_cohort(ids[0], ids[1])
            .unwrap()
    );
}

#[test]
fn freshness_recovers_inactivated_link_and_uses_accepted_graph_snapshot() {
    let fixture = Fixture::new();
    let path = fixture.path("historical-dream.rel");
    let mut rel = Cva::create(&path).unwrap();
    let root = publish(&mut rel, "historical-root", "knowledge", "Root.");
    let untouched = publish(&mut rel, "historical-unrelated", "knowledge", "Unrelated.");
    let source = publish(&mut rel, "historical-source", "extracted", "Source.");
    turns(&mut rel, 0, 1000);
    let pass_id = crate::dream_pass_identity::initial_pass_id(rel.owner_uuid().unwrap(), source);
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

#[test]
fn historical_dream_retained_completed_proof_survives_reopen_without_writes() {
    let fixture = Fixture::new();
    let path = fixture.path("historical-completed-proof.rel");
    let mut rel = Cva::create(&path).unwrap();
    let root = publish(&mut rel, "historical-proof-root", "knowledge", "Root.");
    let linked = publish(&mut rel, "historical-proof-linked", "extracted", "Linked.");
    let empty = publish(&mut rel, "historical-proof-empty", "extracted", "Empty.");
    for (source, roots) in [(linked, BTreeSet::from([root])), (empty, BTreeSet::new())] {
        let pass_id =
            crate::dream_pass_identity::initial_pass_id(rel.owner_uuid().unwrap(), source);
        rel.begin_initial_dream_pass(
            pass_id.clone(),
            source,
            1,
            0,
            roots.clone(),
            rel.graph_version(),
        )
        .unwrap();
        if source == linked {
            rel.set_memory_relation_with_origin(
                source,
                root,
                GraphRelationKind::Topical,
                true,
                GraphRelationOrigin::Dream,
                rel.graph_version(),
            )
            .unwrap();
        }
        rel.record_initial_dream_roots(&pass_id, roots).unwrap();
        rel.accept_initial_dream_settlement(&pass_id, 0).unwrap();
        crate::dream_freshness::settle_initial_pass(&mut rel, source, false, 1).unwrap();
    }
    rel.sync().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let proofs = rel.historical_completed_initial_dream_passes().unwrap();
    assert_eq!(proofs[&linked].roots, BTreeSet::from([root]));
    assert!(proofs[&empty].roots.is_empty());
    assert_eq!(
        rel.historical_dream_representatives(linked, &proofs[&linked].roots)
            .unwrap(),
        std::collections::BTreeMap::from([(root, root)])
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    drop(rel);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened
            .historical_completed_initial_dream_passes()
            .unwrap(),
        proofs
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
