use super::*;
use reliquary_archive::{Archive, AttachmentInput, TurnInput};
use reliquary_container::ReliquaryScope;
use reliquary_vector_store::PackedVectorStore;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

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
