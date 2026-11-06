use super::*;
use reliquary_archive::{Archive, Branch, TurnInput};
use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
use reliquary_ego_store::{EgoIdentityId, EgoStore};
use reliquary_freshness_store::{FreshnessMemoryId, FreshnessStore};
use reliquary_memory_core::{MemoryDraft, MemoryStore};
use reliquary_owner_session::GraphAdmission;
use reliquary_perception_store::{
    EntityDraft, PerceptionStore, RelationshipDraft, RelationshipEntityRef, RelationshipMemoryRef,
    RelationshipParticipant,
};
use reliquary_vector_store::{
    AcceptedArchiveMapping, AcceptedMemoryMapping, ArchiveFragmentId, CompatibilityProbeReference,
    CompatibilityProfile, CompatibilityProfileStore, EmbeddingMode, MemoryVectorStore,
    PackedVectorStore, VectorNormalization,
};
use reliquary_vector_store::{ArchiveVectorStore, VectorGenerationStore};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        std::fs::create_dir_all(&root).unwrap();
        Self(root.join(format!(
            "m01-plan-{}-{}.phy",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn draft(mutation: &str, parent_id: Option<MemoryId>) -> MemoryDraft {
    MemoryDraft {
        title: mutation.into(),
        content: format!("body-{mutation}"),
        category: "fact".into(),
        memory_type: "semantic".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "user".into(),
        lifecycle_state: "active".into(),
        archived: false,
        parent_id,
        superseded_by: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: None,
        source_time_ns: Some(10),
        temporal_inference: None,
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn test_profile(seed: u8) -> CompatibilityProfile {
    let mut profile = CompatibilityProfile {
        id: reliquary_vector_store::CompatibilityProfileId([0; 32]),
        dimensions: 2,
        normalization: VectorNormalization::None,
        probe_suite_version: 1,
        compatibility_policy_version: 2,
        references: [
            EmbeddingMode::Query,
            EmbeddingMode::Query,
            EmbeddingMode::Document,
            EmbeddingMode::Document,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, mode)| CompatibilityProbeReference {
            mode,
            vector: vec![seed as f32, index as f32],
        })
        .collect(),
    };
    profile.id = reliquary_vector_store::profile::compatibility_profile_id(&profile);
    profile
}

fn phy_fixture() -> (Fixture, PhySession, MemoryId, MemoryId) {
    let path = Fixture::new();
    let owner = [41; 16];
    let mut container = Container::create(&path.0, ContainerIdentity::phylactery(), owner).unwrap();
    let mut memories = MemoryStore::create(&mut container).unwrap();
    PackedVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    CompatibilityProfileStore::default()
        .initialize(&mut container)
        .unwrap();
    MemoryVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    let parent_published = memories
        .publish(&mut container, None, 0, draft("parent", None))
        .unwrap();
    let parent = parent_published.revision.record.id;
    let child_published = memories
        .publish(&mut container, None, 0, draft("child", Some(parent)))
        .unwrap();
    let child = child_published.revision.record.id;
    let mut profiles = CompatibilityProfileStore::open(&container).unwrap();
    let mut packed = PackedVectorStore::open(&container).unwrap();
    let mut mappings = MemoryVectorStore::open(&container, &packed, &profiles).unwrap();
    for (index, profile) in [test_profile(1), test_profile(2)].into_iter().enumerate() {
        profiles.put(&mut container, profile.clone()).unwrap();
        let rows = vec![vec![1.0 + index as f32, 0.0], vec![0.0, 1.0 + index as f32]];
        let packed_id = packed.put_f32_rows(&mut container, &rows).unwrap();
        mappings
            .put(
                &mut container,
                &packed,
                &profiles,
                AcceptedMemoryMapping {
                    profile: profile.id,
                    packed: packed_id,
                    packed_rows: 2,
                    body_ids: vec![
                        reliquary_vector_store::MemoryBodyId(
                            parent_published.revision.record.body_id.0,
                        ),
                        reliquary_vector_store::MemoryBodyId(
                            child_published.revision.record.body_id.0,
                        ),
                    ],
                },
            )
            .unwrap();
    }
    container.sync().unwrap();
    drop(memories);
    drop(container);
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let session = PhySession::open_path(&path.0, &graph, []).unwrap();
    (path, session, parent, child)
}

#[test]
fn selected_multiple_profiles_retain_exact_memory_vector_coverage() {
    let (source, session, parent, child) = phy_fixture();
    let destination = Fixture::new();
    let mut policy = phy_policy();
    policy.profile_ids = [test_profile(1).id.0, test_profile(2).id.0]
        .into_iter()
        .collect();
    let plan = plan_phy_rewrite(&source.0, &session, policy).unwrap();
    let graph = GraphAdmission::for_replay(session.owner_id(), [], [], [], [], []);
    execute_memory_rewrite(&plan, &source.0, &destination.0, Some([44; 16]), &graph).unwrap();
    let reopened_graph = GraphAdmission::for_replay([44; 16], [], [], [], [], []);
    let reopened = PhySession::open_path(&destination.0, &reopened_graph, []).unwrap();
    for profile in [test_profile(1), test_profile(2)] {
        for id in [parent, child] {
            let body = reopened.current_memory(id).unwrap().record.body_id;
            assert!(
                reopened
                    .memory_vector_row(profile.id, body)
                    .unwrap()
                    .is_some()
            );
        }
    }
}

#[test]
fn tombstone_duplicate_lifecycle_and_revision_filters_are_deterministic() {
    let (source, session, parent, _) = phy_fixture();
    let owner = session.owner_id();
    drop(session);
    let mut container = Container::open_writer(&source.0).unwrap();
    let mut memories = MemoryStore::open(&container).unwrap();
    let target = memories
        .publish(&mut container, None, 0, draft("canonical", None))
        .unwrap()
        .revision
        .record
        .id;
    let mut duplicate = draft("duplicate", None);
    duplicate.superseded_by = Some(target);
    let duplicate = memories
        .publish(&mut container, None, 0, duplicate)
        .unwrap()
        .revision
        .record
        .id;
    let mut tombstone = draft("tombstoned", None);
    tombstone.lifecycle_state = "inactive".into();
    let tombstone = memories
        .publish(&mut container, None, 0, tombstone)
        .unwrap()
        .revision
        .record
        .id;
    memories.tombstone(&mut container, tombstone, 1).unwrap();
    let mut parent_v2 = draft("parent-revise", None);
    parent_v2.title = "parent".into();
    parent_v2.content = "body-parent".into();
    parent_v2.updated_at_ns = 20;
    let updated = memories
        .publish(&mut container, Some(parent), 1, parent_v2)
        .unwrap()
        .revision;
    container.sync().unwrap();
    drop(memories);
    drop(container);

    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let session = PhySession::open_path(&source.0, &graph, []).unwrap();
    let mut policy = phy_policy();
    policy.memories.include_tombstoned = true;
    policy.memories.drop_known_duplicates = true;
    let plan = plan_phy_rewrite(&source.0, &session, policy.clone()).unwrap();
    assert!(plan.selection.dropped_memory_ids.contains(&duplicate.0));

    policy.memories.include = [tombstone.0].into_iter().collect();
    policy.memories.lifecycle_states = ["inactive".to_owned()].into_iter().collect();
    let tombstone_plan = plan_phy_rewrite(&source.0, &session, policy).unwrap();
    let destination = Fixture::new();
    execute_memory_rewrite(
        &tombstone_plan,
        &source.0,
        &destination.0,
        Some([45; 16]),
        &graph,
    )
    .unwrap();
    let reopened_graph = GraphAdmission::for_replay([45; 16], [], [], [], [], []);
    let reopened = PhySession::open_path(&destination.0, &reopened_graph, []).unwrap();
    assert!(
        reopened
            .memory_history(tombstone)
            .unwrap()
            .unwrap()
            .tombstone
            .is_some()
    );

    let mut revision_policy = phy_policy();
    revision_policy.memories.include = [parent.0].into_iter().collect();
    revision_policy
        .memories
        .retain_revisions_since_memory_version = Some(updated.record.memory_version);
    let revision_plan = plan_phy_rewrite(&source.0, &session, revision_policy).unwrap();
    assert_eq!(revision_plan.selection.retained_revisions, 1);
}

#[test]
fn phy_private_ego_is_retained_only_in_verified_derived_mode() {
    let (source, session, _, _) = phy_fixture();
    let owner = session.owner_id();
    drop(session);
    let mut container = Container::open_writer(&source.0).unwrap();
    let mut ego = EgoStore::open(&container).unwrap();
    let identity = EgoIdentityId([9; 16]);
    ego.create_identity(
        &mut container,
        identity,
        "Owner".into(),
        "Private owner identity".into(),
    )
    .unwrap();
    container.sync().unwrap();
    drop(ego);
    drop(container);
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let session = PhySession::open_path(&source.0, &graph, []).unwrap();
    let mut policy = phy_policy();
    policy.derived_state = DerivedStateRewriteMode::RetainVerified;
    let plan = plan_phy_rewrite(&source.0, &session, policy).unwrap();
    let destination = Fixture::new();
    execute_memory_rewrite(&plan, &source.0, &destination.0, Some([46; 16]), &graph).unwrap();
    let reopened_graph = GraphAdmission::for_replay([46; 16], [], [], [], [], []);
    let reopened = PhySession::open_path(&destination.0, &reopened_graph, []).unwrap();
    assert_eq!(
        reopened.ego_identity(identity).unwrap().text,
        "Private owner identity"
    );
}

fn phy_policy() -> RewritePolicy {
    RewritePolicy {
        archive: ArchiveRewriteMode::Omit,
        freshness: FreshnessRewriteMode::Reset,
        derived_state: DerivedStateRewriteMode::Omit,
        ..RewritePolicy::default()
    }
}

fn rel_fixture() -> (
    Fixture,
    RelSession,
    MemoryId,
    reliquary_vector_store::CompatibilityProfileId,
) {
    let path = Fixture::new();
    let owner = [51; 16];
    let mut container = Container::create(
        &path.0,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        owner,
    )
    .unwrap();
    let mut memories = MemoryStore::create(&mut container).unwrap();
    let mut freshness = FreshnessStore::new(Some(owner));
    freshness.initialize(&mut container).unwrap();
    PackedVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    CompatibilityProfileStore::default()
        .initialize(&mut container)
        .unwrap();
    ArchiveVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    MemoryVectorStore::default()
        .initialize(&mut container)
        .unwrap();
    VectorGenerationStore::empty()
        .initialize(&mut container)
        .unwrap();
    let mut archive = Archive::open(&container).unwrap();
    archive
        .accept_turn(
            &mut container,
            TurnInput {
                id: "turn-1".into(),
                conversation_id: "conversation".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("principal".into()),
                timestamp_ns: 10,
                content: "Remember the blue door.".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
        )
        .unwrap();
    archive
        .put_branch(
            &mut container,
            Branch {
                id: "main".into(),
                conversation_id: "conversation".into(),
                leaf_node_id: "turn-1".into(),
                canonical: true,
            },
        )
        .unwrap();
    let fragments = archive
        .define_branch_fragments(
            &mut container,
            "conversation",
            "main",
            reliquary_archive::FragmentConfig {
                turns: 1,
                overlap: 0,
            },
            true,
        )
        .unwrap();
    let mut memory = draft("rel-memory", None);
    memory.content_source_conversation_id = Some("conversation".into());
    memory.content_source_node_id = Some("turn-1".into());
    let published = memories.publish(&mut container, None, 0, memory).unwrap();
    let id = published.revision.record.id;
    let profile = test_profile(3);
    let mut profiles = CompatibilityProfileStore::open(&container).unwrap();
    profiles.put(&mut container, profile.clone()).unwrap();
    let mut packed = PackedVectorStore::open(&container).unwrap();
    let mut memory_vectors = MemoryVectorStore::open(&container, &packed, &profiles).unwrap();
    let memory_packed = packed
        .put_f32_rows(&mut container, &[vec![3.0, 1.0]])
        .unwrap();
    memory_vectors
        .put(
            &mut container,
            &packed,
            &profiles,
            AcceptedMemoryMapping {
                profile: profile.id,
                packed: memory_packed,
                packed_rows: 1,
                body_ids: vec![reliquary_vector_store::MemoryBodyId(
                    published.revision.record.body_id.0,
                )],
            },
        )
        .unwrap();
    let mut archive_vectors = ArchiveVectorStore::open(&container, &packed).unwrap();
    let archive_packed = packed
        .put_f32_rows(&mut container, &[vec![2.0, 2.0]])
        .unwrap();
    let archive_vector = archive_vectors
        .put(
            &mut container,
            &packed,
            AcceptedArchiveMapping {
                packed_vector_id: archive_packed,
                packed_rows: 1,
                fragment_ids: vec![ArchiveFragmentId(fragments[0].fragment.id.0)],
                max_fragment_archive_version: archive
                    .node_archive_version("conversation", "turn-1")
                    .unwrap(),
            },
        )
        .unwrap();
    packed
        .put_f32_rows(&mut container, &[vec![4.0; 128]])
        .unwrap();
    let mut generations =
        VectorGenerationStore::open(&container, &profiles, &archive_vectors).unwrap();
    generations
        .publish(
            &mut container,
            &profiles,
            &archive_vectors,
            profile.id,
            archive_vector,
            archive.archive_version(),
        )
        .unwrap();
    freshness
        .create_birth(&mut container, FreshnessMemoryId(id.0), 1, true)
        .unwrap();
    container.sync().unwrap();
    drop(archive);
    drop(freshness);
    drop(memories);
    drop(container);
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let session = RelSession::open_path(&path.0, &graph, [], []).unwrap();
    (path, session, id, profile.id)
}

#[test]
fn storage_reclamation_relocates_rel_generation_receipts_and_reopens_history() {
    use reliquary_owner_session::{VerifiedOwnerCopyPlan, reclaim_storage_copy};
    use sha2::{Digest, Sha256};

    let (source, session, memory, profile) = rel_fixture();
    let graph = GraphAdmission::for_replay(session.owner_id(), [], [], [], [], []);
    let destination = Fixture::new();
    let history = session.memory_history(memory).unwrap();
    let vector = session.archive_vector_infos();
    drop(session);
    let orphan = b"unreferenced attachment bytes";
    let mut frame = b"RELFILE1".to_vec();
    frame.extend_from_slice(&Sha256::digest(orphan));
    frame.extend_from_slice(&(orphan.len() as u32).to_le_bytes());
    frame.extend_from_slice(orphan);
    let mut writer = Container::open_writer(&source.0).unwrap();
    writer.append(&frame).unwrap();
    writer.sync().unwrap();
    drop(writer);
    let before = fs::read(&source.0).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let report = reclaim_storage_copy(&plan, &source.0, &destination.0, &graph).unwrap();
    assert_eq!(report.removed_frames, 2);
    assert!(report.candidate_bytes < report.source_bytes);
    assert_eq!(fs::read(&source.0).unwrap(), before);
    let reopened = RelSession::open_path(&destination.0, &graph, [], []).unwrap();
    assert_eq!(reopened.memory_history(memory).unwrap(), history);
    assert_eq!(reopened.archive_vector_infos(), vector);
    let body = reopened.current_memory(memory).unwrap().record.body_id;
    assert_eq!(
        reopened.memory_vector_row(profile, body).unwrap(),
        Some(vec![3.0, 1.0])
    );
    let noop = Fixture::new();
    let plan = VerifiedOwnerCopyPlan::prepare(&destination.0).unwrap();
    let report = reclaim_storage_copy(&plan, &destination.0, &noop.0, &graph).unwrap();
    assert_eq!(report.removed_frames, 0);
    assert_eq!(
        fs::read(&noop.0).unwrap(),
        fs::read(&destination.0).unwrap()
    );
    let mut unknown = Container::open_writer(&noop.0).unwrap();
    unknown.append(b"UNKNOWN1").unwrap();
    unknown.sync().unwrap();
    drop(unknown);
    let untouched = fs::read(&noop.0).unwrap();
    let refused = Fixture::new();
    let plan = VerifiedOwnerCopyPlan::prepare(&noop.0).unwrap();
    let error = reclaim_storage_copy(&plan, &noop.0, &refused.0, &graph).unwrap_err();
    assert!(error.contains("unsupported reclamation frame family"));
    assert!(!refused.0.exists());
    assert_eq!(fs::read(&noop.0).unwrap(), untouched);
}

#[test]
fn storage_reclamation_refuses_malformed_unreceipted_generation_without_output() {
    use reliquary_owner_session::{VerifiedOwnerCopyPlan, reclaim_storage_copy};

    let (source, session, _, _) = rel_fixture();
    let graph = GraphAdmission::for_replay(session.owner_id(), [], [], [], [], []);
    drop(session);
    let mut writer = Container::open_writer(&source.0).unwrap();
    let mut orphan_generation = vec![0u8; 112];
    orphan_generation[..8].copy_from_slice(b"RELVGEN1");
    writer.append(&orphan_generation).unwrap();
    writer.sync().unwrap();
    drop(writer);
    let before = fs::read(&source.0).unwrap();
    let destination = Fixture::new();
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let error = reclaim_storage_copy(&plan, &source.0, &destination.0, &graph).unwrap_err();
    assert!(error.contains("invalid unactivated generation identity"));
    assert!(!destination.0.exists());
    assert_eq!(fs::read(&source.0).unwrap(), before);
}

#[test]
fn profile_repair_checks_current_rel_archive_generation() {
    use reliquary_owner_session::{ProfileRepairError, plan_profile_repair};

    let (source, session, _, profile) = rel_fixture();
    let graph = GraphAdmission::for_replay(session.owner_id(), [], [], [], [], []);
    assert_eq!(
        plan_profile_repair(&source.0, &graph, &[profile])
            .unwrap()
            .retained(),
        &[profile]
    );
    drop(session);
    let mut container = Container::open_writer(&source.0).unwrap();
    let mut archive = Archive::open(&container).unwrap();
    archive
        .accept_turn(
            &mut container,
            TurnInput {
                id: "turn-2".into(),
                conversation_id: "conversation".into(),
                parent_id: Some("turn-1".into()),
                role: "user".into(),
                principal_id: Some("principal".into()),
                timestamp_ns: 20,
                content: "New accepted Archive activity.".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
        )
        .unwrap();
    container.sync().unwrap();
    drop(container);
    assert_eq!(
        plan_profile_repair(&source.0, &graph, &[profile]),
        Err(ProfileRepairError::IncompleteArchiveCoverage(profile))
    );
}

#[test]
fn rel_rewrite_retains_required_archive_provenance_and_effective_freshness() {
    let (source, session, memory, profile) = rel_fixture();
    let destination = Fixture::new();
    let mut policy = RewritePolicy::default();
    policy.memories.include.insert(memory.0);
    policy.profile_ids.insert(profile.0);
    policy.derived_state = DerivedStateRewriteMode::Omit;
    let plan = plan_rel_rewrite(&source.0, &session, policy).unwrap();
    let source_graph = GraphAdmission::for_replay(session.owner_id(), [], [], [], [], []);
    execute_memory_rewrite(
        &plan,
        &source.0,
        &destination.0,
        Some([52; 16]),
        &source_graph,
    )
    .unwrap();
    let destination_graph = GraphAdmission::for_replay([52; 16], [], [], [], [], []);
    let reopened = RelSession::open_path(&destination.0, &destination_graph, [], []).unwrap();
    assert_eq!(
        reopened
            .archive_node("conversation", "turn-1")
            .map(|node| node.id.as_str()),
        Some("turn-1")
    );
    assert_eq!(
        reopened
            .freshness_record(FreshnessMemoryId(memory.0))
            .unwrap()
            .unwrap()
            .admitted_at_turn,
        Some(1)
    );
    let body = reopened.current_memory(memory).unwrap().record.body_id;
    assert!(reopened.memory_vector_row(profile, body).unwrap().is_some());
    assert_eq!(reopened.archive_vector_infos().len(), 1);
    let cut = reopened.current_historical_cut();
    let proof = reopened.historical_proof([52; 16], &cut).unwrap();
    assert_eq!(proof.owner_id, [52; 16]);
    assert!(proof.retained_records > 0);
}

#[test]
fn new_owner_execution_replays_selected_history_and_reopens_independently() {
    let (source, session, parent, child) = phy_fixture();
    let destination = Fixture::new();
    let source_before = std::fs::read(&source.0).unwrap();
    let mut policy = phy_policy();
    policy.memories.include.insert(child.0);
    policy.memories.retain_all_revisions = true;
    let plan = plan_phy_rewrite(&source.0, &session, policy).unwrap();
    let source_graph = GraphAdmission::for_replay(session.owner_id(), [], [], [], [], []);
    let report = execute_memory_rewrite(
        &plan,
        &source.0,
        &destination.0,
        Some([42; 16]),
        &source_graph,
    )
    .unwrap();
    assert_eq!(report.destination_owner_id, [42; 16]);
    assert_eq!(report.retained_memories, 2);
    assert_ne!(report.history_generation, [0; 32]);
    let lineage = reliquary_owner_session::fresh_history_lineage(&destination.0)
        .unwrap()
        .unwrap();
    assert_eq!(lineage.history_generation, report.history_generation);
    assert_eq!(
        lineage.predecessor_generation,
        reliquary_owner_session::owner_history_generation(&source.0).unwrap()
    );
    assert_eq!(lineage.source_owner_id, [41; 16]);
    assert_eq!(std::fs::read(&source.0).unwrap(), source_before);
    let destination_graph = GraphAdmission::for_replay([42; 16], [], [], [], [], []);
    let reopened = PhySession::open_path(&destination.0, &destination_graph, []).unwrap();
    assert_eq!(reopened.current_memory(parent).unwrap().record.id, parent);
    assert_eq!(reopened.current_memory(child).unwrap().record.id, child);
    let cut = reopened.current_historical_cut();
    let proof = reopened.historical_proof([42; 16], cut).unwrap();
    assert_eq!(proof.owner_id, [42; 16]);
    assert!(proof.retained_records >= 2);
}

#[test]
fn preserve_owner_records_a_new_generation_and_refuses_destination_collision() {
    let (source, session, _, _) = phy_fixture();
    let destination = Fixture::new();
    let mut policy = phy_policy();
    policy.identity = RewriteIdentityMode::PreserveOwner;
    let plan = plan_phy_rewrite(&source.0, &session, policy).unwrap();
    let source_cut = session.current_historical_cut();
    let source_proof = session
        .historical_proof(session.owner_id(), source_cut)
        .unwrap();
    let source_graph = GraphAdmission::for_replay(session.owner_id(), [], [], [], [], []);
    let report =
        execute_memory_rewrite(&plan, &source.0, &destination.0, None, &source_graph).unwrap();
    assert_eq!(report.source_owner_id, report.destination_owner_id);
    let lineage = reliquary_owner_session::fresh_history_lineage(&destination.0)
        .unwrap()
        .unwrap();
    assert_eq!(lineage.destination_owner_id, session.owner_id());
    assert_ne!(lineage.predecessor_generation, lineage.history_generation);
    let rewritten = PhySession::open_path(&destination.0, &source_graph, []).unwrap();
    let rewritten_cut = rewritten.current_historical_cut();
    let rewritten_proof = rewritten
        .historical_proof(session.owner_id(), rewritten_cut)
        .unwrap();
    assert_eq!(rewritten_cut, source_cut);
    assert_eq!(rewritten_proof.owner_id, source_proof.owner_id);
    assert_ne!(rewritten_proof.digest, source_proof.digest);
    assert_ne!(
        rewritten_proof.history_generation,
        source_proof.history_generation
    );
    let merge_error =
        reliquary_owner_session::VerifiedOwnerCopyPlan::prepare_prefix_reconciliation(
            &source.0,
            &destination.0,
        )
        .unwrap_err();
    assert!(merge_error.contains("history generations differ"));
    let error =
        execute_memory_rewrite(&plan, &source.0, &destination.0, None, &source_graph).unwrap_err();
    assert!(error.to_string().contains("destination already exists"));
}

#[test]
fn stale_plan_failure_never_creates_destination() {
    let (source, session, _, _) = phy_fixture();
    let destination = Fixture::new();
    let plan = plan_phy_rewrite(&source.0, &session, phy_policy()).unwrap();
    let mut bytes = std::fs::read(&source.0).unwrap();
    bytes.push(0);
    std::fs::write(&source.0, bytes).unwrap();
    let graph = GraphAdmission::for_replay(session.owner_id(), [], [], [], [], []);
    assert_eq!(
        execute_memory_rewrite(&plan, &source.0, &destination.0, Some([43; 16]), &graph),
        Err(RewriteError::SourceChanged)
    );
    assert!(!destination.0.exists());
}

#[test]
fn unsupported_operational_evidence_stops_before_output() {
    let (source, session, memory, _) = rel_fixture();
    let owner = session.owner_id();
    drop(session);
    let mut container = Container::open_writer(&source.0).unwrap();
    container.append(b"RELXUNS1unsupported fixture").unwrap();
    container.sync().unwrap();
    drop(container);
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let session = RelSession::open_path(&source.0, &graph, [], []).unwrap();
    let mut policy = RewritePolicy::default();
    policy.memories.include.insert(memory.0);
    policy.derived_state = DerivedStateRewriteMode::Omit;
    let plan = plan_rel_rewrite(&source.0, &session, policy).unwrap();
    let destination = Fixture::new();
    let error = execute_memory_rewrite(&plan, &source.0, &destination.0, Some([53; 16]), &graph)
        .unwrap_err();
    assert!(matches!(error, RewriteError::Unsupported(_)));
    assert!(!destination.0.exists());
}

#[test]
fn explicit_selection_expands_referential_closure_without_writing() {
    let (path, session, parent, child) = phy_fixture();
    let before = std::fs::read(&path.0).unwrap();
    let mut policy = phy_policy();
    policy.memories.include.insert(child.0);
    let plan = plan_phy_rewrite(&path.0, &session, policy).unwrap();
    let expected: BTreeSet<_> = [parent.0, child.0].into_iter().collect();
    assert_eq!(
        plan.selection
            .retained_memory_ids
            .into_iter()
            .collect::<BTreeSet<_>>(),
        expected
    );
    assert_eq!(plan.selection.retained_revisions, 2);
    assert_eq!(plan.selection.closure_expansions, vec![parent.0]);
    assert!(plan.selection.dropped_memory_ids.is_empty());
    assert_eq!(std::fs::read(&path.0).unwrap(), before);
    assert_ne!(plan.plan_digest, [0; 32]);
}

#[test]
fn relationship_closure_expands_memory_and_replays_history_tombstone_and_owner_refs() {
    let (source, session, parent, child) = phy_fixture();
    let source_owner = session.owner_id();
    drop(session);

    let mut container = Container::open_writer(&source.0).unwrap();
    let mut perception = PerceptionStore::open(&container).unwrap();
    let entity = perception
        .publish_entity(
            &mut container,
            EntityDraft {
                canonical_name: "Closure entity".into(),
                aliases: vec![],
                kind: "person".into(),
                summary: "retained".into(),
                mutation_id: "closure-entity".into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
        )
        .unwrap();
    perception
        .update_entity(
            &mut container,
            entity.id,
            entity.revision,
            EntityDraft {
                canonical_name: "Closure entity".into(),
                aliases: vec!["Closure alias".into()],
                kind: "person".into(),
                summary: "retained revision".into(),
                mutation_id: "closure-entity-update".into(),
                created_at_ns: 1,
                updated_at_ns: 2,
            },
        )
        .unwrap();
    let local_owner = "29292929-2929-2929-2929-292929292929".to_string();
    let foreign_owner = "ffffffff-ffff-ffff-ffff-ffffffffffff".to_string();
    let mut evidence = vec![
        RelationshipMemoryRef {
            owner_id: local_owner.clone(),
            memory_id: parent.0,
        },
        RelationshipMemoryRef {
            owner_id: local_owner.clone(),
            memory_id: child.0,
        },
        RelationshipMemoryRef {
            owner_id: foreign_owner.clone(),
            memory_id: [0xee; 32],
        },
    ];
    evidence.sort_by(|left, right| {
        (&left.owner_id, left.memory_id).cmp(&(&right.owner_id, right.memory_id))
    });
    let first = perception
        .publish_relationship(
            &mut container,
            RelationshipDraft {
                kind: "association".into(),
                participants: vec![RelationshipParticipant {
                    entity: RelationshipEntityRef {
                        owner_id: local_owner.clone(),
                        entity_id: entity.id,
                    },
                    role: None,
                }],
                evidence,
                summary: "first".into(),
                mutation_id: "closure-relationship".into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
        )
        .unwrap();
    let mut second = first.clone();
    second.summary = "second".into();
    second.mutation_id = "closure-relationship-update".into();
    second.updated_at_ns = 2;
    perception
        .update_relationship(
            &mut container,
            first.id,
            first.revision,
            RelationshipDraft {
                kind: second.kind,
                participants: second.participants,
                evidence: second.evidence,
                summary: second.summary,
                mutation_id: second.mutation_id,
                created_at_ns: second.created_at_ns,
                updated_at_ns: second.updated_at_ns,
            },
        )
        .unwrap();
    perception
        .tombstone_relationship(&mut container, first.id, 2)
        .unwrap();
    container.sync().unwrap();
    drop(container);

    let graph = GraphAdmission::for_replay(source_owner, [], [], [], [], []);
    let session = PhySession::open_path(&source.0, &graph, []).unwrap();
    let mut policy = phy_policy();
    policy.memories.include.insert(parent.0);
    policy.derived_state = DerivedStateRewriteMode::RetainVerified;
    let plan = plan_phy_rewrite(&source.0, &session, policy).unwrap();
    assert_eq!(plan.selection.closure_expansions, vec![child.0]);

    let destination = Fixture::new();
    execute_memory_rewrite(&plan, &source.0, &destination.0, Some([66; 16]), &graph).unwrap();
    let output = Container::open_read_only(&destination.0).unwrap();
    let perception = PerceptionStore::open(&output).unwrap();
    assert_eq!(perception.entity(entity.id).unwrap().revision, 2);
    assert_eq!(
        perception.entity_revision(entity.id, 1).unwrap().summary,
        "retained"
    );
    assert!(perception.relationship(first.id).is_none());
    assert_eq!(
        perception
            .relationship_tombstone(first.id)
            .unwrap()
            .revision,
        2
    );
    let replayed = perception.relationship_revision(first.id, 2).unwrap();
    assert_eq!(replayed.summary, "second");
    assert!(
        replayed
            .evidence
            .iter()
            .any(|reference| reference.owner_id == "42424242-4242-4242-4242-424242424242")
    );
    assert!(
        replayed
            .evidence
            .iter()
            .any(|reference| reference.owner_id == foreign_owner)
    );
}

#[test]
fn contradictory_unknown_and_phy_rel_policy_fail_closed() {
    let (path, session, parent, _) = phy_fixture();
    let mut policy = phy_policy();
    policy.memories.include.insert(parent.0);
    policy.memories.exclude.insert(parent.0);
    assert!(matches!(
        plan_phy_rewrite(&path.0, &session, policy),
        Err(RewriteError::InvalidPolicy(_))
    ));
    let mut policy = phy_policy();
    policy.memories.include.insert([255; 32]);
    assert!(matches!(
        plan_phy_rewrite(&path.0, &session, policy),
        Err(RewriteError::UnknownMemory(_))
    ));
    let mut policy = phy_policy();
    policy.archive = ArchiveRewriteMode::RetainAll;
    assert!(matches!(
        plan_phy_rewrite(&path.0, &session, policy),
        Err(RewriteError::Unsupported(_))
    ));
}

#[test]
fn frozen_source_proof_detects_change_after_plan() {
    let (path, session, _, _) = phy_fixture();
    let plan = plan_phy_rewrite(&path.0, &session, phy_policy()).unwrap();
    verify_rewrite_source(&plan, &path.0).unwrap();
    let mut bytes = std::fs::read(&path.0).unwrap();
    bytes.push(0);
    std::fs::write(&path.0, bytes).unwrap();
    assert_eq!(
        verify_rewrite_source(&plan, &path.0),
        Err(RewriteError::SourceChanged)
    );
}
