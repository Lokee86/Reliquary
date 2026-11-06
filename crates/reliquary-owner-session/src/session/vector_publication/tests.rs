use super::*;
use reliquary_container::ReliquaryScope;
use reliquary_vector_store::{CompatibilityProbeReference, CompatibilityProfile, EmbeddingMode};
use std::{fs, path::PathBuf};

fn profile() -> CompatibilityProfile {
    let mut profile = CompatibilityProfile {
        id: CompatibilityProfileId([0; 32]),
        dimensions: 2,
        normalization: VectorNormalization::L2,
        probe_suite_version: 1,
        compatibility_policy_version: 3,
        references: [
            EmbeddingMode::Query,
            EmbeddingMode::Query,
            EmbeddingMode::Document,
            EmbeddingMode::Document,
        ]
        .into_iter()
        .map(|mode| CompatibilityProbeReference {
            mode,
            vector: vec![1.0, 0.0],
        })
        .collect(),
    };
    profile.id = reliquary_vector_store::profile::compatibility_profile_id(&profile);
    profile
}
fn draft() -> MemoryDraft {
    MemoryDraft {
        title: "title".into(),
        content: "exact body".into(),
        category: "fact".into(),
        memory_type: "semantic".into(),
        authority_kind: "user".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
        lifecycle_state: "extracted".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: None,
        source_time_ns: None,
        temporal_inference: None,
        mutation_id: "vector-source".into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

#[test]
fn profile_repair_keeps_exact_covered_set_and_fences_stale_plans() {
    use crate::{ProfileRepairError, execute_profile_repair, plan_profile_repair};

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    fs::create_dir_all(&dir).unwrap();
    let stem = format!("profile-repair-{}", std::process::id());
    let source = dir.join(format!("{stem}.phy"));
    let destination = dir.join(format!("{stem}-candidate.phy"));
    let owner = [172; 16];
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = PhyWriter::create_path(&source, owner).unwrap();
    let memory = writer
        .common
        .memories
        .publish_versioned_creation(&mut writer.container, draft(), 1)
        .unwrap()
        .revision
        .record;
    writer.writer_lease_proof = writer.container.writer_lease_proof();
    let kept = writer.publish_compatibility_profile(profile()).unwrap();
    let mut alternative = profile();
    for probe in &mut alternative.references {
        probe.vector = vec![0.0, 1.0];
    }
    alternative.id = reliquary_vector_store::profile::compatibility_profile_id(&alternative);
    let retired = writer.publish_compatibility_profile(alternative).unwrap();
    writer
        .publish_memory_vectors(
            kept,
            &[MemoryVectorRow {
                memory_id: memory.id,
                revision: memory.revision,
                body_id: memory.body_id,
                vector: vec![1.0, 0.0],
            }],
        )
        .unwrap();
    drop(writer);

    assert_eq!(
        plan_profile_repair(&source, &grant, &[kept, kept]),
        Err(ProfileRepairError::InvalidRetainedSet)
    );
    assert_eq!(
        plan_profile_repair(&source, &grant, &[retired]),
        Err(ProfileRepairError::IncompleteMemoryCoverage(retired))
    );
    let foreign = GraphAdmission::for_replay([173; 16], [], [], [], [], []);
    assert_eq!(
        plan_profile_repair(&source, &foreign, &[kept]),
        Err(ProfileRepairError::OwnerMismatch)
    );
    let plan = plan_profile_repair(&source, &grant, &[kept]).unwrap();
    assert_eq!(plan.retiring(), &[retired]);
    let before = fs::read(&source).unwrap();
    assert!(!destination.exists());
    let report = execute_profile_repair(&plan, &source, &destination, &grant).unwrap();
    assert_eq!(report.retired, [retired]);
    let after = fs::read(&destination).unwrap();
    assert!(after.starts_with(&before));
    assert_eq!(fs::read(&source).unwrap(), before);
    let reopened = PhySession::open_path(&destination, &grant, []).unwrap();
    assert!(reopened.active_compatibility_profile(retired).is_none());
    assert!(reopened.compatibility_profile(retired).is_some());
    assert_eq!(reopened.current_memory(memory.id).unwrap().record, memory);
    assert_eq!(
        reopened.memory_vector_row(kept, memory.body_id).unwrap(),
        Some(vec![1.0, 0.0])
    );
    assert!(execute_profile_repair(&plan, &source, &destination, &grant).is_err());
    assert_eq!(fs::read(&destination).unwrap(), after);
    drop(reopened);
    fs::remove_file(&destination).unwrap();

    let mut writer = Container::open_writer(&source).unwrap();
    writer.append(b"M02STALE").unwrap();
    writer.sync().unwrap();
    drop(writer);
    assert!(execute_profile_repair(&plan, &source, &destination, &grant).is_err());
    assert!(!destination.exists());
    fs::remove_file(&source).unwrap();
}

#[test]
fn retired_profile_prune_replaces_only_exclusive_backing_and_reopens() {
    use crate::{
        VerifiedOwnerCopyPlan, execute_profile_repair, plan_profile_repair,
        prune_retired_profile_vectors_copy,
    };

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    fs::create_dir_all(&dir).unwrap();
    let stem = format!("profile-prune-{}", std::process::id());
    let source = dir.join(format!("{stem}.phy"));
    let repaired = dir.join(format!("{stem}-repaired.phy"));
    let output = dir.join(format!("{stem}-pruned.phy"));
    let owner = [174; 16];
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = PhyWriter::create_path(&source, owner).unwrap();
    let memory = writer
        .common
        .memories
        .publish_versioned_creation(&mut writer.container, draft(), 1)
        .unwrap()
        .revision
        .record;
    writer.writer_lease_proof = writer.container.writer_lease_proof();
    let kept = writer.publish_compatibility_profile(profile()).unwrap();
    let mut old = profile();
    old.dimensions = 512;
    for probe in &mut old.references {
        probe.vector = vec![0.0; 512];
        probe.vector[0] = 1.0;
    }
    old.id = reliquary_vector_store::profile::compatibility_profile_id(&old);
    let retired = writer.publish_compatibility_profile(old).unwrap();
    for (profile, vector) in [
        (kept, vec![1.0, 0.0]),
        (retired, {
            let mut vector = vec![0.0; 512];
            vector[0] = 1.0;
            vector
        }),
    ] {
        writer
            .publish_memory_vectors(
                profile,
                &[MemoryVectorRow {
                    memory_id: memory.id,
                    revision: memory.revision,
                    body_id: memory.body_id,
                    vector,
                }],
            )
            .unwrap();
    }
    drop(writer);
    let repair = plan_profile_repair(&source, &grant, &[kept]).unwrap();
    execute_profile_repair(&repair, &source, &repaired, &grant).unwrap();
    let before = fs::read(&repaired).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&repaired).unwrap();
    let report =
        prune_retired_profile_vectors_copy(&plan, &repaired, &output, &[retired], &grant).unwrap();
    assert_eq!(report.retired_profiles, [retired]);
    assert_eq!(report.pruned_packed_vectors, 1);
    assert!(report.transformed_bytes < report.source_bytes);
    assert_eq!(fs::read(&repaired).unwrap(), before);
    let reopened = PhySession::open_path(&output, &grant, []).unwrap();
    assert_eq!(
        reopened.memory_vector_row(kept, memory.body_id).unwrap(),
        Some(vec![1.0, 0.0])
    );
    assert!(reopened.memory_vector_row(retired, memory.body_id).is_err());
    assert!(
        prune_retired_profile_vectors_copy(&plan, &repaired, &output, &[retired], &grant,).is_err()
    );
    fs::remove_file(&output).unwrap();
    fs::remove_file(&repaired).unwrap();
    fs::remove_file(&source).unwrap();
}

#[test]
fn retired_profile_prune_refuses_shared_active_backing() {
    use crate::{VerifiedOwnerCopyPlan, prune_retired_profile_vectors_copy};

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    fs::create_dir_all(&dir).unwrap();
    let stem = format!("profile-shared-{}", std::process::id());
    let source = dir.join(format!("{stem}.phy"));
    let output = dir.join(format!("{stem}-pruned.phy"));
    let owner = [175; 16];
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = PhyWriter::create_path(&source, owner).unwrap();
    let memory = writer
        .common
        .memories
        .publish_versioned_creation(&mut writer.container, draft(), 1)
        .unwrap()
        .revision
        .record;
    writer.writer_lease_proof = writer.container.writer_lease_proof();
    let kept = writer.publish_compatibility_profile(profile()).unwrap();
    let mut other = profile();
    for probe in &mut other.references {
        probe.vector = vec![0.0, 1.0];
    }
    other.id = reliquary_vector_store::profile::compatibility_profile_id(&other);
    let retired = writer.publish_compatibility_profile(other).unwrap();
    for profile in [kept, retired] {
        writer
            .publish_memory_vectors(
                profile,
                &[MemoryVectorRow {
                    memory_id: memory.id,
                    revision: memory.revision,
                    body_id: memory.body_id,
                    vector: vec![1.0, 0.0],
                }],
            )
            .unwrap();
    }
    writer
        .common
        .profiles
        .retire(&mut writer.container, retired)
        .unwrap();
    writer.container.sync().unwrap();
    drop(writer);

    let before = fs::read(&source).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&source).unwrap();
    let error = prune_retired_profile_vectors_copy(&plan, &source, &output, &[retired], &grant)
        .unwrap_err();
    assert!(error.contains("shared by another profile"), "{error}");
    assert!(!output.exists());
    assert_eq!(fs::read(&source).unwrap(), before);
    fs::remove_file(&source).unwrap();
}

