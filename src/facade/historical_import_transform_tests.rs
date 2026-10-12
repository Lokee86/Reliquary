//! H7 exercises the real native import/checkpoint owner, not a shadow store.
use super::super::PreparedHistoricalImport;
use super::*;
use crate::{AcceptedMemoryUseReceipt, CvaRelation, MemoryId};
use sha2::{Digest, Sha256};

fn checkpoint(cva: &mut Cva) -> PreparedHistoricalImport {
    let prepared = cva
        .freeze_historical_import()
        .unwrap()
        .prepare(&[])
        .unwrap();
    assert!(cva.commit_historical_import(&prepared).unwrap());
    prepared
}
fn receipt(cva: &Cva, id: MemoryId, name: &str) -> AcceptedMemoryUseReceipt {
    AcceptedMemoryUseReceipt {
        owner_uuid: cva.owner_uuid().unwrap(),
        accepted_turn: cva.rel_turn_count(),
        use_id: name.into(),
        memories: vec![id],
    }
}
fn assert_preserved(
    source: &mut Cva,
    copy: &mut Cva,
    id: MemoryId,
    prepared: &PreparedHistoricalImport,
) {
    source.validate_historical_import_copy(copy).unwrap();
    assert_eq!(copy.owner_uuid(), source.owner_uuid());
    assert_eq!(
        copy.freshness_records_at(copy.rel_turn_count()).unwrap(),
        source
            .freshness_records_at(source.rel_turn_count())
            .unwrap()
    );
    assert_eq!(copy.memory(id).unwrap(), source.memory(id).unwrap());
    let pass = crate::dream_freshness::initial_pass_id(source.owner_uuid().unwrap(), id);
    assert!(copy.initial_dream_pass_settled(&pass));
    assert!(!copy.commit_historical_import(prepared).unwrap());
    let retried = copy
        .freeze_historical_import()
        .unwrap()
        .prepare(&[])
        .unwrap();
    assert!(!copy.commit_historical_import(&retried).unwrap());
}

#[test]
fn historical_transform_copy_prefix_extension_and_promotion_preserve_live_authority() {
    let (path, mut cva, id) = historical_import_fixture("h7-copy.rel");
    cva.sync().unwrap();
    let provisional = path.with_file_name("provisional.rel");
    fs::copy(&path, &provisional).unwrap();
    let prepared = checkpoint(&mut cva);
    let checkpoint_only = path.with_file_name("checkpoint-only.rel");
    fs::copy(&path, &checkpoint_only).unwrap();
    let exact = path.with_file_name("exact.rel");
    fs::copy(&path, &exact).unwrap();
    assert_preserved(&mut cva, &mut Cva::open(&exact).unwrap(), id, &prepared);
    let merged = path.with_file_name("prefix.rel");
    let result = Cva::reconcile(&provisional, &path, &merged).unwrap();
    assert_eq!(result.comparison.relation, CvaRelation::RightExtendsLeft);
    assert_preserved(&mut cva, &mut Cva::open(&merged).unwrap(), id, &prepared);
    // A real later turn may have an earlier timestamp. It cannot reorder the
    // checkpoint's already accepted source prefix or move its numerical anchors.
    cva.append_node(
        "later".into(),
        "live".into(),
        None,
        "user".into(),
        -100,
        "Legitimate post-checkpoint activity.",
    )
    .unwrap();
    let used = receipt(&cva, id, "h7-live-use");
    assert_eq!(cva.record_accepted_memory_use(used.clone(), 1).unwrap(), 1);
    let extension = path.with_file_name("extension.rel");
    Cva::reconcile(&checkpoint_only, &path, &extension).unwrap();
    let mut copy = Cva::open(&extension).unwrap();
    assert_preserved(&mut cva, &mut copy, id, &prepared);
    assert_eq!(copy.record_accepted_memory_use(used.clone(), 1).unwrap(), 0);
    let again = path.with_file_name("twice.rel");
    let result = Cva::reconcile(&extension, &path, &again).unwrap();
    assert_eq!(result.comparison.relation, CvaRelation::Identical);
    assert_preserved(&mut cva, &mut Cva::open(&again).unwrap(), id, &prepared);
    drop(copy);
    let result = Cva::reconcile_and_promote(&checkpoint_only, &path).unwrap();
    assert!(result.canonical_change_required);
    let mut promoted = Cva::open(&checkpoint_only).unwrap();
    assert_preserved(&mut cva, &mut promoted, id, &prepared);
    assert_eq!(promoted.record_accepted_memory_use(used, 1).unwrap(), 0);
}

