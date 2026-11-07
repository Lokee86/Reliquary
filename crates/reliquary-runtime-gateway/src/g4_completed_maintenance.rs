//! G4 maintenance proof on an independently completed native cohort.

#[cfg(test)]
use reliquary_application::{
    ArchiveRewriteMode, DerivedStateRewriteMode, FreshnessRewriteMode, RewriteIdentityMode,
    RewritePolicy, execute_memory_rewrite, plan_phy_rewrite, plan_rel_rewrite, reconcile_rel_owner,
    reconcile_rel_prefix, verify_rewrite_source,
};
#[cfg(test)]
use reliquary_archive::{Branch, TurnInput};
#[cfg(test)]
use reliquary_container::Container;
#[cfg(test)]
use reliquary_owner_session::model::CompatibilityProfileId;
#[cfg(test)]
use reliquary_owner_session::{
    GraphAdmission, PhySession, RelSession, RelWriter, VerifiedOwnerCopyPlan,
    execute_profile_repair, plan_profile_repair, prune_terminal_archive_copy, reclaim_storage_copy,
};
#[cfg(test)]
use serde_json::{Value, json};
#[cfg(test)]
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::{fs, path::Path};

/// Runs maintenance against the four owners after cohort Gateway leases have
/// been dropped. Outputs are isolated beneath `root`; cohort sources remain
/// byte-for-byte unchanged.
#[cfg(test)]
pub(crate) fn verify_completed_cohort(
    root: &Path,
    paths: [&Path; 4],
    memory_profile: CompatibilityProfileId,
    semantic_witness: &Value,
) -> Value {
    fs::create_dir_all(root).expect("create G4 maintenance output root");
    let output = root.join("completed-maintenance");
    fs::create_dir_all(&output).expect("create G4 maintenance output directory");
    let before: Vec<_> = paths.iter().map(|path| digest(path)).collect();
    let mut owners = Vec::new();
    let mut operations = Vec::new();
    let mut retained_authority = Vec::new();
    let observe = |source: &Path, destination: &Path| {
        crate::g4_maintenance_observations::compare_preserved_authority(
            source,
            destination,
            semantic_witness,
            memory_profile,
        )
    };
    let original = RelSession::open_path_current(paths[0]).unwrap();
    assert!(
        original
            .memory_history(reliquary_owner_session::model::MemoryId([0xfe; 32]))
            .unwrap()
            .is_none()
    );
    drop(original);
    let corrupt_copy = output.join("corrupt-revision-copy.rel");
    VerifiedOwnerCopyPlan::prepare(paths[0])
        .unwrap()
        .copy_to(paths[0], &corrupt_copy)
        .unwrap();
    let mut corrupt_writer = Container::open_writer(&corrupt_copy).unwrap();
    corrupt_writer.append(b"RELMDLT1\0").unwrap();
    corrupt_writer.sync().unwrap();
    drop(corrupt_writer);
    let corrupt_hash = digest(&corrupt_copy);
    let error = RelSession::open_path_current(&corrupt_copy)
        .err()
        .expect("malformed Memory revision must fail admission");
    assert_eq!(
        error.failure_kind(),
        reliquary_owner_session::OwnerFailureKind::Corruption
    );
    assert_eq!(digest(&corrupt_copy), corrupt_hash);
    operations.push(
        json!({"operation":"corrupt_revision_rejection", "failure_kind":"Corruption",
        "rejected_bytes_unchanged":true, "missing_memory_is_distinct":true}),
    );

    for (index, source) in paths.iter().enumerate() {
        let kind = if index < 2 { "rel" } else { "phy" };
        let owner = Container::open_read_only(source).unwrap().owner_id();
        let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
        // GM explicitly excludes operational Dream/Insomnia/import/external
        // carriers from fresh-history reconstruction. Exercise both policies
        // on the completed owner and require their exact fail-closed contract.
        for identity in [
            RewriteIdentityMode::NewOwner,
            RewriteIdentityMode::PreserveOwner,
        ] {
            let policy = RewritePolicy {
                identity,
                profile_ids: [memory_profile.0].into_iter().collect(),
                archive: if index < 2 {
                    ArchiveRewriteMode::RetainAll
                } else {
                    ArchiveRewriteMode::Omit
                },
                freshness: if index < 2 {
                    FreshnessRewriteMode::PreserveEffectiveCurrent
                } else {
                    FreshnessRewriteMode::Reset
                },
                derived_state: DerivedStateRewriteMode::RetainVerified,
                ..RewritePolicy::default()
            };
            let plan = if index < 2 {
                plan_rel_rewrite(
                    source,
                    &RelSession::open_path_current(source).unwrap(),
                    policy,
                )
                .unwrap()
            } else {
                plan_phy_rewrite(
                    source,
                    &PhySession::open_path_current(source).unwrap(),
                    policy,
                )
                .unwrap()
            };
            let destination = output.join(format!("{kind}-{index}-{identity:?}"));
            let result = execute_memory_rewrite(
                &plan,
                source,
                &destination,
                (identity == RewriteIdentityMode::NewOwner).then_some([0xa0 + index as u8; 16]),
                &graph,
            );
            if index < 2 {
                let error =
                    result.expect_err("operational REL rewrite is an explicit GM typed stop");
                assert!(
                    error.to_string().contains("operational evidence"),
                    "{error}"
                );
                assert!(!destination.exists());
            } else {
                let report = result.expect("supported populated PHY fresh-history rewrite");
                let original = PhySession::open_path_current(source).unwrap();
                let reopened = PhySession::open_path_current(&destination).unwrap();
                let expected_owner = if identity == RewriteIdentityMode::NewOwner {
                    [0xa0 + index as u8; 16]
                } else {
                    owner
                };
                assert_eq!(reopened.owner_id(), expected_owner);
                let ids = original.memory_ids_page(None, 128).unwrap().1;
                assert_eq!(reopened.memory_ids_page(None, 128).unwrap().1, ids);
                assert_eq!(report.retained_memories, ids.len());
                for id in ids {
                    let old = original.current_memory(id).unwrap();
                    let new = reopened.current_memory(id).unwrap();
                    assert_eq!(old.payload, new.payload);
                    assert_eq!(old.record.source_ref, new.record.source_ref);
                    assert_eq!(old.record.source_node_id, new.record.source_node_id);
                }
                let original_proof = original
                    .historical_proof(owner, original.current_historical_cut())
                    .unwrap();
                let proof = reopened
                    .historical_proof(expected_owner, reopened.current_historical_cut())
                    .unwrap();
                assert_ne!(original_proof.history_generation, proof.history_generation);
                assert!(
                    execute_memory_rewrite(
                        &plan,
                        source,
                        &destination,
                        (identity == RewriteIdentityMode::NewOwner)
                            .then_some([0xa0 + index as u8; 16]),
                        &graph
                    )
                    .is_err()
                );
            }
            let stale = output.join(format!("{kind}-{index}-{identity:?}-stale"));
            VerifiedOwnerCopyPlan::prepare(source)
                .unwrap()
                .copy_to(source, &stale)
                .unwrap();
            append_orphan_file(&stale);
            assert!(verify_rewrite_source(&plan, &stale).is_err());
        }
        let copy = output.join(format!("{kind}-{index}-verified-copy"));
        let copy_plan = VerifiedOwnerCopyPlan::prepare(source).unwrap();
        copy_plan.copy_to(source, &copy).unwrap();
        assert_eq!(digest(source), digest(&copy));
        assert!(copy_plan.copy_to(source, &copy).is_err());
        if index < 2 {
            let original = RelSession::open_path_current(source).unwrap();
            let reopened = RelSession::open_path_current(&copy).unwrap();
            let cut = original.current_historical_cut();
            assert_eq!(
                original.historical_proof(owner, &cut).unwrap(),
                reopened.historical_proof(owner, &cut).unwrap()
            );
        } else {
            let original = PhySession::open_path_current(source).unwrap();
            let reopened = PhySession::open_path_current(&copy).unwrap();
            let cut = original.current_historical_cut();
            assert_eq!(
                original.historical_proof(owner, cut).unwrap(),
                reopened.historical_proof(owner, cut).unwrap()
            );
        }
        owners.push(json!({"alias": index, "kind": kind, "owner_id": hex(&owner),
            "fresh_history": if index < 2 { "UnsupportedOperationalState" } else { "VerifiedNewAndPreserveOwner" }, "policies_checked": ["NewOwner", "PreserveOwner"],
            "verified_copy_history_equal": true}));
        let source_session = (copy, owner, kind.to_owned());

        if kind == "rel" && index == 0 {
            // Add a valid, unreachable backing object to a copy, then prove the
            // owner seam reclaims it while preserving all live authority.
            append_orphan_file(&source_session.0);
            {
                use std::io::Read;
                let reader = Container::open_read_only(&source_session.0).unwrap();
                let mut cursor = reader.physical_cursor().unwrap();
                let mut families = std::collections::BTreeSet::new();
                while let Some(mut frame) = cursor.next_frame().unwrap() {
                    let mut magic = [0; 8];
                    frame.read_exact(&mut magic).unwrap();
                    families.insert(String::from_utf8_lossy(&magic).into_owned());
                }
                eprintln!("Completed cohort frame families: {families:?}");
                operations.push(json!({"operation":"source_frame_inventory", "families":families}));
            }
            let graph = GraphAdmission::for_replay(source_session.1, [], [], [], [], []);
            let reclamation = output.join("rel-reclaimed.rel");
            let plan = VerifiedOwnerCopyPlan::prepare(&source_session.0).expect("prepare reclaim");
            let report = reclaim_storage_copy(&plan, &source_session.0, &reclamation, &graph)
                .expect("reclaim supported orphan backing");
            assert!(report.removed_frames > 0);
            retained_authority.push(observe(&source_session.0, &reclamation));
            let noop = output.join("rel-reclaimed-noop.rel");
            let plan = VerifiedOwnerCopyPlan::prepare(&reclamation).expect("prepare no-op reclaim");
            let second =
                reclaim_storage_copy(&plan, &reclamation, &noop, &graph).expect("second reclaim");
            assert_eq!(second.removed_frames, 0);
            assert_eq!(fs::read(&reclamation).unwrap(), fs::read(&noop).unwrap());
            operations.push(
                json!({"operation":"reclaim", "removed_frames":report.removed_frames,
                "second_removed_frames":second.removed_frames, "second_byte_identical":true}),
            );
        }
        if kind == "rel" && index == 0 {
            let profile_repair_path = output.join("rel-profile-repaired.rel");
            let owner = Container::open_read_only(paths[0])
                .expect("open REL profile source")
                .owner_id();
            let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
            let repair = plan_profile_repair(paths[0], &admission, &[memory_profile])
                .expect("plan retained-profile repair");
            let profile_report =
                execute_profile_repair(&repair, paths[0], &profile_repair_path, &admission)
                    .expect("repair REL compatibility profiles");
            assert_eq!(profile_report.owner_id, owner);
            assert_eq!(
                profile_report.retired.len(),
                1,
                "the unused three-dimensional profile is retired"
            );
            assert!(!profile_report.retired.contains(&memory_profile));
            let repaired = RelSession::open_path(&profile_repair_path, &admission, [], [])
                .expect("reopen profile-repaired REL");
            assert_eq!(repaired.owner_id(), owner);
            assert_eq!(
                plan_profile_repair(&profile_repair_path, &admission, &[memory_profile])
                    .unwrap()
                    .retiring()
                    .len(),
                0
            );
            operations.push(json!({"operation":"profile_repair", "retired_profiles":profile_report.retired.iter()
                .map(|id| hex(&id.0)).collect::<Vec<_>>(), "retained_profile":hex(&memory_profile.0),
                "evidence_digest":hex(&profile_report.evidence_digest)}));
            drop(repaired);

            let prune_path = output.join("rel-l08-terminal-pruned.rel");
            let owner = Container::open_read_only(paths[0])
                .expect("open REL terminal source")
                .owner_id();
            let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
            let terminal_source = output.join("rel-l08-terminal-source.rel");
            let plan =
                VerifiedOwnerCopyPlan::prepare(paths[0]).expect("prepare terminal-copy source");
            plan.copy_to(paths[0], &terminal_source)
                .expect("copy completed REL before terminal maintenance");
            let mut terminal_writer = RelWriter::open_path_current(&terminal_source)
                .expect("open terminal-copy REL writer");
            terminal_writer
                .tombstone_conversation("conv-l08")
                .expect("tombstone authored terminal conversation on copy");
            drop(terminal_writer);
            // L08 still grounds retained semantic history and vectors. Its
            // refusal is required even after the conversation is terminal.
            let protected_plan = VerifiedOwnerCopyPlan::prepare(&terminal_source).unwrap();
            let protected_output = output.join("l08-provenance-must-survive.rel");
            assert!(
                prune_terminal_archive_copy(
                    &protected_plan,
                    &terminal_source,
                    &protected_output,
                    &["conv-l08".into()],
                    &admission
                )
                .is_err()
            );
            assert!(!protected_output.exists());
            append_maintenance_turn(&terminal_source, "conv-g4-prune", "terminal-turn");
            RelWriter::open_path_current(&terminal_source)
                .unwrap()
                .tombstone_conversation("conv-g4-prune")
                .unwrap();
            let plan =
                VerifiedOwnerCopyPlan::prepare(&terminal_source).expect("prepare Archive prune");
            let pruned = prune_terminal_archive_copy(
                &plan,
                &terminal_source,
                &prune_path,
                &["conv-g4-prune".to_owned()],
                &admission,
            )
            .expect("prune unreferenced terminal conversation on completed copy");
            assert_eq!(pruned.pruned_conversations, 1);
            assert!(pruned.pruned_turns > 0);
            let reopened = RelSession::open_path(&prune_path, &admission, [], [])
                .expect("independently reopen Archive-pruned REL");
            assert_eq!(reopened.owner_id(), owner);
            assert!(reopened.conversation_tombstone("conv-g4-prune").is_some());
            assert!(
                reopened
                    .archive_node("conv-g4-prune", "terminal-turn")
                    .is_some()
            );
            assert!(
                reopened
                    .resolved_archive_turn("conv-g4-prune", "terminal-turn")
                    .is_err()
            );
            operations.push(json!({"operation":"archive_prune", "conversation":"conv-g4-prune",
                "pruned_conversations":pruned.pruned_conversations, "pruned_turns":pruned.pruned_turns,
                "retained_node_and_tombstone":true, "payload_unavailable":true}));
            drop(reopened);

            // Reclaim the pruned copy so both retained Archive marker families
            // pass through the physical scanner with a real removable object.
            append_orphan_file(&prune_path);
            let reclamation = output.join("rel-terminal-pruned-reclaimed.rel");
            let plan = VerifiedOwnerCopyPlan::prepare(&prune_path)
                .expect("prepare terminal-marker reclamation");
            let report = reclaim_storage_copy(&plan, &prune_path, &reclamation, &admission)
                .expect("retain valid terminal Archive markers during reclamation");
            assert!(report.removed_frames > 0);
            let no_op = output.join("rel-terminal-pruned-reclaimed-noop.rel");
            let plan = VerifiedOwnerCopyPlan::prepare(&reclamation)
                .expect("prepare marker-preserving no-op reclaim");
            let second = reclaim_storage_copy(&plan, &reclamation, &no_op, &admission)
                .expect("second marker-preserving reclaim");
            assert_eq!(second.removed_frames, 0);
            assert_eq!(fs::read(&reclamation).unwrap(), fs::read(&no_op).unwrap());
            retained_authority.push(observe(&prune_path, &reclamation));

            let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
            let baseline = RelSession::open_path(paths[0], &admission, [], [])
                .expect("open original REL proof source");
            let baseline_cut = baseline.current_historical_cut();
            let baseline_proof = baseline
                .historical_proof(owner, &baseline_cut)
                .expect("capture baseline history proof");
            drop(baseline);
            let prefix_left = output.join("rel-prefix-left.rel");
            let prefix_right = output.join("rel-prefix-right.rel");
            let prefix_source =
                VerifiedOwnerCopyPlan::prepare(paths[0]).expect("prepare prefix source copies");
            prefix_source
                .copy_to(paths[0], &prefix_left)
                .expect("copy left prefix owner");
            prefix_source
                .copy_to(paths[0], &prefix_right)
                .expect("copy right prefix owner");
            append_maintenance_turn(&prefix_right, "g4-prefix", "prefix-turn");
            let prefix_output = output.join("rel-prefix-reconciled.rel");
            let prefix = reconcile_rel_prefix(
                &prefix_left,
                &prefix_right,
                &prefix_output,
                &admission,
                [],
                [],
            )
            .expect("reconcile same-owner prefix histories");
            assert!(
                prefix
                    .session
                    .resolved_archive_turn("g4-prefix", "prefix-turn")
                    .unwrap()
                    .is_some()
            );
            assert_eq!(
                prefix
                    .session
                    .historical_proof(owner, &baseline_cut)
                    .unwrap(),
                baseline_proof
            );
            drop(prefix.session);

            let divergent_left = output.join("rel-divergent-left.rel");
            let divergent_right = output.join("rel-divergent-right.rel");
            prefix_source
                .copy_to(paths[0], &divergent_left)
                .expect("copy divergent left owner");
            prefix_source
                .copy_to(paths[0], &divergent_right)
                .expect("copy divergent right owner");
            append_maintenance_turn(&divergent_left, "g4-divergent-left", "left-turn");
            append_maintenance_turn(&divergent_right, "g4-divergent-right", "right-turn");
            let divergent_output = output.join("rel-divergent-reconciled.rel");
            let divergent = reconcile_rel_owner(
                &divergent_left,
                &divergent_right,
                &divergent_output,
                &admission,
                [],
                [],
            )
            .expect("replay supported divergent same-owner histories");
            assert!(
                divergent
                    .session
                    .resolved_archive_turn("g4-divergent-left", "left-turn")
                    .unwrap()
                    .is_some()
            );
            assert!(
                divergent
                    .session
                    .resolved_archive_turn("g4-divergent-right", "right-turn")
                    .unwrap()
                    .is_some()
            );
            assert_eq!(
                divergent
                    .session
                    .historical_proof(owner, &baseline_cut)
                    .unwrap(),
                baseline_proof
            );
            drop(divergent.session);
            retained_authority.push(observe(paths[0], &prefix_output));
            retained_authority.push(observe(paths[0], &divergent_output));
            let retry_output = output.join("rel-divergent-retry.rel");
            drop(
                reconcile_rel_owner(
                    &divergent_output,
                    &divergent_right,
                    &retry_output,
                    &admission,
                    [],
                    [],
                )
                .unwrap()
                .session,
            );
            assert_eq!(
                fs::read(&divergent_output).unwrap(),
                fs::read(&retry_output).unwrap()
            );

            let wrong_owner_output = output.join("rel-wrong-owner-rejected.rel");
            let error =
                reconcile_rel_prefix(paths[0], paths[1], &wrong_owner_output, &admission, [], [])
                    .err()
                    .expect("reject reconciliation across owner identities");
            assert!(!error.is_empty());
            assert!(!wrong_owner_output.exists());
            let conflicting_left = output.join("conflicting-left.rel");
            let conflicting_right = output.join("conflicting-right.rel");
            let conflict_plan = VerifiedOwnerCopyPlan::prepare(paths[0]).unwrap();
            conflict_plan.copy_to(paths[0], &conflicting_left).unwrap();
            conflict_plan.copy_to(paths[0], &conflicting_right).unwrap();
            append_maintenance_turn_content(
                &conflicting_left,
                "g4-conflict",
                "same-node",
                "Left accepted statement",
            );
            append_maintenance_turn_content(
                &conflicting_right,
                "g4-conflict",
                "same-node",
                "Right conflicting statement",
            );
            let hashes = [digest(&conflicting_left), digest(&conflicting_right)];
            let rejected = output.join("conflicting-reconcile-must-not-exist.rel");
            assert!(
                reconcile_rel_owner(
                    &conflicting_left,
                    &conflicting_right,
                    &rejected,
                    &admission,
                    [],
                    []
                )
                .is_err()
            );
            assert!(!rejected.exists());
            assert_eq!(
                [digest(&conflicting_left), digest(&conflicting_right)],
                hashes
            );
            operations.push(
                json!({"operation":"conflicting_reconciliation", "published":false,
                "same_node_different_bodies_rejected":true,"sources_unchanged":true}),
            );
        }
    }

    // Cascade carries the completed native import epoch, so its independent
    // divergence/retry proves import authority as well as Lantern's semantics.
    let cascade = RelSession::open_path_current(paths[1]).unwrap();
    let cascade_owner = cascade.owner_id();
    let cascade_cut = cascade.current_historical_cut();
    let cascade_proof = cascade
        .historical_proof(cascade_owner, &cascade_cut)
        .unwrap();
    drop(cascade);
    let cascade_admission = GraphAdmission::for_replay(cascade_owner, [], [], [], [], []);
    let cascade_plan = VerifiedOwnerCopyPlan::prepare(paths[1]).unwrap();
    let cascade_left = output.join("cascade-divergent-left.rel");
    let cascade_right = output.join("cascade-divergent-right.rel");
    cascade_plan.copy_to(paths[1], &cascade_left).unwrap();
    cascade_plan.copy_to(paths[1], &cascade_right).unwrap();
    append_maintenance_turn(&cascade_left, "g4-cascade-left", "cascade-left");
    append_maintenance_turn(&cascade_right, "g4-cascade-right", "cascade-right");
    let cascade_output = output.join("cascade-divergent-output.rel");
    let merged = reconcile_rel_owner(
        &cascade_left,
        &cascade_right,
        &cascade_output,
        &cascade_admission,
        [],
        [],
    )
    .unwrap();
    assert_eq!(
        merged
            .session
            .historical_proof(cascade_owner, &cascade_cut)
            .unwrap(),
        cascade_proof
    );
    for (conversation, node) in [
        ("g4-cascade-left", "cascade-left"),
        ("g4-cascade-right", "cascade-right"),
        ("conv-c03-additive-import", "C03-t1"),
    ] {
        assert!(
            merged
                .session
                .resolved_archive_turn(conversation, node)
                .unwrap()
                .is_some()
        );
    }
    drop(merged.session);
    retained_authority.push(observe(paths[1], &cascade_output));
    let cascade_retry = output.join("cascade-divergent-retry.rel");
    drop(
        reconcile_rel_owner(
            &cascade_output,
            &cascade_right,
            &cascade_retry,
            &cascade_admission,
            [],
            [],
        )
        .unwrap()
        .session,
    );
    assert_eq!(
        fs::read(&cascade_output).unwrap(),
        fs::read(&cascade_retry).unwrap()
    );

    for (path, expected) in paths.iter().zip(before) {
        assert_eq!(digest(path), expected, "maintenance changed cohort source");
    }
    json!({"owners": owners, "source_hashes_unchanged": true, "rewrite_policies": ["NewOwner", "PreserveOwner"], "operations":operations,
        "retained_authority":retained_authority, "lantern_reconcile_history":true, "cascade_import_reconcile_history":true,
        "both_reconcile_retries_byte_identical":true})
}