#[test]
fn retired_profile_prune_preserves_rel_generation_receipts() {
    use crate::{
        VerifiedOwnerCopyPlan, execute_profile_repair, plan_profile_repair,
        prune_retired_profile_vectors_copy,
    };

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    fs::create_dir_all(&dir).unwrap();
    let stem = format!("rel-profile-prune-{}", std::process::id());
    let source = dir.join(format!("{stem}.rel"));
    let repaired = dir.join(format!("{stem}-repaired.rel"));
    let output = dir.join(format!("{stem}-pruned.rel"));
    let owner = [176; 16];
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::create_path(&source, ReliquaryScope::Project, owner).unwrap();
    writer
        .accept_turn_and_advance_branch(
            reliquary_archive::TurnInput {
                id: "turn-1".into(),
                conversation_id: "conversation-1".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: None,
                timestamp_ns: 1,
                content: "Archive text retained across profile pruning".into(),
                attachments: Vec::new(),
                project_attachments: Vec::new(),
            },
            reliquary_archive::Branch {
                id: "main".into(),
                conversation_id: "conversation-1".into(),
                leaf_node_id: "turn-1".into(),
                canonical: true,
            },
        )
        .unwrap();
    let kept = writer.publish_compatibility_profile(profile()).unwrap();
    let mut old = profile();
    old.dimensions = 512;
    for probe in &mut old.references {
        probe.vector = vec![0.0; 512];
        probe.vector[0] = 1.0;
    }
    old.id = reliquary_vector_store::profile::compatibility_profile_id(&old);
    let retired = writer.publish_compatibility_profile(old).unwrap();
    for (profile, dimensions) in [(kept, 2), (retired, 512)] {
        let input = writer.archive_vector_input(profile).unwrap();
        let mut vector = vec![0.0; dimensions];
        vector[0] = 1.0;
        let rows = input
            .sources()
            .iter()
            .map(|source| ArchiveVectorRow {
                fragment_id: source.fragment_id,
                max_archive_version: source.max_archive_version,
                vector: vector.clone(),
            })
            .collect::<Vec<_>>();
        assert!(!rows.is_empty());
        writer
            .publish_archive_vector_batch(owner, profile, input.source_archive_version(), &rows)
            .unwrap()
            .unwrap();
    }
    drop(writer);

    let repair = plan_profile_repair(&source, &grant, &[kept]).unwrap();
    execute_profile_repair(&repair, &source, &repaired, &grant).unwrap();
    let before = fs::read(&repaired).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&repaired).unwrap();
    let report =
        prune_retired_profile_vectors_copy(&plan, &repaired, &output, &[retired], &grant).unwrap();
    assert_eq!(report.pruned_packed_vectors, 1);
    assert!(report.transformed_bytes < report.source_bytes);
    assert_eq!(fs::read(&repaired).unwrap(), before);

    let original = RelSession::open_path(&repaired, &grant, [], []).unwrap();
    let reopened = RelSession::open_path(&output, &grant, [], []).unwrap();
    let final_cut = original.current_historical_cut();
    let final_proof = original.historical_proof(owner, &final_cut).unwrap();
    assert_eq!(
        reopened.historical_proof(owner, &final_cut).unwrap(),
        final_proof
    );
    let mut first_generation_cut = final_cut.clone();
    first_generation_cut.owner.vector_version = 1;
    let first_generation = original
        .historical_proof(owner, &first_generation_cut)
        .unwrap();
    assert_ne!(first_generation.digest, final_proof.digest);
    assert_eq!(
        reopened
            .historical_proof(owner, &first_generation_cut)
            .unwrap(),
        first_generation
    );
    assert_eq!(
        reopened.generations.generations(),
        original.generations.generations()
    );
    assert_eq!(
        reopened.archive.archive_version(),
        original.archive.archive_version()
    );
    assert_eq!(reopened.archive.node_count(), original.archive.node_count());
    let active_generation = reopened.generations.current_generation(kept).unwrap();
    let active_mapping = reopened
        .archive_vectors
        .get(active_generation.archive_vector_id)
        .unwrap();
    assert_eq!(
        reopened
            .common
            .packed_vectors
            .read_f32_row(active_mapping.packed_vector_id, 0)
            .unwrap(),
        vec![1.0, 0.0]
    );
    let retired_generation = reopened.generations.current_generation(retired).unwrap();
    let retired_mapping = reopened
        .archive_vectors
        .get(retired_generation.archive_vector_id)
        .unwrap();
    assert!(
        reopened
            .common
            .packed_vectors
            .is_pruned(retired_mapping.packed_vector_id)
    );
    assert!(
        reopened
            .common
            .packed_vectors
            .read_f32_row(retired_mapping.packed_vector_id, 0)
            .is_err()
    );

    drop(reopened);
    drop(original);

    // A mapping published without an owning generation cannot be attributed
    // to the retired profile, even if its matrix is also used by that profile.
    let orphan_repaired = dir.join(format!("{stem}-orphan-repaired.rel"));
    let orphan_output = dir.join(format!("{stem}-orphan-pruned.rel"));
    let mut writer = RelWriter::open_path(&source, grant.clone(), [], []).unwrap();
    let generation = writer
        .session
        .generations
        .current_generation(retired)
        .unwrap();
    let mapping = writer
        .session
        .archive_vectors
        .get(generation.archive_vector_id)
        .unwrap();
    writer
        .session
        .archive_vectors
        .put(
            &mut writer.container,
            &writer.session.common.packed_vectors,
            reliquary_vector_store::AcceptedArchiveMapping {
                packed_vector_id: mapping.packed_vector_id,
                packed_rows: 1,
                fragment_ids: vec![reliquary_vector_store::ArchiveFragmentId([97; 32])],
                max_fragment_archive_version: 0,
            },
        )
        .unwrap();
    writer.container.sync().unwrap();
    drop(writer);
    let repair = plan_profile_repair(&source, &grant, &[kept]).unwrap();
    execute_profile_repair(&repair, &source, &orphan_repaired, &grant).unwrap();
    let before_orphan = fs::read(&orphan_repaired).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&orphan_repaired).unwrap();
    let err = prune_retired_profile_vectors_copy(
        &plan,
        &orphan_repaired,
        &orphan_output,
        &[retired],
        &grant,
    )
    .unwrap_err();
    assert!(err.contains("unowned Archive mapping"), "{err}");
    assert!(!orphan_output.exists());
    assert_eq!(fs::read(&orphan_repaired).unwrap(), before_orphan);

    fs::remove_file(&orphan_repaired).unwrap();
    fs::remove_file(&output).unwrap();
    fs::remove_file(&repaired).unwrap();
    fs::remove_file(&source).unwrap();
}

