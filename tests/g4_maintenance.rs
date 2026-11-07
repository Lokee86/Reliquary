use reliquary_memory::{
    DerivedStateRewriteMode, FreshnessRewriteMode, Gateway, HostCredentialBinding, OwnerGrant,
    OwnerScope, Phylactery, Rel, RewriteIdentityMode, RewritePolicy, TrustedHostAdmission,
    VerifiedCopyPlan, execute_rewrite, prune_archive_copy, reclaim_storage_copy, reconcile_rel,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct TestPaths(PathBuf);

impl TestPaths {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "g4-maintenance-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TestPaths {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn add_memory(path: &Path, title: &str, body: &str, timestamp: i64) {
    let rel = Rel::open(path).unwrap();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true).unwrap();
    let token = format!("g4-maintenance-{title}-token-000000000000000000000000000000");
    let (adapter, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "maintenance-fixture",
        &token,
        [OwnerGrant::ManageRel(owner)],
    )])
    .install(&gateway)
    .unwrap();
    let management = adapter
        .authenticate(&token)
        .unwrap()
        .workspace_management(owner)
        .unwrap();
    management
        .create_memory(
            title.to_owned(),
            body.to_owned(),
            "fact".into(),
            "project".into(),
            timestamp,
        )
        .unwrap();
    management.quiesce().unwrap();
    drop(management);
    drop(adapter);
    drop(gateway);
    drop(rel);
}

