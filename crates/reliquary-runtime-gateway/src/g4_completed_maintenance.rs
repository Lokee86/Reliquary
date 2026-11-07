//! G4 maintenance proof on an independently completed native cohort.

#[cfg(test)]
use super::*;
#[cfg(test)]
use reliquary_application::{
    ArchiveRewriteMode, DerivedStateRewriteMode, FreshnessRewriteMode, RewriteIdentityMode,
    RewritePolicy, execute_memory_rewrite, plan_phy_rewrite, plan_rel_rewrite, reconcile_rel_owner,
    reconcile_rel_prefix,
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
) -> Value {
    fs::create_dir_all(root).expect("create G4 maintenance output root");
    let output = root.join("completed-maintenance");
    fs::create_dir_all(&output).expect("create G4 maintenance output directory");
    let before: Vec<_> = paths.iter().map(|path| digest(path)).collect();
    let mut owners = Vec::new();

    for (index, source) in paths.iter().enumerate() {
        let kind = if index < 2 { "rel" } else { "phy" };
        let source_session = if kind == "rel" {
            let owner = Container::open_read_only(source)
                .expect("open REL source")
                .owner_id();
            let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
            let session = RelSession::open_path(source, &graph, [], []).expect("open REL reader");
            // The completed cohort uses one shared REL/PHY embedding profile
            // for Memory and Archive vectors, so selecting it retains both.
            let profiles = [memory_profile.0].into_iter().collect();
            let policy = RewritePolicy {
                identity: RewriteIdentityMode::NewOwner,
                profile_ids: profiles,
                archive: ArchiveRewriteMode::RetainRequiredProvenance,
                freshness: FreshnessRewriteMode::PreserveEffectiveCurrent,
                derived_state: DerivedStateRewriteMode::RetainVerified,
                ..RewritePolicy::default()
            };
            let plan = plan_rel_rewrite(source, &session, policy).expect("plan REL rewrite");
            let new_path = output.join(format!("{kind}-{index}-new.rel"));
            let new_owner = [0xD0u8.wrapping_add(index as u8); 16];
            execute_memory_rewrite(&plan, source, &new_path, Some(new_owner), &graph)
                .expect("execute NewOwner REL rewrite");
            let new_graph = GraphAdmission::for_replay(new_owner, [], [], [], [], []);
            let reopened = RelSession::open_path(&new_path, &new_graph, [], [])
                .expect("independently reopen NewOwner REL");
            assert_eq!(reopened.owner_id(), new_owner);
            drop(reopened);
            let preserve_path = output.join(format!("{kind}-{index}-preserve.rel"));
            let mut preserve_policy = plan.policy.clone();
            preserve_policy.identity = RewriteIdentityMode::PreserveOwner;
            let preserve_plan = plan_rel_rewrite(source, &session, preserve_policy)
                .expect("plan PreserveOwner REL rewrite");
            execute_memory_rewrite(&preserve_plan, source, &preserve_path, None, &graph)
                .expect("execute PreserveOwner REL rewrite");
            let reopened =
                RelSession::open_path(source, &graph, [], []).expect("reopen source REL");
            let preserved = RelSession::open_path(&preserve_path, &graph, [], [])
                .expect("independently reopen PreserveOwner REL");
            assert_eq!(reopened.owner_id(), preserved.owner_id());
            drop((reopened, preserved, session));
            owners.push(json!({"alias": index, "kind": kind, "new_owner": hex(&new_owner), "preserve_owner": "verified"}));
            (new_path, new_owner, kind.to_owned())
        } else {
            let owner = Container::open_read_only(source)
                .expect("open PHY source")
                .owner_id();
            let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
            let session = PhySession::open_path(source, &graph, []).expect("open PHY reader");
            let policy = RewritePolicy {
                identity: RewriteIdentityMode::NewOwner,
                profile_ids: [memory_profile.0].into_iter().collect(),
                archive: ArchiveRewriteMode::Omit,
                freshness: FreshnessRewriteMode::Reset,
                derived_state: DerivedStateRewriteMode::RetainVerified,
                ..RewritePolicy::default()
            };
            let plan = plan_phy_rewrite(source, &session, policy).expect("plan PHY rewrite");
            let new_path = output.join(format!("{kind}-{index}-new.phy"));
            let new_owner = [0xD0u8.wrapping_add(index as u8); 16];
            execute_memory_rewrite(&plan, source, &new_path, Some(new_owner), &graph)
                .expect("execute NewOwner PHY rewrite");
            let new_graph = GraphAdmission::for_replay(new_owner, [], [], [], [], []);
            let reopened =
                PhySession::open_path(&new_path, &new_graph, []).expect("reopen NewOwner PHY");
            assert_eq!(reopened.owner_id(), new_owner);
            drop(reopened);
            let mut preserve_policy = plan.policy.clone();
            preserve_policy.identity = RewriteIdentityMode::PreserveOwner;
            let preserve_plan = plan_phy_rewrite(source, &session, preserve_policy)
                .expect("plan PreserveOwner PHY rewrite");
            let preserve_path = output.join(format!("{kind}-{index}-preserve.phy"));
            execute_memory_rewrite(&preserve_plan, source, &preserve_path, None, &graph)
                .expect("execute PreserveOwner PHY rewrite");
            let preserved = PhySession::open_path(&preserve_path, &graph, [])
                .expect("reopen PreserveOwner PHY");
            assert_eq!(preserved.owner_id(), owner);
            drop((preserved, session));
            owners.push(json!({"alias": index, "kind": kind, "new_owner": hex(&new_owner), "preserve_owner": "verified"}));
            (new_path, new_owner, kind.to_owned())
        };

        if kind == "rel" && index == 0 {
            // Add a valid, unreachable backing object to a copy, then prove the
            // owner seam reclaims it while preserving all live authority.
            append_orphan_file(&source_session.0);
            let graph = GraphAdmission::for_replay(source_session.1, [], [], [], [], []);
            let reclamation = output.join("rel-reclaimed.rel");
            let plan = VerifiedOwnerCopyPlan::prepare(&source_session.0).expect("prepare reclaim");
            let report = reclaim_storage_copy(&plan, &source_session.0, &reclamation, &graph)
                .expect("reclaim supported orphan backing");
            assert!(report.removed_frames > 0);
            let noop = output.join("rel-reclaimed-noop.rel");
            let plan = VerifiedOwnerCopyPlan::prepare(&reclamation).expect("prepare no-op reclaim");
            let second =
                reclaim_storage_copy(&plan, &reclamation, &noop, &graph).expect("second reclaim");
            assert_eq!(second.removed_frames, 0);
            assert_eq!(fs::read(&reclamation).unwrap(), fs::read(&noop).unwrap());
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
            let repaired = RelSession::open_path(&profile_repair_path, &admission, [], [])
                .expect("reopen profile-repaired REL");
            assert_eq!(repaired.owner_id(), owner);
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
            let plan =
                VerifiedOwnerCopyPlan::prepare(&terminal_source).expect("prepare Archive prune");
            let pruned = prune_terminal_archive_copy(
                &plan,
                &terminal_source,
                &prune_path,
                &["conv-l08".to_owned()],
                &admission,
            )
            .expect("prune terminal conv-l08 on copy");
            assert_eq!(pruned.pruned_conversations, 1);
            assert!(pruned.pruned_turns > 0);
            let reopened = RelSession::open_path(&prune_path, &admission, [], [])
                .expect("independently reopen Archive-pruned REL");
            assert_eq!(reopened.owner_id(), owner);
            assert!(reopened.conversation_tombstone("conv-l08").is_some());
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

            let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
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
            drop(divergent.session);

            let wrong_owner_output = output.join("rel-wrong-owner-rejected.rel");
            let error = reconcile_rel_prefix(
                paths[0],
                &source_session.0,
                &wrong_owner_output,
                &admission,
                [],
                [],
            )
            .err()
            .expect("reject reconciliation across owner identities");
            assert!(!error.is_empty());
            assert!(!wrong_owner_output.exists());
        }
    }

    for (path, expected) in paths.iter().zip(before) {
        assert_eq!(digest(path), expected, "maintenance changed cohort source");
    }
    json!({"owners": owners, "source_hashes_unchanged": true, "rewrite_policies": ["NewOwner", "PreserveOwner"], "reclamation": "supported REL orphan removed; exact second no-op"})
}

#[cfg(test)]
fn append_orphan_file(path: &Path) {
    let bytes = b"G4 unreachable Archive backing object";
    let id: [u8; 32] = Sha256::digest(bytes).into();
    let mut frame = b"RELFILE1".to_vec();
    frame.extend_from_slice(&id);
    frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    frame.extend_from_slice(bytes);
    let mut writer = Container::open_writer(path).expect("open maintenance copy writer");
    writer.append(&frame).expect("append orphan backing");
    writer.sync().expect("sync orphan backing");
}

#[cfg(test)]
fn append_maintenance_turn(path: &Path, conversation: &str, turn_id: &str) {
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
                content: format!("Accepted maintenance reconciliation witness: {turn_id}"),
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