#[cfg(test)]
fn append_orphan_file(path: &Path) {
    // Exceed the fixed reclamation-proof overhead so this is a real saving,
    // rather than the intentionally exact no-op for tiny orphan objects.
    let bytes = b"G4 unreachable Archive backing object".repeat(32);
    let id: [u8; 32] = Sha256::digest(&bytes).into();
    let mut frame = b"RELFILE1".to_vec();
    frame.extend_from_slice(&id);
    frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    frame.extend_from_slice(&bytes);
    let mut writer = Container::open_writer(path).expect("open maintenance copy writer");
    writer.append(&frame).expect("append orphan backing");
    writer.sync().expect("sync orphan backing");
}

#[cfg(test)]
fn append_maintenance_turn(path: &Path, conversation: &str, turn_id: &str) {
    append_maintenance_turn_content(
        path,
        conversation,
        turn_id,
        &format!("Accepted maintenance reconciliation witness: {turn_id}"),
    );
}

fn append_maintenance_turn_content(path: &Path, conversation: &str, turn_id: &str, content: &str) {
    let mut writer =
        RelWriter::open_path_current(path).expect("open same-owner maintenance writer");
    writer
        .accept_turn_and_advance_branch(
            TurnInput {
                id: turn_id.into(),
                conversation_id: conversation.into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("operator".into()),
                timestamp_ns: 9_000_000,
                content: content.into(),
                attachments: vec![],
                project_attachments: vec![],
            },
            Branch {
                id: "main".into(),
                conversation_id: conversation.into(),
                leaf_node_id: turn_id.into(),
                canonical: true,
            },
        )
        .expect("append accepted owner turn");
}

#[cfg(test)]
fn digest(path: &Path) -> [u8; 32] {
    Sha256::digest(fs::read(path).expect("read owner bytes")).into()
}

#[cfg(test)]
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