#[test]
fn public_maintenance_rewrites_history_and_reconciles_quiescent_rel_copies() {
    let paths = TestPaths::new();
    let source_path = paths.path("cohort.rel");
    let new_owner_copy = paths.path("new-owner.rel");
    let preserved_copy = paths.path("preserved-owner.rel");
    let prefix_base = paths.path("prefix-base.rel");
    let prefix_extended = paths.path("prefix-extended.rel");
    let prefix_merged = paths.path("prefix-merged.rel");
    let divergent_left = paths.path("divergent-left.rel");
    let divergent_right = paths.path("divergent-right.rel");
    let divergent_merged = paths.path("divergent-merged.rel");
    let collision = paths.path("collision.rel");

    let rel = Rel::create(&source_path, OwnerScope::Project).unwrap();
    let source_id = rel.owner_id();
    drop(rel);
    add_memory(&source_path, "cohort-fact", "A populated REL memory", 100);

    let before = fs::read(&source_path).unwrap();
    let source = Rel::open(&source_path).unwrap();
    let before_info = source.info().unwrap();
    let before_proof = source.current_historical_proof().unwrap();
    assert_eq!(before_info.memory_count, 1);
    assert_eq!(before_proof.owner_id, source_id);
    assert!(before_proof.retained_records > 0);

    let new_owner_plan = source.plan_rewrite(RewritePolicy::default()).unwrap();
    let new_owner_report =
        execute_rewrite(&new_owner_plan, source.path(), &new_owner_copy).unwrap();
    let new_owner = Rel::open(&new_owner_copy).unwrap();
    assert_ne!(new_owner.owner_id(), source_id);
    assert_eq!(
        new_owner.owner_id().as_bytes(),
        new_owner_report.destination_owner_id
    );
    assert_eq!(new_owner.info().unwrap().memory_count, 1);
    assert_eq!(
        new_owner.current_historical_proof().unwrap().owner_id,
        new_owner.owner_id()
    );
    assert_eq!(fs::read(&source_path).unwrap(), before);
    assert_eq!(
        execute_rewrite(&new_owner_plan, source.path(), &new_owner_copy)
            .unwrap_err()
            .kind(),
        reliquary_memory::ProductErrorKind::PublicationCollision
    );

    let mut preserve_policy = RewritePolicy::default();
    preserve_policy.identity = RewriteIdentityMode::PreserveOwner;
    preserve_policy.derived_state = DerivedStateRewriteMode::Omit;
    let preserve_plan = source.plan_rewrite(preserve_policy).unwrap();
    let preserve_report = execute_rewrite(&preserve_plan, source.path(), &preserved_copy).unwrap();
    let preserved = Rel::open(&preserved_copy).unwrap();
    assert_eq!(preserved.owner_id(), source_id);
    assert_eq!(
        preserved.owner_id().as_bytes(),
        preserve_report.destination_owner_id
    );
    assert_eq!(preserved.info().unwrap().memory_count, 1);
    let after_proof = preserved.current_historical_proof().unwrap();
    assert_eq!(after_proof.owner_id, source_id);
    assert_ne!(
        after_proof.history_generation,
        before_proof.history_generation
    );
    assert_eq!(fs::read(&source_path).unwrap(), before);
    drop(preserved);
    drop(new_owner);
    drop(source);

    fs::copy(paths.path("cohort.rel"), &prefix_base).unwrap();
    fs::copy(paths.path("cohort.rel"), &prefix_extended).unwrap();
    add_memory(
        &prefix_extended,
        "prefix-addition",
        "An appended public memory",
        200,
    );
    let base_bytes = fs::read(&prefix_base).unwrap();
    let extended_bytes = fs::read(&prefix_extended).unwrap();
    let prefix_report = reconcile_rel(&prefix_base, &prefix_extended, &prefix_merged).unwrap();
    assert_eq!(
        prefix_report.relation,
        reliquary_memory::ReconciliationRelation::RightExtendsLeft
    );
    assert_eq!(
        Rel::open(&prefix_merged)
            .unwrap()
            .info()
            .unwrap()
            .memory_count,
        2
    );
    assert_eq!(fs::read(&prefix_base).unwrap(), base_bytes);
    assert_eq!(fs::read(&prefix_extended).unwrap(), extended_bytes);
    fs::write(&collision, b"existing destination").unwrap();
    let collision_bytes = fs::read(&collision).unwrap();
    assert_eq!(
        reconcile_rel(&prefix_base, &prefix_extended, &collision)
            .unwrap_err()
            .kind(),
        reliquary_memory::ProductErrorKind::PublicationCollision
    );
    assert_eq!(fs::read(&collision).unwrap(), collision_bytes);

    fs::copy(paths.path("cohort.rel"), &divergent_left).unwrap();
    fs::copy(paths.path("cohort.rel"), &divergent_right).unwrap();
    add_memory(&divergent_left, "left-addition", "Left branch memory", 300);
    add_memory(
        &divergent_right,
        "right-addition",
        "Right branch memory",
        400,
    );
    let left_bytes = fs::read(&divergent_left).unwrap();
    let right_bytes = fs::read(&divergent_right).unwrap();
    let divergent_report =
        reconcile_rel(&divergent_left, &divergent_right, &divergent_merged).unwrap();
    assert_eq!(
        divergent_report.relation,
        reliquary_memory::ReconciliationRelation::Diverged
    );
    let merged = Rel::open(&divergent_merged).unwrap();
    assert_eq!(merged.info().unwrap().memory_count, 3);
    assert!(!merged.current_historical_proof().unwrap().digest.is_empty());
    assert_eq!(fs::read(&divergent_left).unwrap(), left_bytes);
    assert_eq!(fs::read(&divergent_right).unwrap(), right_bytes);
    let retry = paths.path("divergent-retry.rel");
    fs::copy(&divergent_merged, &retry).unwrap();
    let retry_report =
        reconcile_rel(&divergent_merged, &retry, &paths.path("retry-copy.rel")).unwrap();
    assert_eq!(
        retry_report.relation,
        reliquary_memory::ReconciliationRelation::Identical
    );
    assert_eq!(
        fs::read(paths.path("retry-copy.rel")).unwrap(),
        fs::read(&divergent_merged).unwrap()
    );
}

