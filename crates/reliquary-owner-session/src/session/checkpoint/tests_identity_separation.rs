use super::*;
use crate::model::identity::{
    IdentityEndpointProof, IdentitySeparationAction, IdentitySeparationEvent,
};
use crate::session::identity_receipt::tests::{fixture_entity, unresolved};
use crate::{
    FreshHistoryArchive, FreshHistoryFreshness, FreshHistoryIdentity, FreshHistoryRequest,
    HistoricalProofError, VerifiedOwnerCopyPlan, prune_terminal_entity_descriptions_copy,
    reclaim_storage_copy, reconcile_divergent_rel_copy, rewrite_memory_history_copy,
};

fn publish_comparison(writer: &mut RelWriter) -> IdentitySeparationEvent {
    let entity = fixture_entity(writer, "Mira");
    let receipt = unresolved(OWNER, &entity);
    let left = writer
        .read_session()
        .identity_endpoint_proof(entity.id, None)
        .unwrap();
    let event = IdentitySeparationEvent {
        pair: receipt.pair,
        revision: 1,
        predecessor: None,
        action_id: [11; 16],
        operator_id: "operator-a".into(),
        action: IdentitySeparationAction::Confirm {
            receipt_revision: receipt.revision,
            receipt_fingerprint: receipt.fingerprint,
            left,
            right: IdentityEndpointProof {
                history_generation: [19; 32],
                revision: receipt.right.endpoint_revision,
                digest: [20; 32],
            },
        },
    };
    writer.publish_identity_receipt(receipt).unwrap();
    event
}

fn undo(event: &IdentitySeparationEvent) -> IdentitySeparationEvent {
    IdentitySeparationEvent {
        pair: event.pair,
        revision: 2,
        predecessor: Some(event.revision),
        action_id: [12; 16],
        operator_id: "operator-b".into(),
        action: IdentitySeparationAction::Undo {
            target_revision: event.revision,
        },
    }
}

fn admission() -> GraphAdmission {
    GraphAdmission::for_replay(OWNER, [], [], [], [], [])
}

#[test]
fn identity_separation_proofs_survive_exact_and_suffix_checkpoints() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("separation.rel");
    let cache = directory.path().join("private");
    private_cache::test_directory(&cache).unwrap();
    let slot = cache.join("separation.checkpoint");
    let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
    let confirmation = publish_comparison(&mut writer);
    drop(writer);

    let old = RelSession::open_path_current(&source).unwrap();
    let old_cut = old.current_historical_cut();
    let old_proof = old.historical_proof(OWNER, &old_cut).unwrap();
    assert_eq!(old_cut.separation_version, 0);
    assert!(old.identity_separation(&confirmation.pair, None).is_none());
    drop(old);

    let mut writer = RelWriter::open_path(&source, admission(), [], []).unwrap();
    writer
        .publish_identity_separation(confirmation.clone())
        .unwrap();
    drop(writer);

    let (lease, cold, _) = admit(&source, &slot, true).unwrap();
    let covered = lease.extent().unwrap();
    let confirmed_cut = cold.current_historical_cut();
    let confirmed_proof = cold.historical_proof(OWNER, &confirmed_cut).unwrap();
    assert_eq!(confirmed_cut.separation_version, 1);
    assert_eq!(
        confirmed_proof.retained_records,
        old_proof.retained_records + 1
    );
    assert_ne!(confirmed_proof.digest, old_proof.digest);
    assert_eq!(Entry::open(&slot).unwrap().manifest.separation_cut, 1);
    drop((lease, cold));

    let (lease, exact, _) = admit(&source, &slot, false).unwrap();
    assert_eq!(
        exact.identity_separation(&confirmation.pair, None),
        Some(&confirmation)
    );
    assert_eq!(
        exact.historical_proof(OWNER, &confirmed_cut).unwrap(),
        confirmed_proof
    );
    drop((lease, exact));

    let withdrawal = undo(&confirmation);
    let mut writer = RelWriter::open_path(&source, admission(), [], []).unwrap();
    writer
        .session
        .common
        .memories
        .publish(&mut writer.container, None, 0, draft())
        .unwrap();
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    writer
        .publish_identity_separation(withdrawal.clone())
        .unwrap();
    drop(writer);

    let (lease, restored, checkpoint) = admit(&source, &slot, true).unwrap();
    assert!(lease.extent().unwrap() > covered);
    assert_eq!(checkpoint.unwrap().attempted_extent, Some(covered));
    assert_eq!(
        restored.identity_separation(&confirmation.pair, Some(1)),
        Some(&confirmation)
    );
    assert_eq!(
        restored.identity_separation(&confirmation.pair, None),
        Some(&withdrawal)
    );
    let current_cut = restored.current_historical_cut();
    assert_eq!(current_cut.separation_version, 2);
    let current_proof = restored.historical_proof(OWNER, &current_cut).unwrap();
    assert_eq!(
        restored.historical_proof(OWNER, &old_cut).unwrap(),
        old_proof
    );
    assert_eq!(
        restored.historical_proof(OWNER, &confirmed_cut).unwrap(),
        confirmed_proof
    );

    let mut inconsistent = current_cut.clone();
    inconsistent.identity_version = 0;
    assert!(matches!(
        restored.historical_proof(OWNER, &inconsistent),
        Err(HistoricalProofError::InconsistentCut(_))
    ));
    inconsistent = current_cut.clone();
    inconsistent.owner.memory_version = confirmed_cut.owner.memory_version;
    assert!(matches!(
        restored.historical_proof(OWNER, &inconsistent),
        Err(HistoricalProofError::InconsistentCut(_))
    ));
    inconsistent = current_cut.clone();
    inconsistent.separation_version += 1;
    assert!(matches!(
        restored.historical_proof(OWNER, &inconsistent),
        Err(HistoricalProofError::FutureCut(_))
    ));

    drop((lease, restored));
    let full = RelSession::open_path_current(&source).unwrap();
    assert_eq!(
        full.historical_proof(OWNER, &current_cut).unwrap(),
        current_proof
    );
}

