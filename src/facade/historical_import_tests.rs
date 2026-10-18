use super::*;
use crate::insomnia::completion_tests::{process, setup, setup_origin};
use crate::{EpisodeBoundary, EpisodeConfig, EpisodeOrigin, InsomniaPriority};
use std::fs;

#[path = "historical_embedding_tests.rs"]
mod historical_embedding;
#[path = "historical_embedding_evidence_tests.rs"]
mod historical_embedding_evidence;
#[path = "historical_embedding_import_tests.rs"]
mod historical_embedding_import;
#[path = "historical_events_tests.rs"]
mod historical_events;
#[path = "historical_manifest_tests.rs"]
mod historical_manifest;
#[path = "historical_matrix_tests.rs"]
mod historical_matrix;
#[path = "historical_recovery_tests.rs"]
mod historical_recovery;
#[path = "historical_retained_corpus_tests.rs"]
mod historical_retained_corpus;
#[path = "historical_import_transform_tests.rs"]
mod historical_transform;

fn historical_import_fixture(name: &str) -> (std::path::PathBuf, Cva, crate::MemoryId) {
    let (path, cva, ids) = historical_import_fixture_count(name, 1);
    (path, cva, ids[0])
}
fn historical_import_fixture_count(
    name: &str,
    count: usize,
) -> (std::path::PathBuf, Cva, Vec<crate::MemoryId>) {
    historical_import_fixture_clock(name, count, false)
}
fn historical_import_fixture_clock(
    name: &str,
    count: usize,
    regress: bool,
) -> (std::path::PathBuf, Cva, Vec<crate::MemoryId>) {
    let (path, mut cva, _) = setup_origin(name, EpisodeOrigin::Import);
    let ids: Vec<_> = process(&mut cva, count, 110)
        .created
        .iter()
        .map(|memory| memory.id)
        .collect();
    // A completed omission-only Episode supplies a real historical decay tail.
    let mut parent = None;
    for index in 0..8 {
        let user = format!("u{index}");
        let assistant = format!("a{index}");
        cva.append_node(
            user.clone(),
            "c2".into(),
            parent,
            "user".into(),
            if regress {
                50 - index * 2
            } else {
                100 + index * 2
            },
            "Omitted historical activity.",
        )
        .unwrap();
        cva.append_node(
            assistant.clone(),
            "c2".into(),
            Some(user),
            "assistant".into(),
            if regress {
                49 - index * 2
            } else {
                101 + index * 2
            },
            "Acknowledged.",
        )
        .unwrap();
        parent = Some(assistant);
    }
    let episode = cva
        .materialize_path_episodes(
            "c2",
            parent.as_deref().unwrap(),
            EpisodeConfig::default(),
            EpisodeOrigin::Import,
            Some((EpisodeBoundary::Inactivity, 200)),
        )
        .unwrap()
        .created
        .remove(0);
    cva.queue_insomnia_episode(episode.id, InsomniaPriority::Import, 201)
        .unwrap();
    assert!(process(&mut cva, 0, 210).created.is_empty());
    for &id in &ids {
        let pass = crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), id);
        cva.begin_initial_dream_pass(
            pass.clone(),
            id,
            1,
            cva.rel_turn_count(),
            Default::default(),
            cva.graph_version(),
        )
        .unwrap();
        cva.reconcile_dream_lifecycle(id).unwrap();
        cva.accept_initial_dream_settlement(&pass, cva.rel_turn_count())
            .unwrap();
        crate::dream_freshness::settle_initial_pass(&mut cva, id, true, 300).unwrap();
    }
    for conversation in ["c1", "c2"] {
        cva.materialize_path_fragments(
            conversation,
            "a7",
            crate::FragmentConfig {
                turns: 2,
                overlap: 0,
            },
            true,
        )
        .unwrap();
    }
    (path, cva, ids)
}

#[test]
fn physical_import_base_includes_prior_settlement_after_its_frozen_input_prefix() {
    let (path, mut cva, _) = historical_import_fixture("epoch-base-prior-settlement.rel");
    let prepared = cva
        .freeze_historical_import()
        .unwrap()
        .prepare(&[])
        .unwrap();
    cva.commit_historical_import(&prepared).unwrap();
    cva.import_native(
        &crate::imports::NativeImport {
            schema_version: 1,
            source_namespace: "test-base".into(),
            source_format: "test".into(),
            conversations: Vec::new(),
        },
        400,
    )
    .unwrap();
    let scope = cva.import_epoch_scope().unwrap();
    assert!(scope.start_offset > prepared.settlement.input_bytes);
    assert_eq!(
        cva.historical_import_base(&scope).unwrap(),
        prepared.settlement.records
    );
    drop(cva);
    fs::remove_file(path).unwrap();
}

