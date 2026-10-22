use crate::dream_begin_codec::{self, BeginPopulation, BeginRecord};
use crate::{Cva, MemoryDraft, MemoryId};
use std::{collections::BTreeSet, fs, path::PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "dream-population-authority-{}",
                uuid::Uuid::new_v4()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft(name: &str, lifecycle_state: &str, archived: bool) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: name.into(),
        content: format!("Dream population authority {name}"),
        scope: "project".into(),
        lifecycle_state: lifecycle_state.into(),
        archived,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: format!("dream-population-authority-{name}"),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn publish(rel: &mut Cva, name: &str, lifecycle_state: &str, archived: bool) -> MemoryId {
    rel.publish_memory(None, 0, draft(name, lifecycle_state, archived))
        .unwrap()
        .0
        .id
}

fn append_begin(
    rel: &mut Cva,
    source: MemoryId,
    population: BeginPopulation,
    pass_id: Option<String>,
) -> String {
    let owner = rel.owner_uuid().unwrap();
    let pass_id = pass_id.unwrap_or_else(|| crate::dream_freshness::initial_pass_id(owner, source));
    let payload = dream_begin_codec::encode(&BeginRecord {
        owner_uuid: owner,
        pass_id: pass_id.clone(),
        source_id: source,
        turn: 0,
        graph_version_before: rel.graph_version(),
        population,
    })
    .unwrap();
    rel.container.append(&payload).unwrap();
    rel.container.sync().unwrap();
    pass_id
}

fn reopen(path: &std::path::Path) -> Cva {
    Cva::open(path).unwrap()
}

#[test]
fn compact_and_explicit_begin_materialize_their_exact_population_after_reopen() {
    let scratch = Scratch::new();
    let path = scratch.0.join("population.rel");
    let mut rel = Cva::create(&path).unwrap();
    let root = publish(&mut rel, "root", "knowledge", false);
    let source = publish(&mut rel, "source", "extracted", false);
    let expected = BTreeSet::from([root]);
    let cut = rel.memory_version();
    let compact_id = append_begin(
        &mut rel,
        source,
        BeginPopulation::CompactCut {
            memory_cut: cut,
            digest: crate::dream_population_digest::digest(&expected),
        },
        None,
    );
    drop(rel);

    let mut rel = reopen(&path);
    let later = publish(&mut rel, "after-saved-begin", "knowledge", false);
    let before_rejected_roots = fs::read(&path).unwrap();
    for outside in [later, source] {
        assert!(
            rel.record_initial_dream_roots(&compact_id, BTreeSet::from([outside]))
                .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), before_rejected_roots);
    }
    assert_eq!(
        rel.pending_initial_dream_pass(source)
            .unwrap()
            .preexisting_ids,
        expected
    );
    rel.record_initial_dream_roots(&compact_id, expected.clone())
        .unwrap();
    rel.record_initial_dream_roots(&compact_id, expected.clone())
        .unwrap();
    drop(rel);
    assert_eq!(
        reopen(&path)
            .pending_initial_dream_pass(source)
            .unwrap()
            .roots,
        expected
    );

    let explicit_path = scratch.0.join("explicit.rel");
    let mut explicit = Cva::create(&explicit_path).unwrap();
    let explicit_root = publish(&mut explicit, "explicit-root", "knowledge", false);
    let explicit_source = publish(&mut explicit, "explicit-source", "extracted", false);
    let subset = BTreeSet::new();
    let explicit_id = append_begin(
        &mut explicit,
        explicit_source,
        BeginPopulation::ExplicitSet(subset.clone()),
        None,
    );
    drop(explicit);
    let mut explicit = reopen(&explicit_path);
    let pass = explicit
        .pending_initial_dream_pass(explicit_source)
        .unwrap();
    assert_eq!(pass.preexisting_ids, subset);
    assert!(
        explicit
            .record_initial_dream_roots(&explicit_id, BTreeSet::new())
            .is_ok()
    );
    assert!(
        explicit
            .record_initial_dream_roots(&explicit_id, BTreeSet::from([explicit_root]))
            .is_err()
    );
    assert_eq!(
        explicit
            .pending_initial_dream_pass(explicit_source)
            .unwrap()
            .roots,
        BTreeSet::new()
    );
}