// The public product API has no Phylactery mutation operation yet. This test
// checks owner-kind, rewrite identity, and history-generation behavior on PHY.
#[test]
fn public_maintenance_rewrites_empty_phy_and_preserves_source() {
    let paths = TestPaths::new();
    let source = paths.path("private.phy");
    let rewritten = paths.path("private-copy.phy");
    let preserved = paths.path("private-preserved.phy");
    let phy = Phylactery::create(&source).unwrap();
    let source_id = phy.owner_id();
    let before_proof = phy.current_historical_proof().unwrap();
    let before = fs::read(&source).unwrap();

    let mut new_owner_policy = RewritePolicy::default();
    new_owner_policy.archive = reliquary_memory::ArchiveRewriteMode::Omit;
    new_owner_policy.freshness = FreshnessRewriteMode::Reset;
    let new_owner_plan = phy.plan_rewrite(new_owner_policy).unwrap();
    let report = execute_rewrite(&new_owner_plan, &source, &rewritten).unwrap();
    let fresh = Phylactery::open(&rewritten).unwrap();
    assert_ne!(fresh.owner_id(), source_id);
    assert_eq!(fresh.owner_id().as_bytes(), report.destination_owner_id);
    assert_eq!(
        fresh.info().unwrap().kind,
        reliquary_memory::OwnerKind::Phylactery
    );
    assert_eq!(fs::read(&source).unwrap(), before);

    let mut policy = RewritePolicy::default();
    policy.identity = RewriteIdentityMode::PreserveOwner;
    policy.archive = reliquary_memory::ArchiveRewriteMode::Omit;
    policy.freshness = FreshnessRewriteMode::Reset;
    let plan = phy.plan_rewrite(policy).unwrap();
    let report = execute_rewrite(&plan, &source, &preserved).unwrap();
    let preserved = Phylactery::open(&preserved).unwrap();
    assert_eq!(preserved.owner_id(), source_id);
    assert_eq!(preserved.owner_id().as_bytes(), report.destination_owner_id);
    assert_ne!(
        preserved
            .current_historical_proof()
            .unwrap()
            .history_generation,
        before_proof.history_generation
    );
    assert_eq!(fs::read(&source).unwrap(), before);
}

#[test]
fn historical_cuts_distinguish_foreign_future_tampered_and_unavailable_state() {
    use reliquary_owner_session::{HistoricalProofError, PhySession, RelSession};

    let paths = TestPaths::new();
    let rel_path = paths.path("history-rel.rel");
    let rel = Rel::create(&rel_path, OwnerScope::Project).unwrap();
    let rel_owner_id = rel.owner_id();
    drop(rel);
    add_memory(
        &rel_path,
        "history-memory",
        "A retained historical fact",
        10,
    );
    let before = fs::read(&rel_path).unwrap();

    let rel = RelSession::open_path_current(&rel_path).unwrap();
    let cut = rel.current_historical_cut();
    let proof = rel.historical_proof(rel.owner_id(), &cut).unwrap();
    assert_eq!(proof.owner_id, rel.owner_id());
    assert_eq!(
        rel.historical_proof([0xEE; 16], &cut).unwrap_err(),
        HistoricalProofError::WrongOwner
    );
    let mut future = cut.clone();
    future.owner.memory_version += 1;
    assert!(matches!(
        rel.historical_proof(rel.owner_id(), &future),
        Err(HistoricalProofError::FutureCut("Memory"))
    ));
    let mut tampered = cut.clone();
    tampered.archive_version = u64::MAX;
    assert!(matches!(
        rel.historical_proof(rel.owner_id(), &tampered),
        Err(HistoricalProofError::FutureCut("Archive"))
    ));
    drop(rel);

    let public_rel = Rel::open(&rel_path).unwrap();
    assert!(matches!(
        public_rel
            .memory_history_at_cut(
                reliquary_memory::MemoryId([0xFE; 32]),
                cut.owner.memory_version,
            )
            .unwrap(),
        reliquary_memory::MemoryHistoryLookup::Unavailable { .. }
    ));
    assert_eq!(public_rel.owner_id(), rel_owner_id);
    assert_eq!(fs::read(&rel_path).unwrap(), before);

    let phy_path = paths.path("history.phy");
    let phy = Phylactery::create(&phy_path).unwrap();
    let phy_id = phy.owner_id();
    drop(phy);
    let phy = PhySession::open_path_current(&phy_path).unwrap();
    let phy_cut = phy.current_historical_cut();
    assert_eq!(
        phy.historical_proof(phy_id.as_bytes(), phy_cut)
            .unwrap()
            .owner_id,
        phy_id.as_bytes()
    );
    let mut future_phy = phy.current_historical_cut();
    future_phy.memory_version += 1;
    assert!(matches!(
        phy.historical_proof(phy_id.as_bytes(), future_phy),
        Err(HistoricalProofError::FutureCut("Memory"))
    ));
}