#[test]
fn open_epoch_never_retries_an_earlier_import_settlement() {
    let (path, mut cva, _) = historical_import_fixture("epoch-retry-selection.rel");
    let prepared = cva
        .freeze_historical_import()
        .unwrap()
        .prepare(&[])
        .unwrap();
    cva.commit_historical_import(&prepared).unwrap();
    cva.import_native(
        &crate::imports::NativeImport {
            schema_version: 1,
            source_namespace: "retry-selection".into(),
            source_format: "test".into(),
            conversations: Vec::new(),
        },
        400,
    )
    .unwrap();
    cva.container.sync().unwrap();
    let before = fs::read(&path).unwrap();
    // The new empty epoch has its own settlement, even at the same activity cut.
    let scope = cva.import_epoch_scope().unwrap();
    let frozen = cva.freeze_historical_import().unwrap();
    assert!(frozen.epoch);
    assert_ne!(frozen.import_id(), prepared.settlement.import_id);
    assert_eq!(frozen.settlement.start_evidence_digest, scope.start_digest);
    assert!(frozen.settlement.cohort.is_empty());
    let report = cva
        .reconstruct_historical_freshness(Default::default())
        .unwrap();
    assert!(report.committed);
    assert_eq!(report.import_id, frozen.import_id());
    assert_eq!(report.reconstructed_memories, 0);
    assert_ne!(fs::read(&path).unwrap(), before);
    assert_eq!(cva.freshness.import_settlements().len(), 2);
    assert_eq!(cva.freshness.import_settlements()[0], prepared.settlement);
    let accepted = fs::read(&path).unwrap();
    assert!(
        !cva.reconstruct_historical_freshness(Default::default())
            .unwrap()
            .committed
    );
    assert_eq!(fs::read(&path).unwrap(), accepted);
    drop(cva);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.freshness.import_settlements().len(), 2);
    drop(reopened);
    fs::remove_file(path).unwrap();
}

#[test]
fn legacy_cvafrs03_settlement_reopens_with_real_import_birth_evidence() {
    let (path, mut cva, id) = historical_import_fixture("legacy-settlement-reopen.rel");
    let prepared = cva
        .freeze_historical_import()
        .unwrap()
        .prepare(&[])
        .unwrap();
    let mut expected = prepared.settlement.clone();
    let legacy = crate::freshness_storage::encode_legacy_test_fixture(&expected);
    assert!(legacy.starts_with(b"CVAFRS03"));
    cva.container.append(&legacy).unwrap();
    cva.sync().unwrap();
    let persisted = fs::read(&path).unwrap();
    drop(cva);

    let reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.freshness.import_settlements().len(), 1);
    let actual = reopened.freshness.latest_import_settlement().unwrap();
    assert_eq!(actual.base_cut, 0);
    assert_ne!(actual.start_evidence_digest, [0; 32]);
    expected.start_evidence_digest = actual.start_evidence_digest;
    assert_eq!(actual, &expected);
    assert_eq!(reopened.freshness_record(id), Some(expected.records[&id]));
    assert!(actual.encode().starts_with(b"CVAFRS04"));
    assert_eq!(fs::read(&path).unwrap(), persisted);
}