#[test]
fn historical_transform_reclaim_and_attachment_repack_preserve_checkpoint_and_receipts() {
    for repack in [false, true] {
        let (path, mut cva, id) = historical_import_fixture("h7-physical.rel");
        let bytes = vec![42; 64 * 1024];
        if repack {
            let file = cva.store_file("old.bin".into(), None, &bytes).unwrap();
            cva.bind_legacy_project_file(
                file.id,
                crate::ProjectFileRef {
                    revision: crate::ProjectRevisionRef {
                        repository: crate::ProjectRepositoryRef {
                            kind: crate::ProjectRepositoryKind::Lore,
                            repository_id: "h7-repository".into(),
                            project_path: ".".into(),
                        },
                        revision: "verified-revision".into(),
                    },
                    path: "uploads/old.bin".into(),
                    content_hash: Some(Sha256::digest(&bytes).into()),
                },
            )
            .unwrap();
        } else {
            cva.container
                .append(
                    &crate::archive_codec::encode_content(
                        crate::archive_store::hash_content(&bytes),
                        &bytes,
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        let prepared = checkpoint(&mut cva);
        let used = receipt(&cva, id, "h7-physical-use");
        cva.record_accepted_memory_use(used.clone(), 1).unwrap();
        let original = fs::read(&path).unwrap();
        let output = path.with_file_name("compact.rel");
        if repack {
            assert_eq!(
                cva.repack_project_backed_attachments(&output)
                    .unwrap()
                    .dropped_content_objects,
                1
            );
        } else {
            assert_eq!(
                cva.reclaim_storage(&output).unwrap().orphan_content_chunks,
                1
            );
        }
        assert!(fs::metadata(&output).unwrap().len() < original.len() as u64);
        assert_eq!(fs::read(&path).unwrap(), original);
        let mut copy = Cva::open(&output).unwrap();
        assert_preserved(&mut cva, &mut copy, id, &prepared);
        let before_retry = fs::read(&output).unwrap();
        assert_eq!(copy.record_accepted_memory_use(used, 1).unwrap(), 0);
        assert_eq!(fs::read(&output).unwrap(), before_retry);
    }
}

#[test]
fn historical_transform_detects_receipt_loss_hidden_by_score_saturation() {
    let (path, mut cva, id) = historical_import_fixture("h7-receipt-loss.rel");
    checkpoint(&mut cva);
    cva.record_accepted_memory_use(receipt(&cva, id, "first"), 1)
        .unwrap();
    let prefix = fs::read(&path).unwrap();
    cva.record_accepted_memory_use(receipt(&cva, id, "lost-proof"), 1)
        .unwrap();
    let truncated = path.with_file_name("lost-proof.rel");
    fs::write(&truncated, prefix).unwrap();
    let copy = Cva::open(&truncated).unwrap();
    assert_eq!(
        cva.freshness_records_at(32).unwrap(),
        copy.freshness_records_at(32).unwrap()
    );
    assert_eq!(
        cva.freshness.import_checkpoint(),
        copy.freshness.import_checkpoint()
    );
    assert!(
        cva.validate_historical_import_copy(&copy)
            .unwrap_err()
            .to_string()
            .contains("receipt/proof authority")
    );
}

#[test]
fn historical_transform_divergent_reorder_rejects_before_output_or_promotion() {
    let (path, mut left, id) = historical_import_fixture("h7-divergent.rel");
    let prepared = checkpoint(&mut left);
    let right_path = path.with_file_name("right.rel");
    fs::copy(&path, &right_path).unwrap();
    let mut right = Cva::open(&right_path).unwrap();
    left.append_node(
        "left".into(),
        "left".into(),
        None,
        "user".into(),
        500,
        "Left.",
    )
    .unwrap();
    right
        .append_node(
            "right".into(),
            "right".into(),
            None,
            "user".into(),
            400,
            "Right.",
        )
        .unwrap();
    let left_bytes = fs::read(&path).unwrap();
    let right_bytes = fs::read(&right_path).unwrap();
    let output = path.with_file_name("unsupported.rel");
    assert_eq!(
        Cva::compare(&path, &right_path).unwrap().relation,
        CvaRelation::Diverged
    );
    assert!(
        Cva::reconcile(&path, &right_path, &output)
            .unwrap_err()
            .to_string()
            .contains("source rebasing")
    );
    assert!(!output.exists());
    assert!(Cva::reconcile_and_promote(&path, &right_path).is_err());
    assert_eq!(fs::read(&path).unwrap(), left_bytes);
    assert_eq!(fs::read(&right_path).unwrap(), right_bytes);
    assert_preserved(&mut left, &mut Cva::open(&path).unwrap(), id, &prepared);
}

#[test]
fn historical_transform_conflicting_manifest_and_policy_never_overwrite_acceptance() {
    let (path, mut left, _) = historical_import_fixture("h7-conflict.rel");
    left.sync().unwrap();
    let right_path = path.with_file_name("right.rel");
    fs::copy(&path, &right_path).unwrap();
    let mut right = Cva::open(&right_path).unwrap();
    let prepared = checkpoint(&mut left);
    right
        .store_file(
            "changed-input.bin".into(),
            None,
            b"changed retained owner input",
        )
        .unwrap();
    let other = right
        .freeze_historical_import()
        .unwrap()
        .prepare(&[])
        .unwrap();
    right.commit_historical_import(&other).unwrap();
    assert_eq!(
        left.freshness.import_checkpoint().unwrap().import_id,
        right.freshness.import_checkpoint().unwrap().import_id
    );
    let original = fs::read(&path).unwrap();
    let output = path.with_file_name("conflict.rel");
    assert!(
        Cva::reconcile(&path, &right_path, &output)
            .unwrap_err()
            .to_string()
            .contains("ImportManifestConflict")
    );
    assert!(!output.exists());
    assert!(left.commit_historical_import(&other).is_err());
    let mut policy = left.freshness_policy();
    policy.decay_turns_per_point = 11;
    assert!(left.set_freshness_policy_before_enrollment(policy).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(!left.commit_historical_import(&prepared).unwrap());
    // A valid same-owner replica may have pinned a different base policy before
    // enrollment; checkpoint reconciliation must reject it before replay.
    let policy_path = path.with_file_name("other-policy.rel");
    let mut other_policy =
        Cva::create_rel_with_uuid(&policy_path, left.owner_uuid().unwrap(), None).unwrap();
    other_policy
        .set_freshness_policy_before_enrollment(policy)
        .unwrap();
    let policy_output = path.with_file_name("policy-conflict.rel");
    assert!(
        Cva::reconcile(&path, &policy_path, &policy_output)
            .unwrap_err()
            .to_string()
            .contains("owner policy conflict")
    );
    assert!(!policy_output.exists());
    let migrated = path.with_file_name("migration.rel");
    assert!(matches!(
        crate::migrate_file(&path, &migrated),
        Err(crate::MigrationError::AlreadyCurrent)
    ));
    assert!(!migrated.exists());
}

#[test]
fn historical_transform_unverified_corpus_owner_or_policy_cannot_transplant_numeric_anchors() {
    let (path, mut cva, _) = historical_import_fixture("h7-transplant.rel");
    cva.sync().unwrap();
    let prefix = fs::read(&path).unwrap();
    checkpoint(&mut cva);
    let cp = cva.freshness.import_checkpoint().unwrap().clone();
    for case in 0..3 {
        let candidate_path = path.with_file_name(format!("transplant-{case}.rel"));
        fs::write(&candidate_path, &prefix).unwrap();
        let mut candidate = Cva::open(&candidate_path).unwrap();
        let mut altered = cp.clone();
        if case == 0 {
            altered.corpus_digest = [9; 32];
        }
        if case == 1 {
            altered.owner = [9; 16];
        }
        altered.import_id =
            crate::freshness_storage::historical_import_id(altered.owner, altered.corpus_digest);
        let mut payload = altered.encode();
        if case == 2 {
            // Keep valid framing/checksum while selecting an unsupported H policy.
            payload[8 + 16 + 4 * 32] ^= 1;
            let end = payload.len() - 32;
            let checksum = Sha256::digest(&payload[..end]);
            payload[end..].copy_from_slice(&checksum);
        }
        candidate.container.append(&payload).unwrap();
        candidate.sync().unwrap();
        drop(candidate);
        let before = fs::read(&candidate_path).unwrap();
        let error = Cva::open(&candidate_path).err().unwrap().to_string();
        assert!(
            error.contains(match case {
                0 => "corpus identity",
                1 => "owner/base policy",
                _ => "reconstruction policy",
            }),
            "{error}"
        );
        let output = path.with_file_name(format!("rejected-transplant-{case}.rel"));
        assert!(Cva::reconcile(&path, &candidate_path, &output).is_err());
        assert!(!output.exists());
        assert_eq!(fs::read(&candidate_path).unwrap(), before);
    }
}

#[test]
fn historical_transform_regenerated_archive_restores_accepted_state_without_reconstruction() {
    let (path, mut cva, id) = historical_import_fixture("h7-regenerated.rel");
    let prepared = checkpoint(&mut cva);
    let record = cva.freshness_record(id).unwrap();
    assert_eq!(record.score, 99);
    let original = fs::read(&path).unwrap();
    cva.append_node(
        "u0".into(),
        "c1".into(),
        None,
        "user".into(),
        10,
        "Atomic Insomnia completion fact 0.",
    )
    .unwrap();
    for episode in cva.episodes() {
        cva.queue_insomnia_episode(episode.id, InsomniaPriority::Import, 400)
            .unwrap();
    }
    assert!(
        cva.claim_insomnia_episode("regenerator", 401, 1000)
            .unwrap()
            .is_none()
    );
    crate::dream_freshness::settle_initial_pass(&mut cva, id, true, 402).unwrap();
    let retried = cva
        .freeze_historical_import()
        .unwrap()
        .prepare(&[])
        .unwrap();
    assert!(!cva.commit_historical_import(&retried).unwrap());
    assert!(!cva.commit_historical_import(&prepared).unwrap());
    assert_eq!(cva.freshness_record(id), Some(record));
    assert_eq!(cva.rel_turn_count(), 32);
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn historical_transform_principal_conversion_rejects_but_unchanged_repack_preserves() {
    let principal = "phy-11111111-1111-4111-8111-111111111111";
    let (path, mut cva, _) = historical_import_fixture("h7-principal-reject.rel");
    checkpoint(&mut cva);
    let original = fs::read(&path).unwrap();
    let output = path.with_file_name("rejected.rel");
    assert!(
        cva.repack_missing_user_principal(&output, principal)
            .unwrap_err()
            .to_string()
            .contains("source-identity conversion")
    );
    assert!(!output.exists());
    assert_eq!(fs::read(&path).unwrap(), original);
    let (path, mut cva, id) = historical_import_fixture("h7-principal-no-change.rel");
    let identified = path.with_file_name("identified.rel");
    assert_eq!(
        cva.repack_missing_user_principal(&identified, principal)
            .unwrap()
            .rewritten_user_turns,
        16
    );
    let mut cva = Cva::open(&identified).unwrap();
    let prepared = checkpoint(&mut cva);
    let output = path.with_file_name("unchanged.rel");
    let result = cva
        .repack_missing_user_principal(&output, principal)
        .unwrap();
    assert_eq!(result.rewritten_user_turns, 0);
    assert_eq!(result.preserved_user_turns, 16);
    assert_preserved(&mut cva, &mut Cva::open(&output).unwrap(), id, &prepared);
}
