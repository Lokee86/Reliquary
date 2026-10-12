use super::*;
use crate::insomnia::completion_tests::{process, setup, setup_origin};
use crate::{EpisodeBoundary, EpisodeConfig, EpisodeOrigin, InsomniaPriority};
use std::fs;

#[path = "historical_import_transform_tests.rs"]
mod historical_transform;

fn historical_import_fixture(name: &str) -> (std::path::PathBuf, Cva, crate::MemoryId) {
    let (path, mut cva, _) = setup_origin(name, EpisodeOrigin::Import);
    let id = process(&mut cva, 1, 110).created[0].id;
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
            100 + index * 2,
            "Omitted historical activity.",
        )
        .unwrap();
        cva.append_node(
            assistant.clone(),
            "c2".into(),
            Some(user),
            "assistant".into(),
            101 + index * 2,
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
    (path, cva, id)
}

#[test]
fn historical_import_checkpoint_reopens_and_retries_without_erasing_live_use() {
    let (path, mut cva, id) = historical_import_fixture("historical-checkpoint.cva");
    let original_revision = cva.memory(id).unwrap().revision;
    assert_eq!(cva.freshness_record(id).unwrap().score, 100);
    let frozen = cva.freeze_historical_import([7; 32]).unwrap();
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
    assert!(cva.freshness_record_at(id, 31).is_err());
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
    assert!(cva.freeze_historical_import([8; 32]).is_err());
    let retried = cva
        .freeze_historical_import([7; 32])
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
        .freeze_historical_import([7; 32])
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
    assert!(cva.freeze_historical_import([7; 32]).is_err());
    drop(cva);
    let reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.freshness_record(id), original);
}

#[test]
fn historical_import_rejects_live_completion_and_unfinished_generation() {
    let (_, mut live, _) = setup("historical-checkpoint-live.cva");
    process(&mut live, 1, 110);
    assert!(live.freeze_historical_import([7; 32]).is_err());
    let (_, mut unfinished, _) =
        setup_origin("historical-checkpoint-pending.cva", EpisodeOrigin::Import);
    assert!(unfinished.freeze_historical_import([7; 32]).is_err());
}

#[test]
fn historical_reconstruction_unavailable_preserves_import_owner_and_file() {
    let (path, mut cva, id) = historical_import_fixture("historical-coordinator-cut.rel");
    cva.sync().unwrap();
    let before = fs::read(&path).unwrap();
    let record = cva.freshness_record(id);
    let versions = (
        cva.latest_global_version(),
        cva.archive_version(),
        cva.graph_version(),
    );
    for fill_missing_embeddings in [false, true] {
        let error = cva
            .reconstruct_historical_freshness(HistoricalReconstructionOptions {
                fill_missing_embeddings,
            })
            .unwrap_err();
        if fill_missing_embeddings {
            assert!(matches!(
                error,
                HistoricalReconstructionError::EmbeddingFillUnavailable
            ));
        } else {
            assert!(matches!(
                error,
                HistoricalReconstructionError::PipelineUnavailable
            ));
        }
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(cva.freshness_record(id), record);
        assert_eq!(
            (
                cva.latest_global_version(),
                cva.archive_version(),
                cva.graph_version()
            ),
            versions
        );
        assert!(cva.freshness.import_checkpoint().is_none());
    }
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
    assert!(cva.freshness.import_checkpoint().is_none());
}
