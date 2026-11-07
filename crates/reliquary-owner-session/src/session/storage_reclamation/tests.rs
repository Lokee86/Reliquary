use super::*;
use reliquary_archive::{Archive, AttachmentInput, Branch, TurnInput};
use reliquary_container::ReliquaryScope;
use reliquary_vector_store::{
    AcceptedArchiveMapping, ArchiveFragmentId, CompatibilityProbeReference, CompatibilityProfile,
    EmbeddingMode, PackedVectorStore, VectorNormalization,
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

fn archive_profile() -> CompatibilityProfile {
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

struct Temp(PathBuf);

impl Temp {
    fn new(label: &str) -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".owner-session-test-tmp");
        fs::create_dir_all(&root).unwrap();
        Self(root.join(format!(
            "reclamation-{label}-{}-{}.rel",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn source_with_orphan_file() -> (Temp, VerifiedOwnerCopyPlan, GraphAdmission, Vec<u8>) {
    let source = Temp::new("source");
    let owner = [0x72; 16];
    drop(
        RelWriter::create_path(&source.0, ReliquaryScope::Project, owner)
            .expect("create REL fixture"),
    );
    let orphan = vec![0x5a; 512];
    let mut frame = b"RELFILE1".to_vec();
    frame.extend_from_slice(&Sha256::digest(&orphan));
    frame.extend_from_slice(&(orphan.len() as u32).to_le_bytes());
    frame.extend_from_slice(&orphan);
    let mut writer = Container::open_writer(&source.0).unwrap();
    writer.append(&frame).unwrap();
    writer.sync().unwrap();
    drop(writer);
    let before = fs::read(&source.0).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    (source, plan, admission, before)
}

#[test]
fn injected_copy_sync_and_reopen_failures_leave_source_exact_and_no_destination() {
    let (source, plan, admission, before) = source_with_orphan_file();
    let cases = [
        (
            "copy",
            ReclamationFaults {
                copy: true,
                ..ReclamationFaults::default()
            },
        ),
        (
            "sync",
            ReclamationFaults {
                sync: true,
                ..ReclamationFaults::default()
            },
        ),
        (
            "reopen",
            ReclamationFaults {
                reopen: true,
                ..ReclamationFaults::default()
            },
        ),
    ];

    for (label, faults) in cases {
        let destination = Temp::new(label);
        let error =
            reclaim_storage_copy_inner(&plan, &source.0, &destination.0, &admission, faults)
                .unwrap_err();
        assert!(error.contains(&format!("injected reclamation {label} failure")));
        assert!(!destination.0.exists());
        assert_eq!(fs::read(&source.0).unwrap(), before);
        RelSession::open_path(&source.0, &admission, [], []).unwrap();
    }
}

#[test]
fn orphan_archive_file_is_reclaimed() {
    let (source, plan, admission, before) = source_with_orphan_file();
    let destination = Temp::new("reclaimed");
    let report = reclaim_storage_copy(&plan, &source.0, &destination.0, &admission).unwrap();
    assert_eq!(report.removed_frames, 1);
    assert!(report.candidate_bytes < report.source_bytes);
    assert_eq!(fs::read(&source.0).unwrap(), before);
    RelSession::open_path(&destination.0, &admission, [], []).unwrap();
}

#[test]
fn referenced_archive_file_is_retained_in_an_exact_noop_copy() {
    let source = Temp::new("referenced-file");
    let owner = [0x73; 16];
    drop(RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap());
    let attachment = vec![0x6b; 256];
    let mut writer = Container::open_writer(&source.0).unwrap();
    let mut archive = Archive::open(&writer).unwrap();
    archive
        .accept_turn(
            &mut writer,
            TurnInput {
                id: "turn".into(),
                conversation_id: "conversation".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("principal".into()),
                timestamp_ns: 1,
                content: "turn with an attachment".into(),
                attachments: vec![AttachmentInput {
                    filename: "evidence.bin".into(),
                    mime_type: Some("application/octet-stream".into()),
                    bytes: attachment.clone(),
                }],
                project_attachments: vec![],
            },
        )
        .unwrap();
    writer.sync().unwrap();
    drop(writer);
    let before = fs::read(&source.0).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let destination = Temp::new("referenced-copy");
    let report = reclaim_storage_copy(&plan, &source.0, &destination.0, &admission).unwrap();
    assert_eq!(report.removed_frames, 0);
    assert_eq!(fs::read(&destination.0).unwrap(), before);
    let reopened = RelSession::open_path(&destination.0, &admission, [], []).unwrap();
    assert_eq!(
        reopened.archive_source_attachments("conversation", "turn")[0].bytes,
        attachment
    );
}

#[test]
fn reclamation_retains_admitted_insomnia_v2_history_while_removing_orphan_backing() {
    let source = Temp::new("insomnia-v2-source");
    let owner = [0x7a; 16];
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut rel = RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap();
    rel.accept_turn_and_advance_branch(
        TurnInput {
            id: "work-turn".into(),
            conversation_id: "work-conversation".into(),
            parent_id: None,
            role: "user".into(),
            principal_id: Some("worker-principal".into()),
            timestamp_ns: 10,
            content: "accepted work source".into(),
            attachments: vec![],
            project_attachments: vec![],
        },
        Branch {
            id: "main".into(),
            conversation_id: "work-conversation".into(),
            leaf_node_id: "work-turn".into(),
            canonical: true,
        },
    )
    .unwrap();
    let episode = rel
        .finalize_episode(
            "work-conversation",
            "work-turn",
            "work-turn",
            reliquary_archive::EpisodeOrigin::Live,
            reliquary_archive::EpisodeBoundary::Explicit,
            11,
        )
        .unwrap();
    rel.claim_next_insomnia_input("worker", 12, 100, 1)
        .unwrap()
        .unwrap();
    rel.container.sync().unwrap();
    drop(rel);

    let orphan = vec![0x4d; 256];
    let mut frame = b"RELFILE1".to_vec();
    frame.extend_from_slice(&Sha256::digest(&orphan));
    frame.extend_from_slice(&(orphan.len() as u32).to_le_bytes());
    frame.extend_from_slice(&orphan);
    let mut container = Container::open_writer(&source.0).unwrap();
    container.append(&frame).unwrap();
    container.sync().unwrap();
    drop(container);

    let source_container = Container::open_read_only(&source.0).unwrap();
    let history = reliquary_insomnia_store::InsomniaStore::work_history(
        &source_container,
        WorkEpisodeId(episode.0),
    )
    .unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(
        history.iter().map(|work| work.state).collect::<Vec<_>>(),
        [
            reliquary_insomnia_store::InsomniaWorkState::Pending,
            reliquary_insomnia_store::InsomniaWorkState::Processing,
        ]
    );
    drop(source_container);

    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let output = Temp::new("insomnia-v2-reclaimed");
    let report = reclaim_storage_copy(&plan, &source.0, &output.0, &admission).unwrap();
    assert_eq!(report.removed_frames, 1);
    let output_container = Container::open_read_only(&output.0).unwrap();
    assert_eq!(
        reliquary_insomnia_store::InsomniaStore::work_history(
            &output_container,
            WorkEpisodeId(episode.0),
        )
        .unwrap(),
        history
    );
    drop(output_container);

    let repeat = Temp::new("insomnia-v2-reclaimed-repeat");
    let repeat_plan = VerifiedOwnerCopyPlan::prepare(&output.0).unwrap();
    let repeat_report =
        reclaim_storage_copy(&repeat_plan, &output.0, &repeat.0, &admission).unwrap();
    assert_eq!(repeat_report.removed_frames, 0);
    assert_eq!(fs::read(&repeat.0).unwrap(), fs::read(&output.0).unwrap());
}

#[test]
fn malformed_insomnia_v2_is_rejected_before_reclamation_output() {
    let source = Temp::new("malformed-insomnia-v2");
    let owner = [0x7b; 16];
    drop(RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap());
    let mut container = Container::open_writer(&source.0).unwrap();
    container.append(b"RELINSW2").unwrap();
    container.sync().unwrap();
    drop(container);

    let before = fs::read(&source.0).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let output = Temp::new("malformed-insomnia-v2-output");
    assert!(reclaim_storage_copy(&plan, &source.0, &output.0, &admission).is_err());
    assert!(!output.0.exists());
    assert_eq!(fs::read(&source.0).unwrap(), before);
}

#[test]
fn phy_reclamation_removes_unmapped_packed_backing_and_reopens() {
    let source = Temp::new("phy-source");
    let owner = [0x74; 16];
    drop(PhyWriter::create_path(&source.0, owner).unwrap());
    let mut writer = Container::open_writer(&source.0).unwrap();
    let mut packed = PackedVectorStore::open(&writer).unwrap();
    packed
        .put_f32_rows(&mut writer, &[vec![0.25; 128]])
        .unwrap();
    writer.sync().unwrap();
    drop(writer);
    let before = fs::read(&source.0).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let destination = Temp::new("phy-reclaimed");
    let report = reclaim_storage_copy(&plan, &source.0, &destination.0, &admission).unwrap();
    assert_eq!(report.removed_frames, 1);
    assert!(report.candidate_bytes < report.source_bytes);
    assert_eq!(fs::read(&source.0).unwrap(), before);
    PhySession::open_path(&destination.0, &admission, []).unwrap();
}

#[test]
fn unactivated_archive_mapping_and_exclusive_backing_are_reclaimed() {
    let source = Temp::new("orphan-archive-vector");
    let destination = Temp::new("orphan-archive-vector-reclaimed");
    let owner = [0x75; 16];
    drop(RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap());
    let mut container = Container::open_writer_as(&source.0, FileKind::Reliquary).unwrap();
    let mut packed = PackedVectorStore::open(&container).unwrap();
    let packed_id = packed
        .put_f32_rows(&mut container, &[vec![0.5; 128]])
        .unwrap();
    let mut mappings = ArchiveVectorStore::open(&container, &packed).unwrap();
    mappings
        .put(
            &mut container,
            &packed,
            AcceptedArchiveMapping {
                packed_vector_id: packed_id,
                packed_rows: 1,
                fragment_ids: vec![ArchiveFragmentId([0xf2; 32])],
                max_fragment_archive_version: 0,
            },
        )
        .unwrap();
    container.sync().unwrap();
    drop(container);

    let before = fs::read(&source.0).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let report = reclaim_storage_copy(&plan, &source.0, &destination.0, &admission).unwrap();
    assert_eq!(report.removed_frames, 2);
    assert!(report.candidate_bytes < report.source_bytes);
    assert_eq!(fs::read(&source.0).unwrap(), before);
    let reopened = RelSession::open_path(&destination.0, &admission, [], []).unwrap();
    assert!(reopened.archive_vector_infos().is_empty());
    assert!(reopened.common.packed_vectors.infos().is_empty());
}

#[test]
fn activated_mapping_and_shared_backing_are_retained() {
    let source = Temp::new("active-and-orphan-archive-vector");
    let destination = Temp::new("active-and-orphan-archive-vector-reclaimed");
    let owner = [0x76; 16];
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap();
    writer
        .accept_turn_and_advance_branch(
            TurnInput {
                id: "turn".into(),
                conversation_id: "conversation".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("principal".into()),
                timestamp_ns: 1,
                content: "retained Archive-vector source".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
            Branch {
                id: "main".into(),
                conversation_id: "conversation".into(),
                leaf_node_id: "turn".into(),
                canonical: true,
            },
        )
        .unwrap();
    let profile = writer
        .publish_compatibility_profile(archive_profile())
        .unwrap();
    let input = writer.archive_vector_input(profile).unwrap();
    let rows = input
        .sources()
        .iter()
        .map(|source| ArchiveVectorRow {
            fragment_id: source.fragment_id,
            max_archive_version: source.max_archive_version,
            vector: vec![1.0, 0.0],
        })
        .collect::<Vec<_>>();
    let generation = writer
        .publish_archive_vector_batch(owner, profile, input.source_archive_version(), &rows)
        .unwrap()
        .unwrap();
    let active = writer
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
            AcceptedArchiveMapping {
                packed_vector_id: active.packed_vector_id,
                packed_rows: 1,
                fragment_ids: vec![ArchiveFragmentId([0xf3; 32])],
                max_fragment_archive_version: 0,
            },
        )
        .unwrap();
    writer.container.sync().unwrap();
    drop(writer);

    let before = fs::read(&source.0).unwrap();
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let report = reclaim_storage_copy(&plan, &source.0, &destination.0, &admission).unwrap();
    assert_eq!(report.removed_frames, 1);
    assert_eq!(fs::read(&source.0).unwrap(), before);
    let reopened = RelSession::open_path(&destination.0, &admission, [], []).unwrap();
    assert_eq!(reopened.archive_vector_infos().len(), 1);
    assert_eq!(reopened.vector_generation(generation.id), Some(generation));
    let retained = reopened
        .archive_vectors
        .get(generation.archive_vector_id)
        .unwrap();
    assert_eq!(
        reopened
            .common
            .packed_vectors
            .read_f32_row(retained.packed_vector_id, 0)
            .unwrap(),
        vec![1.0, 0.0]
    );
}
