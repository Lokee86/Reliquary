use super::*;
use reliquary_container::ReliquaryScope;
use reliquary_vector_store::generation::{
    GenerationPayload, encode_generation, vector_generation_id,
};
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
            "reclamation-gen-{label}-{}-{}.rel",
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

#[test]
fn valid_unreceipted_generation_is_reclaimed_without_source_mutation() {
    let source = Temp::new("orphan-source");
    let output = Temp::new("orphan-output");
    let owner = [0x84; 16];
    drop(RelWriter::create_path(&source.0, ReliquaryScope::Project, owner).unwrap());
    let profile = CompatibilityProfileId([0x91; 32]);
    let archive = reliquary_vector_store::ArchiveVectorId([0x92; 32]);
    let payload = GenerationPayload {
        id: vector_generation_id(profile, archive, 1),
        compatibility_profile_id: profile,
        archive_vector_id: archive,
        source_archive_version: 1,
    };
    let mut writer = Container::open_writer_as(&source.0, FileKind::Reliquary).unwrap();
    writer.append(&encode_generation(payload)).unwrap();
    writer.sync().unwrap();
    drop(writer);
    let before = fs::read(&source.0).unwrap();
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    assert!(RelSession::open_path(&source.0, &admission, [], []).is_err());
    let plan = VerifiedOwnerCopyPlan::prepare(&source.0).unwrap();
    let report = reclaim_storage_copy(&plan, &source.0, &output.0, &admission).unwrap();
    assert_eq!(report.removed_frames, 1);
    assert!(report.candidate_bytes < report.source_bytes);
    assert_eq!(fs::read(&source.0).unwrap(), before);
    let reopened = RelSession::open_path(&output.0, &admission, [], []).unwrap();
    assert!(reopened.generations.generations().is_empty());
    let next = Temp::new("orphan-noop");
    let plan = VerifiedOwnerCopyPlan::prepare(&output.0).unwrap();
    let noop = reclaim_storage_copy(&plan, &output.0, &next.0, &admission).unwrap();
    assert_eq!(noop.removed_frames, 0);
    assert_eq!(fs::read(&output.0).unwrap(), fs::read(&next.0).unwrap());
}
