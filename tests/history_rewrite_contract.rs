use reliquary_memory::rewrite::{
    ArchiveSelection, FreshnessRewriteMode, IdentityPolicy, MemoryBase, ProvenancePolicy,
    RewriteRequest, plan_rewrite_file, rewrite_file, rewrite_from_plan,
};
use reliquary_memory::{
    Cva, EgoAnchorPriority, EpisodeBoundary, EpisodeConfig, EpisodeOrigin, FragmentConfig,
    MemoryDraft, MemoryId, Phylactery, SimulatedEmbeddingEndpoint, VectorNormalization,
};
use std::{fs, path::PathBuf};

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let path = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(format!("history-rewrite-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft(mutation: &str, title: &str, content: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "user".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: title.into(),
        content: content.into(),
        scope: "user".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(10),
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn seed_rel(path: &std::path::Path) -> Vec<MemoryId> {
    let mut rel = Cva::create(path).unwrap();
    let first = rel
        .publish_memory(None, 0, draft("one", "one", "first memory"))
        .unwrap()
        .0
        .id;
    let second = rel
        .publish_memory(None, 0, draft("two", "two", "second memory"))
        .unwrap()
        .0
        .id;
    rel.sync().unwrap();
    vec![first, second]
}

fn id_hex(id: MemoryId) -> String {
    id.0.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_profile(id: [u8; 32]) -> String {
    id.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn digest(path: &std::path::Path) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(fs::read(path).unwrap())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn default_all_current_rel_and_phy_rewrites_create_independent_owners() {
    let temp = Temp::new();
    let rel_source = temp.path("source.rel");
    let rel_output = temp.path("rewritten.rel");
    let ids = seed_rel(&rel_source);
    let source_hash = digest(&rel_source);
    let source_owner = Cva::open(&rel_source).unwrap().owner_uuid().unwrap();
    let request = RewriteRequest::default();
    let report = rewrite_file(&rel_source, &rel_output, &request).unwrap();
    assert_eq!(report.plan.retained_memories, 2);
    assert_eq!(report.plan.dropped_memories, 0);
    let mut source = Cva::open(&rel_source).unwrap();
    let mut rewritten = Cva::open(&rel_output).unwrap();
    assert_ne!(source.owner_id(), rewritten.owner_id());
    assert_ne!(
        rewritten.history_generation(),
        reliquary_memory::HistoryGenerationId::ZERO
    );
    assert_eq!(
        rewritten.rewrite_genesis().unwrap().predecessor_owner,
        source_owner
    );
    for id in ids {
        let mut expected = source.memory(id).unwrap();
        let actual = rewritten.memory(id).unwrap();
        expected.global_version = actual.global_version;
        expected.memory_version = actual.memory_version;
        assert_eq!(expected, actual);
    }
    drop(source);
    drop(rewritten);
    assert_eq!(digest(&rel_source), source_hash);

    let phy_source = temp.path("source.phy");
    let phy_output = temp.path("rewritten.phy");
    let mut phy = Phylactery::create(&phy_source).unwrap();
    let id = phy
        .publish_memory(None, 0, draft("phy-one", "phy", "PHY memory"))
        .unwrap()
        .0
        .id;
    phy.sync().unwrap();
    drop(phy);
    let report = rewrite_file(&phy_source, &phy_output, &RewriteRequest::default()).unwrap();
    assert_eq!(report.plan.retained_memories, 1);
    let mut source = Phylactery::open(&phy_source).unwrap();
    let mut rewritten = Phylactery::open(&phy_output).unwrap();
    assert_ne!(source.owner_id(), rewritten.owner_id());
    let mut expected = source.memory(id).unwrap();
    let actual = rewritten.memory(id).unwrap();
    expected.global_version = actual.global_version;
    expected.memory_version = actual.memory_version;
    assert_eq!(expected, actual);
}

#[test]
fn explicit_exclusion_wins_and_archived_superseded_and_freshness_filters_are_planned() {
    let temp = Temp::new();
    let source = temp.path("selection.rel");
    let mut rel = Cva::create(&source).unwrap();
    let ordinary = rel
        .publish_memory(None, 0, draft("ordinary", "ordinary", "ordinary"))
        .unwrap()
        .0
        .id;
    let archived_draft = MemoryDraft {
        archived: true,
        ..draft("archived", "archived", "archived")
    };
    let archived = rel.publish_memory(None, 0, archived_draft).unwrap().0.id;
    let superseded_draft = MemoryDraft {
        superseded_by: Some(ordinary),
        ..draft("superseded", "superseded", "superseded")
    };
    let superseded = rel.publish_memory(None, 0, superseded_draft).unwrap().0.id;
    rel.sync().unwrap();
    drop(rel);

    let request = RewriteRequest {
        memories: reliquary_memory::rewrite::MemorySelection {
            base: MemoryBase::AllCurrent,
            include_ids: vec![id_hex(superseded)],
            exclude_ids: vec![id_hex(superseded)],
            include_archived: false,
            include_superseded: false,
            drop_known_duplicates: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let plan = plan_rewrite_file(&source, &request).unwrap();
    assert_eq!(plan.retained_memories, 1);
    let reasons = plan
        .memory_decisions
        .iter()
        .map(|decision| {
            (
                decision.id.as_str(),
                decision.retain,
                decision.reason.as_str(),
            )
        })
        .collect::<Vec<_>>();
    let ordinary_hex = id_hex(ordinary);
    let archived_hex = id_hex(archived);
    let superseded_hex = id_hex(superseded);
    assert!(
        reasons
            .iter()
            .any(|(id, retain, reason)| *id == ordinary_hex.as_str()
                && *retain
                && *reason == "current Memory")
    );
    assert!(
        reasons
            .iter()
            .any(|(id, retain, reason)| *id == archived_hex.as_str()
                && !*retain
                && *reason == "archived Memory excluded")
    );
    assert!(
        reasons
            .iter()
            .any(|(id, retain, reason)| *id == superseded_hex.as_str()
                && !*retain
                && *reason == "explicitly excluded")
    );

    let dedupe_request = RewriteRequest {
        memories: reliquary_memory::rewrite::MemorySelection {
            drop_known_duplicates: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let dedupe_plan = plan_rewrite_file(&source, &dedupe_request).unwrap();
    assert_eq!(dedupe_plan.retained_memories, 2);
    assert!(dedupe_plan.memory_decisions.iter().any(|decision| {
        decision.id == superseded_hex
            && !decision.retain
            && decision.reason == "known superseded duplicate"
    }));

    let freshness_request = RewriteRequest {
        memories: reliquary_memory::rewrite::MemorySelection {
            minimum_effective_freshness: Some(i16::MAX),
            ..Default::default()
        },
        ..Default::default()
    };
    let freshness_plan = plan_rewrite_file(&source, &freshness_request).unwrap();
    assert_eq!(freshness_plan.retained_memories, 0);
    assert!(
        freshness_plan
            .memory_decisions
            .iter()
            .all(|decision| !decision.retain
                && decision.reason == "below minimum effective Freshness")
    );
}

#[test]
fn dry_run_planning_and_output_refusal_preserve_both_files() {
    let temp = Temp::new();
    let source = temp.path("source.rel");
    let output = temp.path("existing.rel");
    seed_rel(&source);
    let source_hash = digest(&source);
    let plan = plan_rewrite_file(&source, &RewriteRequest::default()).unwrap();
    assert_eq!(plan.retained_memories, 2);
    assert!(!output.exists());
    assert_eq!(digest(&source), source_hash);

    fs::write(&output, b"existing destination").unwrap();
    let destination_hash = digest(&output);
    assert!(rewrite_file(&source, &output, &RewriteRequest::default()).is_err());
    assert_eq!(digest(&source), source_hash);
    assert_eq!(digest(&output), destination_hash);
}

#[test]
fn independent_rewrite_refuses_mounted_writer_and_accepts_shared_readers() {
    let temp = Temp::new();
    let source = temp.path("leased-source.rel");
    let output = temp.path("leased-output.rel");
    seed_rel(&source);
    let before = digest(&source);
    let mounted = Cva::open(&source).unwrap();
    assert!(plan_rewrite_file(&source, &RewriteRequest::default()).is_err());
    assert!(!output.exists());
    drop(mounted);
    let snapshot = Cva::open_read_only(&source).unwrap();
    let plan = plan_rewrite_file(&source, &RewriteRequest::default()).unwrap();
    rewrite_from_plan(&source, &output, &plan).unwrap();
    assert_eq!(digest(&source), before);
    drop(snapshot);
    assert_eq!(Cva::open(&output).unwrap().memory_ids().len(), 2);
}

#[test]
fn accepted_plan_rejects_source_drift_before_creating_output() {
    let temp = Temp::new();
    let source_path = temp.path("drift-source.rel");
    let output_path = temp.path("drift-output.rel");
    seed_rel(&source_path);
    let plan = plan_rewrite_file(&source_path, &RewriteRequest::default()).unwrap();
    let mut source = Cva::open(&source_path).unwrap();
    source
        .append_node(
            "later".into(),
            "drift-conversation".into(),
            None,
            "user".into(),
            100,
            "source changed after planning",
        )
        .unwrap();
    source.sync().unwrap();
    drop(source);
    let changed_hash = digest(&source_path);
    assert!(rewrite_from_plan(&source_path, &output_path, &plan).is_err());
    assert!(!output_path.exists());
    assert_eq!(digest(&source_path), changed_hash);
}

#[test]
fn zero_one_and_multiple_profiles_compact_rows_to_retained_memories() {
    let temp = Temp::new();
    let source = temp.path("profile-source.rel");
    let mut rel = Cva::create(&source).unwrap();
    let first_memory = rel
        .publish_memory(None, 0, draft("first", "first", "first vector body"))
        .unwrap()
        .0
        .id;
    let second_memory = rel
        .publish_memory(None, 0, draft("second", "second", "second vector body"))
        .unwrap()
        .0
        .id;
    let endpoint_a = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 101);
    let profile_a = rel.establish_compatibility_profile(&endpoint_a).unwrap();
    rel.build_missing_memory_vectors(profile_a.id, &endpoint_a)
        .unwrap();
    let endpoint_b = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 202);
    let profile_b = rel.establish_compatibility_profile(&endpoint_b).unwrap();
    rel.build_missing_memory_vectors(profile_b.id, &endpoint_b)
        .unwrap();
    rel.sync().unwrap();
    drop(rel);

    let zero_output = temp.path("zero-profiles.rel");
    rewrite_file(&source, &zero_output, &RewriteRequest::default()).unwrap();
    let zero = Cva::open(&zero_output).unwrap();
    assert_eq!(zero.compatibility_profile_stats().profiles, 0);
    assert_eq!(zero.memory_vector_stats().bindings, 0);
    assert_eq!(zero.packed_vector_stats().rows, 0);
    drop(zero);

    let one_output = temp.path("one-profile.rel");
    let one_request = RewriteRequest {
        profiles: vec![hex_profile(profile_a.id.0)],
        memories: reliquary_memory::rewrite::MemorySelection {
            exclude_ids: vec![id_hex(second_memory)],
            ..Default::default()
        },
        ..Default::default()
    };
    let one_report = rewrite_file(&source, &one_output, &one_request).unwrap();
    assert_eq!(one_report.plan.selected_profiles.len(), 1);
    let one = Cva::open(&one_output).unwrap();
    assert_eq!(one.compatibility_profile_stats().profiles, 1);
    assert_eq!(one.memory_vector_stats().bindings, 1);
    assert_eq!(one.packed_vector_stats().rows, 1);
    assert_eq!(one.memory_ids(), vec![first_memory]);
    drop(one);

    let many_output = temp.path("many-profiles.rel");
    let many_request = RewriteRequest {
        profiles: vec![hex_profile(profile_a.id.0), hex_profile(profile_b.id.0)],
        ..Default::default()
    };
    let many_report = rewrite_file(&source, &many_output, &many_request).unwrap();
    assert_eq!(many_report.plan.selected_profiles.len(), 2);
    let many = Cva::open(&many_output).unwrap();
    assert_eq!(many.compatibility_profile_stats().profiles, 2);
    assert_eq!(many.memory_vector_stats().bindings, 4);
    assert_eq!(many.packed_vector_stats().rows, 4);

    let phy_source = temp.path("profile-source.phy");
    let phy_output = temp.path("profile-phy-output.phy");
    let mut phy = Phylactery::create(&phy_source).unwrap();
    phy.publish_memory(
        None,
        0,
        draft("phy-first", "first", "first PHY vector body"),
    )
    .unwrap();
    phy.publish_memory(
        None,
        0,
        draft("phy-second", "second", "second PHY vector body"),
    )
    .unwrap();
    let phy_endpoint_a = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 303);
    let phy_profile_a = phy
        .establish_compatibility_profile(&phy_endpoint_a)
        .unwrap();
    phy.build_missing_memory_vectors(phy_profile_a.id, &phy_endpoint_a)
        .unwrap();
    let phy_endpoint_b = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 404);
    let phy_profile_b = phy
        .establish_compatibility_profile(&phy_endpoint_b)
        .unwrap();
    phy.build_missing_memory_vectors(phy_profile_b.id, &phy_endpoint_b)
        .unwrap();
    phy.sync().unwrap();
    drop(phy);
    let phy_request = RewriteRequest {
        profiles: vec![
            hex_profile(phy_profile_a.id.0),
            hex_profile(phy_profile_b.id.0),
        ],
        ..Default::default()
    };
    rewrite_file(&phy_source, &phy_output, &phy_request).unwrap();
    let rewritten_phy = Phylactery::open(&phy_output).unwrap();
    assert_eq!(rewritten_phy.compatibility_profile_stats().profiles, 2);
    assert_eq!(rewritten_phy.memory_vector_stats().bindings, 4);
    assert_eq!(rewritten_phy.packed_vector_stats().rows, 4);
}

#[test]
fn selected_rel_profile_preserves_compacted_archive_generation() {
    let temp = Temp::new();
    let source = temp.path("archive-profile-source.rel");
    let output = temp.path("archive-profile-output.rel");
    let mut rel = Cva::create(&source).unwrap();
    rel.append_node(
        "first".into(),
        "conversation".into(),
        None,
        "user".into(),
        1,
        "first source turn",
    )
    .unwrap();
    rel.append_node(
        "second".into(),
        "conversation".into(),
        Some("first".into()),
        "assistant".into(),
        2,
        "second source turn",
    )
    .unwrap();
    let fragments = rel
        .materialize_path_fragments("conversation", "second", FragmentConfig::default(), true)
        .unwrap();
    assert!(!fragments.is_empty());
    rel.publish_memory(
        None,
        0,
        draft("archive-vector-memory", "memory", "vector body"),
    )
    .unwrap();
    let endpoint = SimulatedEmbeddingEndpoint::new(8, VectorNormalization::L2, 505);
    let profile = rel.establish_compatibility_profile(&endpoint).unwrap();
    rel.build_missing_memory_vectors(profile.id, &endpoint)
        .unwrap();
    rel.build_archive_vector_generation(profile.id, &endpoint)
        .unwrap();
    let source_fragments = rel.fragments().len();
    assert_eq!(rel.archive_vector_stats().rows as usize, source_fragments);
    rel.sync().unwrap();
    drop(rel);

    let request = RewriteRequest {
        profiles: vec![hex_profile(profile.id.0)],
        archive: ArchiveSelection::AllRetained,
        ..Default::default()
    };
    rewrite_file(&source, &output, &request).unwrap();
    let rewritten = Cva::open(&output).unwrap();
    let retained_fragments = rewritten.fragments().len();
    assert_eq!(retained_fragments, source_fragments);
    assert_eq!(
        rewritten.archive_vector_stats().rows as usize,
        retained_fragments
    );
    assert_eq!(rewritten.vector_generation_stats().generations, 1);
    assert_eq!(rewritten.vector_generation_stats().active_profiles, 1);
    assert!(rewritten.current_vector_generation(profile.id).is_some());
}

#[test]
fn phy_rewrite_preserves_authoritative_ego_identity_and_anchor() {
    let temp = Temp::new();
    let source_path = temp.path("ego-source.phy");
    let output_path = temp.path("ego-rewrite.phy");
    let mut source = Phylactery::create(&source_path).unwrap();
    let identity = source
        .create_ego_identity("Aster".into(), "Keep user-owned identity.".into())
        .unwrap();
    let anchor = source
        .create_ego_anchor(
            EgoAnchorPriority::High,
            "Keep this user-owned anchor.".into(),
        )
        .unwrap();
    source.sync().unwrap();
    drop(source);

    rewrite_file(&source_path, &output_path, &RewriteRequest::default()).unwrap();
    let rewritten = Phylactery::open(&output_path).unwrap();
    assert_eq!(rewritten.ego_identity(), Some(identity));
    assert_eq!(rewritten.ego_anchors(), vec![anchor]);
}

#[test]
fn rel_provenance_policy_preserves_closure_strips_deliberately_or_rejects() {
    let temp = Temp::new();
    let source_path = temp.path("provenance-source.rel");
    let mut source = Cva::create(&source_path).unwrap();
    source
        .append_node(
            "source".into(),
            "conversation".into(),
            None,
            "user".into(),
            1,
            "origin",
        )
        .unwrap();
    source
        .append_node(
            "content".into(),
            "conversation".into(),
            Some("source".into()),
            "assistant".into(),
            2,
            "content",
        )
        .unwrap();
    let episode = source
        .materialize_path_episodes(
            "conversation",
            "content",
            EpisodeConfig::default(),
            EpisodeOrigin::Live,
            Some((EpisodeBoundary::Explicit, 3)),
        )
        .unwrap()
        .created
        .into_iter()
        .next()
        .unwrap();
    let mut memory_draft = draft("provenance", "grounded", "Grounded memory");
    memory_draft.source_node_id = Some("source".into());
    memory_draft.content_source_conversation_id = Some("conversation".into());
    memory_draft.content_source_node_id = Some("content".into());
    memory_draft.source_episode_id = Some(episode.id);
    let id = source.publish_memory(None, 0, memory_draft).unwrap().0.id;
    source.sync().unwrap();
    drop(source);
    let mut episode_only = Cva::open(&source_path).unwrap();
    let mut revised = draft("episode-only", "episode", "Episode-only provenance");
    revised.source_node_id = Some("source".into());
    revised.source_episode_id = Some(episode.id);
    episode_only.publish_memory(None, 0, revised).unwrap();
    episode_only.sync().unwrap();
    drop(episode_only);
    let source_hash = digest(&source_path);

    let preserve_path = temp.path("provenance-preserved.rel");
    let preserve = RewriteRequest {
        archive: ArchiveSelection::ProvenanceClosureOnly,
        provenance: ProvenancePolicy::PreserveClosure,
        ..Default::default()
    };
    rewrite_file(&source_path, &preserve_path, &preserve).unwrap();
    let mut preserved = Cva::open(&preserve_path).unwrap();
    let preserved_memory = preserved.memory(id).unwrap();
    assert_eq!(preserved_memory.source_node_id.as_deref(), Some("source"));
    assert_eq!(
        preserved_memory.content_source_node_id.as_deref(),
        Some("content")
    );
    assert!(preserved.stats().nodes >= 2);
    assert!(preserved.stats().episodes >= 1);

    let strip_path = temp.path("provenance-stripped.rel");
    let strip = RewriteRequest {
        provenance: ProvenancePolicy::Strip,
        ..Default::default()
    };
    rewrite_file(&source_path, &strip_path, &strip).unwrap();
    let mut stripped = Cva::open(&strip_path).unwrap();
    let stripped_memory = stripped.memory(id).unwrap();
    assert_eq!(stripped_memory.source_node_id, None);
    assert_eq!(stripped_memory.content_source_conversation_id, None);
    assert_eq!(stripped_memory.content_source_node_id, None);
    assert_eq!(stripped_memory.source_episode_id, None);

    let reject_path = temp.path("provenance-rejected.rel");
    let reject = RewriteRequest {
        provenance: ProvenancePolicy::Reject,
        ..Default::default()
    };
    assert!(rewrite_file(&source_path, &reject_path, &reject).is_err());
    assert!(!reject_path.exists());
    assert_eq!(digest(&source_path), source_hash);
}

#[test]
fn preserve_owner_policy_keeps_the_source_owner_identity() {
    let temp = Temp::new();
    let source_path = temp.path("preserve-owner-source.rel");
    let output_path = temp.path("preserve-owner-output.rel");
    seed_rel(&source_path);
    let source = Cva::open(&source_path).unwrap();
    let owner = source.owner_id();
    let owner_uuid = source.owner_uuid().unwrap();
    drop(source);
    let request = RewriteRequest {
        identity: IdentityPolicy::PreserveOwner,
        freshness: FreshnessRewriteMode::Reset,
        ..Default::default()
    };
    rewrite_file(&source_path, &output_path, &request).unwrap();
    let output = Cva::open(&output_path).unwrap();
    assert_eq!(output.owner_id(), owner);
    assert_ne!(
        output.history_generation(),
        reliquary_memory::HistoryGenerationId::ZERO
    );
    let genesis = output.rewrite_genesis().unwrap();
    assert_eq!(
        genesis.predecessor_generation,
        reliquary_memory::HistoryGenerationId::ZERO
    );
    assert_eq!(genesis.predecessor_owner, owner_uuid);
    drop(output);
    assert!(matches!(
        Cva::compare(&source_path, &output_path),
        Err(reliquary_memory::CvaReconcileError::HistoryGenerationConflict { .. })
    ));
}
#[test]
fn retained_revisions_and_recomputed_freshness_reopen_with_fresh_history() {
    use reliquary_memory::rewrite::RevisionPolicy;
    let temp = Temp::new();
    let source = temp.path("revision-source.rel");
    let output = temp.path("revision-output.rel");
    let ids = seed_rel(&source);
    let mut rel = Cva::open(&source).unwrap();
    let mut revised = draft("revision-two", "one", "first memory");
    revised.archived = true;
    rel.publish_memory(Some(ids[0]), 1, revised).unwrap();
    rel.sync().unwrap();
    drop(rel);
    let mut request = RewriteRequest::default();
    request.memories.revisions = RevisionPolicy::AllRetained;
    request.freshness = FreshnessRewriteMode::RecomputeRetainedHistory;
    let plan = plan_rewrite_file(&source, &request).unwrap();
    assert_eq!(
        plan.selected_revisions
            .iter()
            .find(|r| r.memory_id == id_hex(ids[0]))
            .unwrap()
            .source_revisions,
        vec![1, 2]
    );
    rewrite_from_plan(&source, &output, &plan).unwrap();
    let mut rewritten = Cva::open(&output).unwrap();
    assert_eq!(
        rewritten.memory_revision(ids[0], 1).unwrap().content,
        "first memory"
    );
    assert!(rewritten.memory_revision(ids[0], 2).unwrap().archived);
    assert_eq!(rewritten.freshness_record(ids[0]).unwrap().score, 100);
}

#[test]
fn rewritten_same_generation_branches_reconcile_with_explicit_baselines() {
    let temp = Temp::new();
    let source = temp.path("lineage-source.rel");
    let base = temp.path("lineage-base.rel");
    let left = temp.path("lineage-left.rel");
    let right = temp.path("lineage-right.rel");
    let output = temp.path("lineage-merged.rel");
    let ids = seed_rel(&source);
    rewrite_file(&source, &base, &RewriteRequest::default()).unwrap();
    fs::copy(&base, &left).unwrap();
    fs::copy(&base, &right).unwrap();
    for (path, id) in [(&left, "left"), (&right, "right")] {
        let mut rel = Cva::open(path).unwrap();
        rel.append_node(id.into(), id.into(), None, "user".into(), 100, "tail")
            .unwrap();
        rel.sync().unwrap();
    }
    Cva::reconcile(&left, &right, &output).unwrap();
    let merged = Cva::open(&output).unwrap();
    assert_eq!(
        merged.history_generation(),
        Cva::open(&base).unwrap().history_generation()
    );
    assert_eq!(merged.freshness_record(ids[0]).unwrap().score, 100);
}

#[test]
fn provenance_closure_keeps_project_file_bindings_before_attachment_turns() {
    use reliquary_memory::{
        IncomingTurn, ProjectFileRef, ProjectRepositoryKind, ProjectRepositoryRef,
        ProjectRevisionRef,
    };
    use sha2::{Digest, Sha256};
    let temp = Temp::new();
    let source = temp.path("project-attachment-source.rel");
    let output = temp.path("project-attachment-output.rel");
    let mut rel = Cva::create_project(&source).unwrap();
    let reference = ProjectFileRef {
        revision: ProjectRevisionRef {
            repository: ProjectRepositoryRef {
                kind: ProjectRepositoryKind::Lore,
                repository_id: "stable-repository".into(),
                project_path: ".".into(),
            },
            revision: "accepted-revision".into(),
        },
        path: "uploads/plan.txt".into(),
        content_hash: Some(Sha256::digest(b"source").into()),
    };
    let file = rel
        .register_project_file("plan.txt".into(), None, 6, reference.clone())
        .unwrap();
    let legacy = rel
        .store_file("legacy-plan.txt".into(), None, b"source")
        .unwrap();
    rel.bind_legacy_project_file(legacy.id, reference.clone())
        .unwrap();
    rel.ingest_turn(IncomingTurn {
        id: "turn".into(),
        conversation_id: "project-conversation".into(),
        parent_id: None,
        role: "user".into(),
        principal_id: None,
        timestamp_ns: 10,
        content: "attached plan".into(),
        attachments: vec![],
        project_attachments: vec![file.clone(), legacy.clone()],
    })
    .unwrap();
    let mut memory = draft("project-memory", "plan", "accepted plan");
    memory.content_source_conversation_id = Some("project-conversation".into());
    memory.content_source_node_id = Some("turn".into());
    let memory_id = rel.publish_memory(None, 0, memory).unwrap().0.id;
    rel.link_file_to_memory(file.id, memory_id).unwrap();
    rel.sync().unwrap();
    drop(rel);
    rewrite_file(&source, &output, &RewriteRequest::default()).unwrap();
    let rewritten = Cva::open(&output).unwrap();
    assert_eq!(rewritten.files_for_memory(memory_id).len(), 1);
    assert_eq!(rewritten.project_file_ref(file.id), Some(reference.clone()));
    assert_eq!(
        rewritten.project_file_ref(legacy.id),
        Some(reference.clone())
    );
    assert_eq!(rewritten.stats().source_attachments, 2);
}

#[test]
fn frozen_plan_tampering_is_rejected_before_creating_output() {
    let temp = Temp::new();
    let source = temp.path("plan-source.rel");
    let output = temp.path("plan-output.rel");
    seed_rel(&source);
    let mut plan = plan_rewrite_file(&source, &RewriteRequest::default()).unwrap();
    plan.selected_revisions[0].source_revisions.clear();
    assert!(
        rewrite_from_plan(&source, &output, &plan)
            .unwrap_err()
            .to_string()
            .contains("canonical source selection")
    );
    assert!(!output.exists());
}

#[test]
fn terminal_memory_selection_preserves_tombstone_and_available_revision() {
    let temp = Temp::new();
    let source = temp.path("terminal-source.rel");
    let output = temp.path("terminal-output.rel");
    let ids = seed_rel(&source);
    let mut rel = Cva::open(&source).unwrap();
    rel.tombstone_memory(ids[0], 1).unwrap();
    rel.sync().unwrap();
    drop(rel);
    let mut request = RewriteRequest::default();
    request.memories.include_tombstoned = true;
    rewrite_file(&source, &output, &request).unwrap();
    let mut output = Cva::open(&output).unwrap();
    assert!(matches!(
        output.memory_lifecycle_state(ids[0]).unwrap(),
        reliquary_memory::owner_lifecycle::LifecycleState::Tombstoned(_)
    ));
    assert_eq!(
        output.memory_revision(ids[0], 1).unwrap().content,
        "first memory"
    );
}

#[test]
fn graph_baseline_preserves_admitted_nodes_after_last_edge_is_removed() {
    use reliquary_memory::GraphRelationKind;
    let temp = Temp::new();
    let source = temp.path("isolated-source.rel");
    let output = temp.path("isolated-output.rel");
    let ids = seed_rel(&source);
    let mut rel = Cva::open(&source).unwrap();
    rel.set_memory_relation(
        ids[1],
        ids[0],
        GraphRelationKind::Topical,
        true,
        rel.graph_version(),
    )
    .unwrap();
    rel.set_memory_relation(
        ids[1],
        ids[0],
        GraphRelationKind::Topical,
        false,
        rel.graph_version(),
    )
    .unwrap();
    assert_eq!(rel.graph_stats().memory_nodes, 2);
    assert_eq!(rel.graph_stats().memory_active_relations, 0);
    rel.sync().unwrap();
    drop(rel);
    rewrite_file(&source, &output, &RewriteRequest::default()).unwrap();
    let rel = Cva::open(&output).unwrap();
    assert_eq!(rel.graph_stats().memory_nodes, 2);
    assert_eq!(rel.graph_stats().memory_active_relations, 0);
}

#[test]
fn graph_baseline_rejects_rechecksummed_wrong_owner() {
    use sha2::{Digest, Sha256};
    let temp = Temp::new();
    let source = temp.path("baseline-owner-source.rel");
    let output = temp.path("baseline-owner-output.rel");
    seed_rel(&source);
    rewrite_file(&source, &output, &RewriteRequest::default()).unwrap();
    let mut bytes = fs::read(&output).unwrap();
    let start = bytes.windows(8).position(|w| w == b"CVAGBL01").unwrap();
    let len = u64::from_le_bytes(bytes[start - 8..start].try_into().unwrap()) as usize;
    let end = start + len;
    bytes[start + 8] ^= 1;
    let digest = Sha256::digest(&bytes[start..end - 32]);
    bytes[end - 32..end].copy_from_slice(&digest);
    fs::write(&output, bytes).unwrap();
    assert!(Cva::open(&output).is_err());
}

#[test]
fn frozen_plan_refuses_replacement_with_identical_bytes() {
    let temp = Temp::new();
    let source = temp.path("replaced-source.rel");
    let replacement = temp.path("replacement.rel");
    let output = temp.path("replacement-output.rel");
    seed_rel(&source);
    let plan = plan_rewrite_file(&source, &RewriteRequest::default()).unwrap();
    let original = fs::read(&source).unwrap();
    fs::write(&replacement, &original).unwrap();
    fs::remove_file(&source).unwrap();
    fs::rename(&replacement, &source).unwrap();
    assert!(rewrite_from_plan(&source, &output, &plan).is_err());
    assert!(!output.exists());
    assert_eq!(fs::read(&source).unwrap(), original);
}
