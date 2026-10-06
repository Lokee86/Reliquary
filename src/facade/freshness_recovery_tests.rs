//! F3/F5 owner-boundary contracts for Dream settlement recovery.
//! These internal tests exercise concrete owner recovery seams at durable crash prefixes.

#[path = "freshness_dream_history_tests.rs"]
mod historical_graph;

#[test]
fn frozen_dream_settlement_can_evaluate_without_owner_and_preserves_accepted_graph() {
    let fixture = Fixture::new();
    let path = fixture.path("detached-settlement.rel");
    let mut rel = Cva::create(&path).unwrap();
    let root = publish(&mut rel, "detached-root", "knowledge", "Root.");
    let later = publish(&mut rel, "detached-later", "knowledge", "Later.");
    let source = publish(&mut rel, "detached-source", "extracted", "Source.");
    turns(&mut rel, 0, 1000);
    let pass_id = crate::dream_freshness::initial_pass_id(rel.owner_uuid().unwrap(), source);
    rel.begin_initial_dream_pass(
        pass_id.clone(),
        source,
        1,
        1000,
        BTreeSet::from([root, later]),
        rel.graph_version(),
    )
    .unwrap();
    rel.set_memory_relation_with_origin(
        source,
        root,
        GraphRelationKind::Topical,
        true,
        GraphRelationOrigin::Dream,
        rel.graph_version(),
    )
    .unwrap();
    rel.accept_initial_dream_settlement(&pass_id, 1000).unwrap();
    let work = crate::dream_freshness::prepare_initial_settlement(&mut rel, source, true)
        .unwrap()
        .unwrap();
    // A later live edge is deliberately absent from the accepted snapshot.
    rel.set_memory_relation_with_origin(
        root,
        later,
        GraphRelationKind::Topical,
        true,
        GraphRelationOrigin::Dream,
        rel.graph_version(),
    )
    .unwrap();
    let before_later = rel.freshness_record(later);
    let result = std::thread::spawn(move || work.evaluate(4))
        .join()
        .unwrap()
        .unwrap();
    result.commit(&mut rel).unwrap();
    assert_eq!(rel.freshness_record(root).unwrap().score, 50);
    assert_eq!(rel.freshness_record(later), before_later);
    assert_eq!(
        rel.freshness_record(source).unwrap().admitted_at_turn,
        Some(1000)
    );
    assert!(rel.initial_dream_pass_settled(&pass_id));
    let before = fs::read(&path).unwrap();
    crate::dream_freshness::settle_initial_pass(&mut rel, source, true, 1).unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    drop(rel);
    assert!(
        Cva::open(path)
            .unwrap()
            .initial_dream_pass_settled(&pass_id)
    );
}
#[test]
fn divergent_pending_dream_history_is_rejected_before_output() {
    let fixture = Fixture::new();
    let left_path = fixture.path("pending-left.rel");
    let right_path = fixture.path("pending-right.rel");
    let output = fixture.path("pending-output.rel");
    let mut left = Cva::create(&left_path).unwrap();
    let source = publish(&mut left, "pending-source", "extracted", "Pending.");
    fs::copy(&left_path, &right_path).unwrap();
    let pass_id = crate::dream_freshness::initial_pass_id(left.owner_uuid().unwrap(), source);
    left.begin_initial_dream_pass(pass_id, source, 1, 0, BTreeSet::new(), left.graph_version())
        .unwrap();
    let mut right = Cva::open(&right_path).unwrap();
    turns(&mut right, 0, 1);
    let left_bytes = fs::read(&left_path).unwrap();
    let right_bytes = fs::read(&right_path).unwrap();
    assert!(Cva::reconcile(&left_path, &right_path, &output).is_err());
    assert!(!output.exists());
    assert_eq!(fs::read(left_path).unwrap(), left_bytes);
    assert_eq!(fs::read(right_path).unwrap(), right_bytes);
}