#[test]
fn roots_outside_explicit_population_are_rejected_before_append_and_source_is_never_a_root() {
    let scratch = Scratch::new();
    let path = scratch.0.join("roots.rel");
    let mut rel = Cva::create(&path).unwrap();
    let root = publish(&mut rel, "root", "knowledge", false);
    let source = publish(&mut rel, "source", "extracted", false);
    let pass_id = append_begin(
        &mut rel,
        source,
        BeginPopulation::ExplicitSet(BTreeSet::new()),
        None,
    );
    drop(rel);

    let mut rel = reopen(&path);
    let before = fs::metadata(&path).unwrap().len();
    assert!(
        rel.record_initial_dream_roots(&pass_id, BTreeSet::from([root]))
            .is_err()
    );
    assert_eq!(fs::metadata(&path).unwrap().len(), before);
    drop(rel);

    let source_root_path = scratch.0.join("source-root.rel");
    let mut source_root = Cva::create(&source_root_path).unwrap();
    let source = publish(&mut source_root, "source-root", "extracted", false);
    let pass_id = append_begin(
        &mut source_root,
        source,
        BeginPopulation::ExplicitSet(BTreeSet::new()),
        None,
    );
    drop(source_root);
    let mut source_root = reopen(&source_root_path);
    let before = fs::metadata(&source_root_path).unwrap().len();
    assert!(
        source_root
            .record_initial_dream_roots(&pass_id, BTreeSet::from([source]))
            .is_err()
    );
    assert_eq!(fs::metadata(&source_root_path).unwrap().len(), before);
}

#[test]
fn begin_retry_and_empty_roots_reopen_as_one_valid_no_link_pass() {
    let scratch = Scratch::new();
    let path = scratch.0.join("retry.rel");
    let mut rel = Cva::create(&path).unwrap();
    let root = publish(&mut rel, "root", "knowledge", false);
    let source = publish(&mut rel, "source", "extracted", false);
    let owner = rel.owner_uuid().unwrap();
    let pass_id = crate::dream_freshness::initial_pass_id(owner, source);
    let explicit = BeginPopulation::ExplicitSet(BTreeSet::from([root]));
    append_begin(&mut rel, source, explicit.clone(), Some(pass_id.clone()));
    append_begin(&mut rel, source, explicit, Some(pass_id.clone()));
    drop(rel);

    let mut rel = reopen(&path);
    let pass = rel.pending_initial_dream_pass(source).unwrap();
    assert_eq!(pass.pass_id, pass_id);
    assert_eq!(pass.preexisting_ids, BTreeSet::from([root]));
    rel.record_initial_dream_roots(&pass_id, BTreeSet::new())
        .unwrap();
    drop(rel);
    let pass = reopen(&path).pending_initial_dream_pass(source).unwrap();
    assert!(pass.roots.is_empty());
}

#[test]
fn invalid_first_pass_identity_and_archived_source_are_rejected_without_pending_pass() {
    let scratch = Scratch::new();
    let path = scratch.0.join("eligibility.rel");
    let mut rel = Cva::create(&path).unwrap();
    let root = publish(&mut rel, "root", "knowledge", false);
    let source = publish(&mut rel, "source", "extracted", false);
    let wrong_id = "forged-initial-pass-id".to_owned();
    let graph_cut = rel.graph_version();
    let result =
        rel.begin_initial_dream_pass(wrong_id, source, 1, 0, BTreeSet::from([root]), graph_cut);
    assert!(result.is_err());
    assert!(rel.pending_initial_dream_pass(source).is_none());

    let archived = publish(&mut rel, "archived-source", "extracted", true);
    let archived_id = crate::dream_freshness::initial_pass_id(rel.owner_uuid().unwrap(), archived);
    let graph_cut = rel.graph_version();
    let result = rel.begin_initial_dream_pass(
        archived_id,
        archived,
        1,
        0,
        BTreeSet::from([root]),
        graph_cut,
    );
    assert!(result.is_err());
    assert!(rel.pending_initial_dream_pass(archived).is_none());
}