#[test]
fn vector_publication_recovers_orphan_packed_bytes_and_rejects_invalid_sources() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    fs::create_dir_all(&root).unwrap();
    let path = root.join(format!(
        "vectors-{}-{}.rel",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let owner = [171; 16];
    let mut writer = RelWriter::create_path(&path, ReliquaryScope::Project, owner).unwrap();
    let published = writer
        .session
        .common
        .memories
        .publish_versioned_creation(&mut writer.container, draft(), 1)
        .unwrap();
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    let profile = writer.publish_compatibility_profile(profile()).unwrap();
    let row = MemoryVectorRow {
        memory_id: published.revision.record.id,
        revision: 1,
        body_id: published.revision.record.body_id,
        vector: vec![1.0, 0.0],
    };
    let before = writer.container.latest_version();
    for bad in [
        MemoryVectorRow {
            revision: 2,
            ..row.clone()
        },
        MemoryVectorRow {
            body_id: reliquary_memory_core::MemoryBodyId([99; 32]),
            ..row.clone()
        },
        MemoryVectorRow {
            vector: vec![f32::NAN, 0.0],
            ..row.clone()
        },
        MemoryVectorRow {
            vector: vec![2.0, 0.0],
            ..row.clone()
        },
    ] {
        assert!(writer.publish_memory_vectors(profile, &[bad]).is_err());
        assert_eq!(writer.container.latest_version(), before);
    }
    writer
        .session
        .common
        .packed_vectors
        .put_f32_rows(&mut writer.container, &[row.vector.clone()])
        .unwrap();
    writer.container.sync().unwrap();
    drop(writer);
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::open_path(&path, admission.clone(), [], []).unwrap();
    assert!(
        writer
            .publish_memory_vectors(profile, &[row.clone()])
            .unwrap()
    );
    assert_eq!(writer.session.common.packed_vectors.infos().len(), 1);
    assert!(
        !writer
            .publish_memory_vectors(profile, &[row.clone()])
            .unwrap()
    );
    let before = writer.container.latest_version();
    assert!(
        writer
            .publish_memory_vectors(
                profile,
                &[MemoryVectorRow {
                    vector: vec![0.0, 1.0],
                    ..row.clone()
                }]
            )
            .is_err()
    );
    assert_eq!(writer.container.latest_version(), before);
    drop(writer);
    let mut writer = RelWriter::open_path(&path, admission.clone(), [], []).unwrap();
    let input = writer
        .read_session()
        .memory_vector_input(profile, &[row.memory_id])
        .unwrap();
    assert_eq!(input.existing(), &[Some(vec![1.0, 0.0])]);
    writer
        .session
        .common
        .profiles
        .retire(&mut writer.container, profile)
        .unwrap();
    writer.container.sync().unwrap();
    writer.session.writer_lease_proof = writer.container.writer_lease_proof();
    assert!(writer.publish_memory_vectors(profile, &[row]).is_err());
    assert!(
        writer
            .publish_compatibility_profile(self::profile())
            .is_err()
    );
    drop(writer);
    fs::remove_file(path).unwrap();
}