use crate::{
    Cva, FreshnessEventProof, FreshnessState, GraphRelationKind, GraphRelationOrigin, MemoryDraft,
    MemoryId,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("freshness-recovery-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft(key: &str, lifecycle: &str, content: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: key.into(),
        content: content.into(),
        scope: "project".into(),
        lifecycle_state: lifecycle.into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(0),
        mutation_id: key.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn publish(rel: &mut Cva, key: &str, lifecycle: &str, content: &str) -> MemoryId {
    let (memory, changed) = rel
        .publish_memory(None, 0, draft(key, lifecycle, content))
        .unwrap();
    assert!(changed);
    memory.id
}

fn turns(rel: &mut Cva, from: usize, to: usize) {
    for n in from..to {
        rel.append_node(
            format!("freshness-turn-{n}"),
            "conversation".into(),
            None,
            if n % 2 == 0 { "user" } else { "assistant" }.into(),
            n as i64,
            "accepted owner activity",
        )
        .unwrap();
    }
}

fn event_proof(rel: &Cva) -> FreshnessEventProof {
    FreshnessEventProof {
        graph_version: rel.memory_graph_version(),
        community_generation: None,
        community_graph_version: None,
    }
}

/// Simulate the durable file prefix after the Freshness event sync but before
/// Dream's final settled-marker append. Container chunks are length-prefixed;
/// removing exactly the terminal marker preserves the preceding admission/event.
fn truncate_final_settled_marker(path: &std::path::Path) {
    let mut bytes = fs::read(path).unwrap();
    let header_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let mut offset = header_len;
    let mut marker_offset = None;
    while offset < bytes.len() {
        let chunk_len = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize;
        let payload_start = offset + 8;
        let payload_end = payload_start + chunk_len;
        let payload = &bytes[payload_start..payload_end];
        if payload.starts_with(b"CVDREAM1") && payload.get(24) == Some(&2) {
            marker_offset = Some(offset);
        }
        offset = payload_end;
    }
    let marker_offset = marker_offset.expect("final Dream settled-marker chunk");
    assert_eq!(offset, bytes.len());
    assert_eq!(
        marker_offset
            + 8
            + u64::from_le_bytes(bytes[marker_offset..marker_offset + 8].try_into().unwrap())
                as usize,
        bytes.len(),
        "the settled marker must be the last durable append in this fixture"
    );
    bytes.truncate(marker_offset);
    fs::write(path, bytes).unwrap();
}

#[test]
fn first_dream_partial_graph_publication_recovers_original_settlement_cut_once() {
    let fixture = Fixture::new();
    let path = fixture.path("source.rel");
    let mut rel = Cva::create(&path).unwrap();

    let first_root = publish(&mut rel, "existing-root-a", "knowledge", "Existing A.");
    let second_root = publish(&mut rel, "existing-root-b", "knowledge", "Existing B.");
    let source = publish(&mut rel, "new-source", "extracted", "New extracted memory.");

    turns(&mut rel, 0, 6);
    let begin_turn = rel.rel_turn_count();
    let preexisting = BTreeSet::from([first_root, second_root]);
    let pass_id = crate::dream_freshness::initial_pass_id(rel.owner_uuid().unwrap(), source);
    let _pass = rel
        .begin_initial_dream_pass(
            pass_id.clone(),
            source,
            1,
            begin_turn,
            preexisting.clone(),
            rel.graph_version(),
        )
        .unwrap();

    // Simulate a partial Dream pass: the first relation has a Freshness root
    // receipt, while a second committed graph relation has not yet been receipted.
    rel.set_memory_relation_with_origin(
        source,
        first_root,
        GraphRelationKind::Factual,
        true,
        GraphRelationOrigin::Dream,
        rel.graph_version(),
    )
    .unwrap();
    rel.record_initial_dream_roots(&pass_id, BTreeSet::from([first_root]))
        .unwrap();
    rel.set_memory_relation_with_origin(
        source,
        second_root,
        GraphRelationKind::Topical,
        true,
        GraphRelationOrigin::Dream,
        rel.graph_version(),
    )
    .unwrap();

    turns(&mut rel, 6, 14);
    let accepted_settlement_turn = rel.rel_turn_count();
    assert!(accepted_settlement_turn > begin_turn);
    rel.accept_initial_dream_settlement(&pass_id, accepted_settlement_turn)
        .unwrap();
    rel.sync().unwrap();
    drop(rel);

    let mut rel = Cva::open(&path).unwrap();
    let recovered = rel.pending_initial_dream_pass(source).unwrap();
    assert_eq!(recovered.pass_id, pass_id);
    assert_eq!(recovered.turn, begin_turn);
    assert_eq!(
        recovered.accepted_settlement_turn,
        Some(accepted_settlement_turn)
    );
    assert_eq!(recovered.preexisting_ids, preexisting);
    assert_eq!(recovered.roots, BTreeSet::from([first_root]));
    assert_eq!(
        rel.graph_relations()
            .iter()
            .filter(|edge| edge.origin == GraphRelationOrigin::Dream)
            .count(),
        2,
        "both first-cycle graph writes survive even though the second per-pair receipt was missed"
    );

    // The adapter's recovery scan contributes the missed root before settlement.
    // A failed all-effects validation must not apply any target effect.
    rel.record_initial_dream_roots(&pass_id, BTreeSet::from([first_root, second_root]))
        .unwrap();
    let before_a = rel.freshness_record(first_root).unwrap();
    let before_b = rel.freshness_record(second_root).unwrap();
    let bad_target = MemoryId([0xff; 32]);
    let proof = event_proof(&rel);
    let failed = rel.settle_initial_dream_pass(
        &pass_id,
        true,
        proof,
        BTreeMap::from([(first_root, 50), (second_root, 50), (bad_target, 50)]),
    );
    assert!(failed.is_err());
    assert_eq!(rel.freshness_record(first_root).unwrap(), before_a);
    assert_eq!(rel.freshness_record(second_root).unwrap(), before_b);

    // Replaying the original pass publishes one complete multi-target event at
    // the accepted settlement cut; its begin/attempt cut is not an event clock.
    assert!(
        rel.settle_initial_dream_pass(
            &pass_id,
            true,
            proof,
            BTreeMap::from([(first_root, 50), (second_root, 50)]),
        )
        .unwrap()
    );
    let admitted_source = rel.freshness_record(source).unwrap();
    assert_eq!(
        admitted_source.admitted_at_turn,
        Some(accepted_settlement_turn)
    );
    for id in [first_root, second_root] {
        let record = rel.freshness_record(id).unwrap();
        assert_eq!(record.score_at(accepted_settlement_turn).unwrap(), 100);
        assert_eq!(
            record.state_at(accepted_settlement_turn).unwrap(),
            FreshnessState::Fresh
        );
    }

    rel.sync().unwrap();
    drop(rel);

    // The normal settlement already synced its event and admission. Cutting
    // only the final Dream marker recreates the crash interleaving between the
    // two owner journals without relying on timing or an implementation hook.
    truncate_final_settled_marker(&path);
    let mut rel = Cva::open(&path).unwrap();
    let recovered_after_event = rel.pending_initial_dream_pass(source).unwrap();
    assert_eq!(recovered_after_event.turn, begin_turn);
    assert_eq!(
        recovered_after_event.accepted_settlement_turn,
        Some(accepted_settlement_turn)
    );
    assert_eq!(
        rel.freshness_record(source).unwrap().admitted_at_turn,
        Some(accepted_settlement_turn),
        "the admission append survives the missing Dream marker"
    );
    let event_postimage_a = rel.freshness_record(first_root).unwrap();
    let event_postimage_b = rel.freshness_record(second_root).unwrap();
    assert_eq!(
        event_postimage_a
            .score_at(accepted_settlement_turn)
            .unwrap(),
        100
    );
    assert_eq!(
        event_postimage_b
            .score_at(accepted_settlement_turn)
            .unwrap(),
        100
    );

    assert!(
        rel.settle_initial_dream_pass(
            &pass_id,
            true,
            event_proof(&rel),
            BTreeMap::from([(first_root, 50), (second_root, 50)]),
        )
        .unwrap(),
        "retry writes the missing Dream marker while the event receipt suppresses duplicate credit"
    );
    assert!(rel.pending_initial_dream_pass(source).is_none());
    assert_eq!(rel.freshness_record(first_root).unwrap(), event_postimage_a);
    assert_eq!(
        rel.freshness_record(second_root).unwrap(),
        event_postimage_b
    );
    assert!(
        !rel.settle_initial_dream_pass(
            &pass_id,
            true,
            event_proof(&rel),
            BTreeMap::from([(first_root, 50), (second_root, 50)]),
        )
        .unwrap(),
        "a fully settled retry is an idempotent no-op"
    );
    assert_eq!(rel.freshness_record(first_root).unwrap(), event_postimage_a);
    assert_eq!(
        rel.freshness_record(second_root).unwrap(),
        event_postimage_b
    );

    // A later graph rewrite among existing Memories is not a new first-cycle
    // producer event and must leave both records untouched.
    let before_rewire_a = rel.freshness_record(first_root).unwrap();
    let before_rewire_b = rel.freshness_record(second_root).unwrap();
    rel.set_memory_relation_with_origin(
        first_root,
        second_root,
        GraphRelationKind::Causal,
        true,
        GraphRelationOrigin::Dream,
        rel.graph_version(),
    )
    .unwrap();
    assert_eq!(rel.freshness_record(first_root).unwrap(), before_rewire_a);
    assert_eq!(rel.freshness_record(second_root).unwrap(), before_rewire_b);
}

// Append to src/facade/freshness_recovery_tests.rs. Existing Fixture and publish
// helpers are in scope. `recent_receipt_stats` is a cfg(test) FreshnessStore
// accessor returning (resident receipt count, encoded resident receipt bytes).

#[test]
fn accepted_use_receipt_cache_stays_bounded_and_oldest_retry_scans_durable_ledger() {
    use crate::AcceptedMemoryUseReceipt;

    let fixture = Fixture::new();
    let path = fixture.path("bounded-receipts.rel");
    let mut rel = Cva::create(&path).unwrap();
    let source = publish(&mut rel, "bounded-receipt-source", "knowledge", "Source.");
    let target = publish(&mut rel, "bounded-receipt-target", "knowledge", "Target.");
    turns(&mut rel, 0, 1000);
    let owner = rel.owner_uuid().unwrap();
    let oldest = AcceptedMemoryUseReceipt {
        owner_uuid: owner,
        accepted_turn: 1000,
        use_id: "cache-use-0000".into(),
        memories: vec![source],
    };
    assert_eq!(
        rel.record_accepted_memory_use(oldest.clone(), 1).unwrap(),
        1
    );

    // Keep all events on one REL activity cut so this tests receipt residency,
    // not a large Archive fixture. Every use ID is an independently accepted use.
    for index in 0..300 {
        let receipt = AcceptedMemoryUseReceipt {
            owner_uuid: owner,
            accepted_turn: 1000,
            use_id: format!("cache-use-{:04}", index + 1),
            memories: vec![source],
        };
        assert_eq!(rel.record_accepted_memory_use(receipt, 1).unwrap(), 1);
    }

    let (resident_count, resident_bytes) = rel.freshness.recent_receipt_stats();
    assert!(
        resident_count <= 128,
        "resident receipt count: {resident_count}"
    );
    assert!(
        resident_bytes <= 256 * 1024,
        "resident receipt bytes: {resident_bytes}"
    );

    // A changed live graph makes an accidental propagation-on-retry visible.
    // The durable receipt must resolve before traversing this newly added edge.
    rel.set_memory_relation_with_origin(
        source,
        target,
        GraphRelationKind::Topical,
        true,
        GraphRelationOrigin::Dream,
        rel.graph_version(),
    )
    .unwrap();
    let before_source = rel.freshness_record(source).unwrap();
    let before_target = rel.freshness_record(target).unwrap();
    rel.sync().unwrap();
    let bytes_before_retry = fs::read(&path).unwrap();
    assert_eq!(
        rel.record_accepted_memory_use(oldest.clone(), 1).unwrap(),
        0
    );
    assert_eq!(rel.freshness_record(source).unwrap(), before_source);
    assert_eq!(rel.freshness_record(target).unwrap(), before_target);
    assert_eq!(fs::read(&path).unwrap(), bytes_before_retry);

    drop(rel);
    let mut reopened = Cva::open(&path).unwrap();
    let (resident_count, resident_bytes) = reopened.freshness.recent_receipt_stats();
    assert!(
        resident_count <= 128,
        "reopened resident receipt count: {resident_count}"
    );
    assert!(
        resident_bytes <= 256 * 1024,
        "reopened resident receipt bytes: {resident_bytes}"
    );
    let bytes_before_retry = fs::read(&path).unwrap();
    assert_eq!(reopened.record_accepted_memory_use(oldest, 1).unwrap(), 0);
    assert_eq!(reopened.freshness_record(source).unwrap(), before_source);
    assert_eq!(reopened.freshness_record(target).unwrap(), before_target);
    assert_eq!(fs::read(&path).unwrap(), bytes_before_retry);
}