#[test]
fn historical_import_settlement_reopens_and_retries_without_erasing_live_use() {
    let (path, mut cva, id) = historical_import_fixture("historical-checkpoint.cva");
    let original_revision = cva.memory(id).unwrap().revision;
    assert_eq!(cva.freshness_record(id).unwrap().score, 100);
    let pre_settlement_projection = cva.freshness_record_at(id, 31).unwrap();
    let frozen = cva.freeze_historical_import().unwrap();
    assert_eq!(frozen.cut(), 32);
    assert_eq!(frozen.initial_records()[&id].admitted_at_turn, Some(16));
    let prepared = frozen.prepare(&[]).unwrap();
    assert_eq!(prepared.records()[&id].score, 99);
    assert!(cva.commit_historical_import(&prepared).unwrap());
    assert_eq!(cva.freshness_record(id), Some(prepared.records()[&id]));
    assert_eq!(cva.memory(id).unwrap().revision, original_revision);
    let pass = crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), id);
    assert!(cva.initial_dream_pass_settled(&pass));
    let before = fs::read(&path).unwrap();
    assert!(!cva.commit_historical_import(&prepared).unwrap());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(
        cva.freshness_record_at(id, 31).unwrap(),
        pre_settlement_projection
    );
    drop(cva);
    let mut cva = Cva::open(&path).unwrap();
    assert_eq!(
        cva.freshness_record_at(id, 32).unwrap(),
        Some(prepared.records()[&id])
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    // Same-cut live receipts belong after the accepted checkpoint.
    let receipt = crate::AcceptedMemoryUseReceipt {
        owner_uuid: cva.owner_uuid().unwrap(),
        accepted_turn: 32,
        use_id: "after-checkpoint".into(),
        memories: vec![id],
    };
    assert_eq!(
        cva.record_accepted_memory_use(receipt.clone(), 301)
            .unwrap(),
        1
    );
    let live = cva.freshness_record(id).unwrap();
    assert_eq!(live.score, 100);
    assert!(!cva.commit_historical_import(&prepared).unwrap());
    assert_eq!(cva.freshness_record(id), Some(live));
    let retried = cva
        .freeze_historical_import()
        .unwrap()
        .prepare(&[])
        .unwrap();
    assert!(!cva.commit_historical_import(&retried).unwrap());
    drop(cva);
    let mut cva = Cva::open(&path).unwrap();
    assert_eq!(cva.freshness_record_at(id, 32).unwrap(), Some(live));
    assert_eq!(cva.record_accepted_memory_use(receipt, 302).unwrap(), 0);
    assert!(!cva.commit_historical_import(&prepared).unwrap());
}

#[test]
fn historical_import_freeze_detects_input_drift_before_any_projection_switch() {
    let (path, mut cva, id) = historical_import_fixture("historical-checkpoint-drift.cva");
    let prepared = cva
        .freeze_historical_import()
        .unwrap()
        .prepare(&[])
        .unwrap();
    let original = cva.freshness_record(id);
    cva.append_node(
        "later".into(),
        "live".into(),
        None,
        "user".into(),
        500,
        "Post-freeze activity.",
    )
    .unwrap();
    let before = fs::read(&path).unwrap();
    assert!(
        cva.commit_historical_import(&prepared)
            .unwrap_err()
            .to_string()
            .contains("ImportInputDrift")
    );
    assert_eq!(cva.freshness_record(id), original);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(cva.freeze_historical_import().is_err());
    drop(cva);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.freshness_record(id), original);
}

#[test]
fn historical_import_rejects_live_completion_and_unfinished_generation() {
    let (_, mut live, _) = setup("historical-checkpoint-live.cva");
    process(&mut live, 1, 110);
    assert!(live.freeze_historical_import().is_err());
    let (_, mut unfinished, _) =
        setup_origin("historical-checkpoint-pending.cva", EpisodeOrigin::Import);
    assert!(unfinished.freeze_historical_import().is_err());
}

#[test]
fn historical_embedding_fill_unavailable_preserves_import_owner_and_file() {
    let (path, mut cva, id) = historical_import_fixture("historical-fill-unavailable.rel");
    cva.sync().unwrap();
    let before = fs::read(&path).unwrap();
    let record = cva.freshness_record(id);
    assert!(matches!(
        cva.reconstruct_historical_freshness(HistoricalReconstructionOptions {
            fill_missing_embeddings: true
        }),
        Err(HistoricalReconstructionError::EmbeddingFillUnavailable)
    ));
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(cva.freshness_record(id), record);
    assert!(cva.freshness.import_settlements().is_empty());
}

#[test]
fn historical_reconstruction_rejects_unfinished_owner_without_writes() {
    let (path, mut cva, _) =
        setup_origin("historical-coordinator-pending.rel", EpisodeOrigin::Import);
    cva.sync().unwrap();
    let before = fs::read(&path).unwrap();
    assert!(matches!(
        cva.reconstruct_historical_freshness(HistoricalReconstructionOptions::default()),
        Err(HistoricalReconstructionError::Owner(_))
    ));
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(cva.freshness.import_settlements().is_empty());
}

