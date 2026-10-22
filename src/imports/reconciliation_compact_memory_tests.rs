use super::*;
use crate::{Cva, MemoryDraft, MemoryId};
use std::fs;
use std::path::{Path, PathBuf};

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("reconcile-compact-memory-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn downgrade_header_to_v1(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

fn memory_versions(cva: &mut Cva) -> Vec<(u64, u64, Vec<u8>)> {
    let mut versions = Vec::new();
    for chunk in cva.container.chunks().unwrap() {
        let payload = cva.container.read(chunk).unwrap();
        if let Some((global, local)) =
            crate::memory_compact_codec::publication::coordinates(&payload).unwrap()
        {
            versions.push((global, local, payload));
        }
    }
    versions.sort_by_key(|(_, local, _)| *local);
    versions
}

fn canonical_memory_evidence(cva: &mut Cva) -> Vec<Vec<u8>> {
    memory_versions(cva)
        .into_iter()
        .map(|(global, local, payload)| {
            super::canonical_memory_publication(cva, global, local, &payload).unwrap()
        })
        .collect()
}

fn updated_draft(mut draft: MemoryDraft, mutation_id: &str, updated_at_ns: i64) -> MemoryDraft {
    draft.mutation_id = mutation_id.into();
    draft.updated_at_ns = updated_at_ns;
    draft
}

#[test]
fn replay_interval_accepts_compact_memory_created_after_its_boundary() {
    let scratch = ScratchDirectory::new();
    let path = scratch.file("compact-interval.rel");
    let mut cva = Cva::create(&path).unwrap();
    let boundary = cva.container.chunks().unwrap().len();
    let draft = crate::entity_candidate_test_support::memory_draft(
        "compact-interval-memory",
        "Interval title",
        "Interval body",
    );
    let (memory, changed) = cva.publish_memory(None, 0, draft.clone()).unwrap();
    assert!(changed);
    let mut revision = updated_draft(draft, "compact-interval-revision", 20);
    revision.temporal_status = "historical".into();
    let (_, changed) = cva
        .publish_memory(Some(memory.id), memory.revision, revision)
        .unwrap();
    assert!(changed);

    super::validate_replay_interval(&mut cva, boundary).unwrap();
}

#[test]
fn replay_interval_rejects_malformed_and_unknown_compact_memory_families() {
    for (name, payload) in [
        ("malformed-known.rel", b"CVAMC001".as_slice()),
        ("unknown-version.rel", b"CVAMC002".as_slice()),
    ] {
        let scratch = ScratchDirectory::new();
        let path = scratch.file(name);
        let mut cva = Cva::create(&path).unwrap();
        let boundary = cva.container.chunks().unwrap().len();
        cva.container.append(payload).unwrap();
        assert!(super::validate_replay_interval(&mut cva, boundary).is_err());
    }
}