#[test]
fn runtime_capture_ignores_ranking_and_retry_keeps_the_saved_physical_prefix() {
    let scratch = Scratch::new();
    let path = scratch.0.join("runtime-prefix.rel");
    let mut rel = Cva::create(&path).unwrap();
    let root = publish(&mut rel, "unranked-root", "knowledge", false);
    let source = publish(&mut rel, "runtime-source", "extracted", false);
    let end = rel.container.physical_length().unwrap();
    let cut = rel.memory_version();
    let expected = rel.dream_population_at_prefix(end, cut, source).unwrap();
    assert_eq!(expected, BTreeSet::from([root]));
    // An unavailable ranking candidate grants no birth authority and cannot
    // prevent capturing the complete accepted publication population.
    let pass = crate::dream_freshness::begin_initial_pass(
        &mut rel,
        source,
        [MemoryId([252; 32])].into_iter(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(pass.preexisting_ids, expected);
    publish(&mut rel, "later-birth", "knowledge", false);
    assert_eq!(
        crate::dream_freshness::begin_initial_pass(&mut rel, source, std::iter::empty())
            .unwrap()
            .unwrap(),
        pass
    );
    drop(rel);
    assert_eq!(reopen(&path).pending_initial_dream_pass(source), Some(pass));
}

#[test]
fn destination_transport_proves_exactness_or_preserves_a_literal_exception() {
    let scratch = Scratch::new();
    let path = scratch.0.join("destination.rel");
    let mut rel = Cva::create(&path).unwrap();
    let first = publish(&mut rel, "destination-first", "knowledge", false);
    let second = publish(&mut rel, "destination-second", "knowledge", false);
    let source = publish(&mut rel, "destination-source", "extracted", false);
    let mut pass = crate::InitialDreamPass {
        pass_id: crate::dream_freshness::initial_pass_id(rel.owner_uuid().unwrap(), source),
        source_id: source,
        first_revision: 1,
        turn: 0,
        accepted_settlement_turn: None,
        preexisting_ids: BTreeSet::from([first, second]),
        graph_version_before: rel.graph_version(),
        roots: BTreeSet::new(),
    };
    let compact = rel.current_dream_begin_payload(&pass).unwrap();
    assert!(matches!(
        dream_begin_codec::decode(&compact).unwrap().population,
        BeginPopulation::CompactCut { .. }
    ));
    let object = rel.container.append(&compact).unwrap();
    publish(&mut rel, "after-destination-begin", "knowledge", false);
    assert_eq!(
        rel.materialized_dream_begin(object, &compact).unwrap(),
        Some(pass.clone())
    );
    pass.preexisting_ids = BTreeSet::from([first]);
    let exception = rel.current_dream_begin_payload(&pass).unwrap();
    assert_eq!(
        dream_begin_codec::decode(&exception).unwrap().population,
        BeginPopulation::ExplicitSet(pass.preexisting_ids.clone())
    );
    let object = rel.container.append(&exception).unwrap();
    assert_eq!(
        rel.materialized_dream_begin(object, &exception).unwrap(),
        Some(pass)
    );
}

#[test]
fn fresh_history_rewrite_discards_old_begin_authority_under_its_existing_policy() {
    let scratch = Scratch::new();
    let path = scratch.0.join("rewrite-source.rel");
    let output = scratch.0.join("rewrite-output.rel");
    let mut rel = Cva::create(&path).unwrap();
    let root = publish(&mut rel, "rewrite-root", "knowledge", false);
    let source = publish(&mut rel, "rewrite-source", "extracted", false);
    let pass = crate::dream_freshness::begin_initial_pass(&mut rel, source, std::iter::empty())
        .unwrap()
        .unwrap();
    assert_eq!(pass.preexisting_ids, BTreeSet::from([root]));
    rel.sync().unwrap();
    drop(rel);
    let before = fs::read(&path).unwrap();
    crate::rewrite::rewrite_file(&path, &output, &crate::rewrite::RewriteRequest::default())
        .unwrap();
    let mut rewritten = Cva::open(&output).unwrap();
    assert_ne!(
        rewritten.owner_uuid(),
        Cva::open(&path).unwrap().owner_uuid()
    );
    assert!(rewritten.pending_initial_dream_pass(source).is_none());
    assert!(
        rewritten
            .historical_completed_initial_dream_passes()
            .unwrap()
            .is_empty()
    );
    for id in [root, source] {
        assert_eq!(rewritten.memory(id).unwrap().revision, 1);
    }
    {
        let mut ingest_frame =
            |frame: crate::container::PhysicalFrame| -> Result<(), crate::CvaError> {
                let payload = frame.payload.as_slice();
                {
                    assert!(!payload.starts_with(b"CVDREAM2"));
                    assert!(!(payload.starts_with(b"CVDREAM1") && payload.get(24) == Some(&0)));
                    Ok(())
                }
            };
        let mut cursor = rewritten.container.physical_cursor().unwrap();
        (|| {
            while let Some(frame) = cursor.next_frame()? {
                ingest_frame(frame)?;
            }
            Ok::<(), crate::CvaError>(())
        })()
    }
    .unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn compact_begin_survives_copy_prune_and_reclaim_without_losing_terminal_births() {
    let scratch = Scratch::new();
    let path = scratch.0.join("maintenance.rel");
    let mut rel = Cva::create(&path).unwrap();
    let mut root_draft = draft("maintenance-root", "knowledge", false);
    root_draft.content = "prunable historical Dream root ".repeat(2048);
    let root = rel.publish_memory(None, 0, root_draft).unwrap().0.id;
    let source = publish(&mut rel, "maintenance-source", "extracted", false);
    let pass = crate::dream_freshness::begin_initial_pass(&mut rel, source, std::iter::empty())
        .unwrap()
        .unwrap();
    rel.tombstone_memory(root, 1).unwrap();
    rel.sync().unwrap();
    let original = fs::read(&path).unwrap();
    let copied = scratch.0.join("copied.rel");
    fs::copy(&path, &copied).unwrap();
    assert_eq!(
        Cva::open(&copied)
            .unwrap()
            .pending_initial_dream_pass(source),
        Some(pass.clone())
    );
    let pruned = scratch.0.join("pruned.rel");
    rel.prune_tombstoned_history(
        &pruned,
        crate::HistoryPruneRequest {
            targets: vec![crate::owner_lifecycle::LifecycleTarget::Memory(root)],
        },
    )
    .unwrap();
    let mut pruned_rel = Cva::open(&pruned).unwrap();
    assert_eq!(
        pruned_rel.pending_initial_dream_pass(source),
        Some(pass.clone())
    );
    let reclaimed = scratch.0.join("reclaimed.rel");
    pruned_rel.reclaim_storage(&reclaimed).unwrap();
    assert_eq!(
        Cva::open(&reclaimed)
            .unwrap()
            .pending_initial_dream_pass(source),
        Some(pass)
    );
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn dream_begin_raw_byte_measurement_preserves_exact_synthetic_populations() {
    for count in [0, 1, 2, 64] {
        let scratch = Scratch::new();
        let path = scratch.0.join("measurement.rel");
        let mut rel = Cva::create(&path).unwrap();
        let ids = (0..count)
            .map(|index| {
                publish(
                    &mut rel,
                    &format!("measurement-root-{index}"),
                    "knowledge",
                    false,
                )
            })
            .collect::<BTreeSet<_>>();
        let source = publish(&mut rel, "measurement-source", "extracted", false);
        let pass = crate::InitialDreamPass {
            pass_id: crate::dream_freshness::initial_pass_id(rel.owner_uuid().unwrap(), source),
            source_id: source,
            first_revision: 1,
            turn: 0,
            accepted_settlement_turn: None,
            preexisting_ids: ids,
            graph_version_before: 0,
            roots: BTreeSet::new(),
        };
        let prior_payload_bytes = 89 + pass.pass_id.len() + count * 32;
        let current = rel.current_dream_begin_payload(&pass).unwrap();
        let object = rel.container.append(&current).unwrap();
        assert_eq!(
            rel.materialized_dream_begin(object, &current).unwrap(),
            Some(pass.clone())
        );
        rel.sync().unwrap();
        drop(rel);
        assert_eq!(
            Cva::open(&path).unwrap().pending_initial_dream_pass(source),
            Some(pass.clone())
        );
        println!(
            "Dream Begin population={count}: legacy_payload={} current_payload={} framing_bytes=8",
            prior_payload_bytes,
            current.len()
        );
        assert_eq!(
            current.len(),
            pass.pass_id.len() + if count == 0 { 81 } else { 117 }
        );
    }
}