#[test]
fn historical_manifest_is_complete_owner_derived_and_reopen_stable() {
    let (path, mut cva, _) = historical_import_fixture("h1-complete.rel");
    cva.sync().unwrap();
    let before = fs::read(&path).unwrap();
    let a = manifest::build(&mut cva).unwrap();
    let b = manifest::build(&mut cva).unwrap();
    assert_eq!(a.digest, b.digest);
    assert_eq!(a.value, b.value);
    assert_eq!(
        a.digest,
        a.value.digest("reliquary:historical-manifest:v1").unwrap()
    );
    let encoding::Value::Map(fields) = &a.value else {
        panic!()
    };
    let actual: BTreeSet<_> = fields.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        actual,
        BTreeSet::from([
            "schema_version",
            "policy_version",
            "owner_uuid",
            "import_id",
            "corpus_digest",
            "archive_cut",
            "source_records",
            "memory_records",
            "cohort_records",
            "graph_proof",
            "community_proof",
            "vector_proof",
            "scoring_policy",
            "replacement_proof",
            "input_token"
        ])
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    for (index, _) in fields.iter().enumerate() {
        let mut changed = a.value.clone();
        let encoding::Value::Map(fields) = &mut changed else {
            panic!()
        };
        fields[index].1 = encoding::Value::Null;
        assert_ne!(
            a.digest,
            changed.digest("reliquary:historical-manifest:v1").unwrap()
        );
    }
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(a.digest, manifest::build(&mut reopened).unwrap().digest);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn historical_manifest_pins_unknown_community_and_rejects_malformed_present_state() {
    use crate::{Community, CommunityId, CommunitySnapshot};
    let id = MemoryId([1; 32]);
    let ids = BTreeSet::from([id]);
    let snapshot = CommunitySnapshot {
        generation: 1,
        derived_graph_version: 9,
        algorithm_version: 2,
        seed: 1,
        resolution: 1.0,
        quality: 0.0,
        communities: vec![Community {
            id: CommunityId([2; 32]),
            members: vec![id],
        }],
    };
    let validated = manifest::community_proof(Some(snapshot.clone()), 9, &ids).unwrap();
    let stale = manifest::community_proof(Some(snapshot.clone()), 10, &ids).unwrap();
    assert_ne!(validated, stale);
    let mut duplicate = snapshot.clone();
    duplicate.communities[0].members.push(id);
    assert!(manifest::community_proof(Some(duplicate), 10, &ids).is_err());
    let mut foreign = snapshot.clone();
    foreign.communities[0].members[0] = MemoryId([9; 32]);
    assert!(manifest::community_proof(Some(foreign), 9, &ids).is_err());
    let mut nonfinite = snapshot;
    nonfinite.quality = f64::NAN;
    assert!(manifest::community_proof(Some(nonfinite), 10, &ids).is_err());
}

#[test]
fn historical_manifest_rejects_legitimate_use_without_writing() {
    let (path, mut cva, id) = historical_import_fixture("h1-use-isolation.rel");
    cva.record_accepted_memory_use(
        crate::AcceptedMemoryUseReceipt {
            owner_uuid: cva.owner_uuid().unwrap(),
            accepted_turn: cva.rel_turn_count(),
            use_id: "legitimate-use-before-reconstruction".into(),
            memories: vec![id],
        },
        401,
    )
    .unwrap();
    let before = fs::read(&path).unwrap();
    assert!(
        manifest::build(&mut cva)
            .err()
            .unwrap()
            .to_string()
            .contains("accepted use")
    );
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn historical_native_reconstruction_uses_accepted_clock_despite_timestamp_regressions() {
    let (path, mut cva, ids) = historical_import_fixture_clock("n4-accepted-clock.rel", 1, true);
    historical_events::vectors(&mut cva);
    let timeline = cva.historical_freshness_timeline().unwrap();
    let positions = timeline.activity_order.clone();
    let report = cva
        .reconstruct_historical_freshness(Default::default())
        .unwrap();
    let scores = cva.freshness_record(ids[0]);
    assert!(report.committed);
    cva.sync().unwrap();
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.historical_freshness_timeline().unwrap(), timeline);
    for (i, key) in positions.iter().enumerate() {
        assert_eq!(
            reopened.activity_position_for_turn(&key.conversation_id, &key.node_id),
            Some(i as u64 + 1)
        );
    }
    assert_eq!(reopened.freshness_record(ids[0]), scores);
    let bytes = std::fs::read(&path).unwrap();
    reopened
        .reconstruct_historical_freshness(Default::default())
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
