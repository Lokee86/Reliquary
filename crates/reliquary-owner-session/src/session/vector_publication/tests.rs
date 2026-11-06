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