#[test]
fn active_archive_prune_request_fails_closed_and_reclaim_is_exact_noop_on_populated_rel() {
    use reliquary_owner_session::{
        GraphAdmission, RelSession, RelWriter,
        model::{Branch, TurnInput},
    };

    let paths = TestPaths::new();
    let source = paths.path("populated-source.rel");
    let rejected = paths.path("active-prune.rel");
    let pruned = paths.path("terminal-pruned.rel");
    let reclaim_source = paths.path("reclaim-source.rel");
    let reclaimed = paths.path("reclaimed-copy.rel");
    let second = paths.path("reclaimed-copy-again.rel");

    let rel = Rel::create(&source, OwnerScope::Project).unwrap();
    let owner = rel.owner_id().as_bytes();
    drop(rel);
    add_memory(&source, "active-fact", "Active state stays available", 1);
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::open_path(&source, admission.clone(), [], []).unwrap();
    writer
        .accept_turn_and_advance_branch(
            TurnInput {
                id: "L08-terminal-turn".into(),
                conversation_id: "conv-l08".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("principal-a".into()),
                timestamp_ns: 2,
                content: "Temporary G4 rehearsal text must be pruned after tombstone.".repeat(200),
                attachments: vec![],
                project_attachments: vec![],
            },
            Branch {
                id: "main".into(),
                conversation_id: "conv-l08".into(),
                leaf_node_id: "L08-terminal-turn".into(),
                canonical: true,
            },
        )
        .unwrap();
    drop(writer);

    let plan = VerifiedCopyPlan::prepare(&source).unwrap();
    assert!(prune_archive_copy(&plan, &source, &rejected, &["conv-l08".into()]).is_err());
    assert!(!rejected.exists());

    let mut writer = RelWriter::open_path(&source, admission.clone(), [], []).unwrap();
    assert!(writer.tombstone_conversation("conv-l08").unwrap());
    let source_cut = writer.read_session().current_historical_cut();
    let source_proof = writer
        .read_session()
        .historical_proof(owner, &source_cut)
        .unwrap();
    drop(writer);
    let source_bytes = fs::read(&source).unwrap();
    let plan = VerifiedCopyPlan::prepare(&source).unwrap();
    let prune_report = prune_archive_copy(&plan, &source, &pruned, &["conv-l08".into()]).unwrap();
    assert_eq!(prune_report.pruned_conversations, 1);
    assert_eq!(prune_report.pruned_turns, 1);
    assert!(prune_report.transformed_bytes < prune_report.source_bytes);
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    let pruned_reader = RelSession::open_path(&pruned, &admission, [], []).unwrap();
    assert!(
        pruned_reader
            .archive_node("conv-l08", "L08-terminal-turn")
            .is_some()
    );
    assert!(
        pruned_reader
            .resolved_archive_turn("conv-l08", "L08-terminal-turn")
            .is_err()
    );
    assert!(pruned_reader.conversation_tombstone("conv-l08").is_some());
    let pruned_proof = pruned_reader.historical_proof(owner, &source_cut).unwrap();
    assert_eq!(
        pruned_proof.history_generation,
        source_proof.history_generation
    );
    assert!(pruned_proof.unavailable_payloads > source_proof.unavailable_payloads);
    drop(pruned_reader);

    drop(Rel::create(&reclaim_source, OwnerScope::Project).unwrap());
    add_memory(
        &reclaim_source,
        "reclaim-live-fact",
        "The supported reclaim copy keeps this live mapping.",
        3,
    );
    let reclaim_source_bytes = fs::read(&reclaim_source).unwrap();
    let reclaim_plan = VerifiedCopyPlan::prepare(&reclaim_source).unwrap();
    let first = reclaim_storage_copy(&reclaim_plan, &reclaim_source, &reclaimed).unwrap();
    assert_eq!(first.removed_frames, 0);
    assert_eq!(fs::read(&reclaim_source).unwrap(), reclaim_source_bytes);
    let second_plan = VerifiedCopyPlan::prepare(&reclaimed).unwrap();
    let second_report = reclaim_storage_copy(&second_plan, &reclaimed, &second).unwrap();
    assert_eq!(second_report.removed_frames, 0);
    assert_eq!(fs::read(&reclaimed).unwrap(), fs::read(&second).unwrap());
    assert_eq!(Rel::open(&second).unwrap().info().unwrap().memory_count, 1);
}