#[test]
fn identity_separation_copies_retain_history_and_refuse_inexact_rewrites() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.rel");
    let prefix = directory.path().join("prefix.rel");
    let extended = directory.path().join("extended.rel");
    let reclaimed = directory.path().join("reclaimed.rel");
    let pruned = directory.path().join("pruned.rel");
    let refused = directory.path().join("refused.rel");

    let mut writer = RelWriter::create_current_path(&source, OWNER).unwrap();
    let confirmation = publish_comparison(&mut writer);
    writer
        .publish_identity_separation(confirmation.clone())
        .unwrap();
    drop(writer);
    VerifiedOwnerCopyPlan::prepare(&source)
        .unwrap()
        .copy_to(&source, &prefix)
        .unwrap();

    let withdrawal = undo(&confirmation);
    let mut writer = RelWriter::open_path(&source, admission(), [], []).unwrap();
    writer
        .publish_identity_separation(withdrawal.clone())
        .unwrap();
    let entity = confirmation.pair.local_entity;
    writer
        .session
        .common
        .perception
        .tombstone_entity(&mut writer.container, entity, 1)
        .unwrap();
    // Force reclamation to remove backing from its copy while retaining the audit chain.
    let orphan = vec![0x5a; 512];
    let mut frame = b"RELFILE1".to_vec();
    frame.extend_from_slice(&Sha256::digest(&orphan));
    frame.extend_from_slice(&(orphan.len() as u32).to_le_bytes());
    frame.extend_from_slice(&orphan);
    writer.container.append(&frame).unwrap();
    writer.container.sync().unwrap();
    drop(writer);
    let original = fs::read(&source).unwrap();

    let (plan, right_wins) =
        VerifiedOwnerCopyPlan::prepare_prefix_reconciliation(&prefix, &source).unwrap();
    assert!(right_wins);
    plan.copy_to(&source, &extended).unwrap();
    assert_eq!(fs::read(&extended).unwrap(), original);
    let baseline = RelSession::open_path_current(&extended).unwrap();
    let cut = baseline.current_historical_cut();
    let proof = baseline.historical_proof(OWNER, &cut).unwrap();
    assert_eq!(
        baseline.identity_separation(&confirmation.pair, Some(1)),
        Some(&confirmation)
    );
    assert_eq!(
        baseline.identity_separation(&confirmation.pair, None),
        Some(&withdrawal)
    );

    let report = reclaim_storage_copy(&plan, &source, &reclaimed, &admission()).unwrap();
    assert_eq!(report.removed_frames, 1);
    let after = RelSession::open_path_current(&reclaimed).unwrap();
    assert_eq!(after.historical_proof(OWNER, &cut).unwrap(), proof);

    prune_terminal_entity_descriptions_copy(&plan, &source, &pruned, &[entity], &admission())
        .unwrap();
    let after = RelSession::open_path_current(&pruned).unwrap();
    assert_eq!(
        after.identity_separation(&confirmation.pair, Some(1)),
        Some(&confirmation)
    );
    assert_eq!(
        after.identity_separation(&confirmation.pair, None),
        Some(&withdrawal)
    );
    assert!(after.entity_description_prune(entity, 1).is_some());

    let request = FreshHistoryRequest {
        source_owner_id: OWNER,
        source_digest: Sha256::digest(&original).into(),
        plan_digest: [1; 32],
        identity: FreshHistoryIdentity::PreserveOwner,
        retained_memory_ids: vec![],
        retain_all_revisions: false,
        retain_revisions_since_memory_version: None,
        retain_derived_state: false,
        archive: FreshHistoryArchive::Omit,
        freshness: FreshHistoryFreshness::Reset,
        profile_ids: vec![],
    };
    let error = rewrite_memory_history_copy(&source, &refused, &request, &admission()).unwrap_err();
    assert!(error.contains("separation confirmations"), "{error}");
    assert!(!refused.exists());

    let mut writer = RelWriter::open_path(&prefix, admission(), [], []).unwrap();
    writer
        .publish_identity_separation(IdentitySeparationEvent {
            action_id: [13; 16],
            ..withdrawal
        })
        .unwrap();
    drop(writer);
    let error = reconcile_divergent_rel_copy(&source, &prefix, &refused, &admission(), &[], &[])
        .unwrap_err();
    assert!(
        error.to_string().contains("separation confirmations"),
        "{error}"
    );
    assert!(!refused.exists());
    assert_eq!(fs::read(&source).unwrap(), original);
}
