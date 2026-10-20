//! Step177 assertions for direct Memory writes in both owner facades.
use crate::container::publication_envelope;
use crate::memory_compact_codec::publication::{self, MemoryPublication};
use crate::owner_lifecycle::TombstoneOutcome;
use crate::{
    Container, ContainerError, Cva, Memory, MemoryDraft, MemoryError, MemoryId, Phylactery,
};
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("memory-envelope-writer-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn draft(mutation_id: &str, scope: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "One immutable body".into(),
        content: "The body is separate from its publication envelopes.".into(),
        scope: scope.into(),
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
        source_time_ns: None,
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns: if scope == "revised" { 20 } else { 10 },
    }
}

trait MemoryOwner {
    fn publish(
        &mut self,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
    ) -> Result<(Memory, bool), MemoryError>;
    fn tombstone(&mut self, id: MemoryId, revision: u64) -> Result<TombstoneOutcome, MemoryError>;
    fn memory_version(&self) -> u64;
}

impl MemoryOwner for Cva {
    fn publish(
        &mut self,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
    ) -> Result<(Memory, bool), MemoryError> {
        self.publish_memory(id, expected_revision, draft)
    }
    fn tombstone(&mut self, id: MemoryId, revision: u64) -> Result<TombstoneOutcome, MemoryError> {
        self.tombstone_memory(id, revision)
    }
    fn memory_version(&self) -> u64 {
        Cva::memory_version(self)
    }
}

impl MemoryOwner for Phylactery {
    fn publish(
        &mut self,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
    ) -> Result<(Memory, bool), MemoryError> {
        self.publish_memory(id, expected_revision, draft)
    }
    fn tombstone(&mut self, id: MemoryId, revision: u64) -> Result<TombstoneOutcome, MemoryError> {
        self.tombstone_memory(id, revision)
    }
    fn memory_version(&self) -> u64 {
        Phylactery::memory_version(self)
    }
}

fn file_bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap()
}

fn payloads(path: &Path) -> Vec<Vec<u8>> {
    let mut container = Container::open(path).unwrap();
    let mut result = Vec::new();
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            result.push(payload.to_vec());
            Ok(())
        })
        .unwrap();
    result
}

fn memory_envelopes(path: &Path) -> Vec<Vec<u8>> {
    payloads(path)
        .into_iter()
        .filter(|payload| {
            publication_envelope::decode(payload)
                .ok()
                .flatten()
                .is_some_and(|envelope| {
                    envelope.body.get(..8).is_some_and(|magic| {
                        magic == b"CVAMC002" || magic == b"CVAMD002" || magic == b"CVAMT002"
                    })
                })
        })
        .collect()
}

fn assert_no_legacy_publication_sidecars(path: &Path) {
    let payloads = payloads(path);
    for magic in [
        b"CVAMEMV1".as_slice(),
        b"CVAVERS",
        b"CVAMEMT1",
        b"CVAMC001",
        b"CVAMD001",
    ] {
        assert!(
            !payloads.iter().any(|payload| payload.starts_with(magic)),
            "unexpected old Memory publication sidecar {}",
            String::from_utf8_lossy(magic)
        );
    }
}

fn exercise_direct_writer(owner: &mut impl MemoryOwner, path: &Path) {
    let (first, created) = owner.publish(None, 0, draft("create", "first")).unwrap();
    assert!(created);
    assert_eq!(memory_envelopes(path).len(), 1);
    assert_eq!(
        payloads(path)
            .iter()
            .filter(|payload| payload.starts_with(b"CVAMBDY1"))
            .count(),
        1,
        "creation writes its immutable body separately"
    );
    assert_no_legacy_publication_sidecars(path);

    let after_creation = file_bytes(path);
    let (retry, created) = owner.publish(None, 0, draft("create", "first")).unwrap();
    assert!(!created);
    assert_eq!(retry, first);
    assert_eq!(
        file_bytes(path),
        after_creation,
        "retry adds no bytes or clock"
    );

    let (second, created) = owner
        .publish(Some(first.id), first.revision, draft("revise", "revised"))
        .unwrap();
    assert!(created);
    assert_eq!(second.revision, 2);
    assert_eq!(owner.memory_version(), 2);
    assert_eq!(memory_envelopes(path).len(), 2);
    assert_eq!(
        payloads(path)
            .iter()
            .filter(|payload| payload.starts_with(b"CVAMBDY1"))
            .count(),
        1,
        "metadata-only delta reuses the immutable body"
    );
    assert_no_legacy_publication_sidecars(path);

    let before_tombstone = file_bytes(path);
    let outcome = owner.tombstone(first.id, second.revision).unwrap();
    let TombstoneOutcome::Published(coordinates) = outcome else {
        panic!("first terminal write must publish")
    };
    assert_eq!(owner.memory_version(), 3);
    assert_eq!(memory_envelopes(path).len(), 3);
    assert_no_legacy_publication_sidecars(path);
    let terminal_payload = memory_envelopes(path).pop().unwrap();
    let envelope = publication_envelope::decode(&terminal_payload)
        .unwrap()
        .unwrap();
    assert!(matches!(
        publication::decode(envelope.body, envelope.global_version).unwrap(),
        Some(MemoryPublication::Tombstone { id, revision: 3, memory_version: 3 }) if id == first.id
    ));

    let after_tombstone = file_bytes(path);
    assert_ne!(before_tombstone, after_tombstone);
    assert_eq!(
        owner.tombstone(first.id, second.revision).unwrap(),
        TombstoneOutcome::AlreadyTombstoned(coordinates)
    );
    assert_eq!(
        file_bytes(path),
        after_tombstone,
        "terminal retry is byte-stable"
    );
    assert_eq!(owner.memory_version(), 3);
}

#[test]
fn direct_rel_and_phy_writes_are_one_envelope_each_and_reopen() {
    let scratch = Scratch::new();
    let rel_path = scratch.file("direct.rel");
    let mut rel = Cva::create(&rel_path).unwrap();
    exercise_direct_writer(&mut rel, &rel_path);
    rel.sync().unwrap();
    drop(rel);
    let reopened = Cva::open(&rel_path).unwrap();
    assert_eq!(reopened.memories.records().len(), 2);
    assert_eq!(reopened.memory_version(), 3);
    assert!(matches!(
        reopened.memory_lifecycle_state(MemoryId([0; 32])),
        Err(MemoryError::MissingMemory)
    ));

    let phy_path = scratch.file("direct.phy");
    let mut phy = Phylactery::create(&phy_path).unwrap();
    exercise_direct_writer(&mut phy, &phy_path);
    phy.sync().unwrap();
    drop(phy);
    let reopened = Phylactery::open(&phy_path).unwrap();
    assert_eq!(reopened.memories.records().len(), 2);
    assert_eq!(reopened.memory_version(), 3);
    assert_no_legacy_publication_sidecars(&phy_path);
}

fn prepare_two_phy_envelopes(path: &Path) -> (Memory, u64, Option<i64>) {
    let mut phy = Phylactery::create(path).unwrap();
    let first = phy
        .publish_memory(None, 0, draft("crash-create", "first"))
        .unwrap()
        .0;
    let second = phy
        .publish_memory(Some(first.id), 1, draft("crash-revise", "revised"))
        .unwrap()
        .0;
    phy.sync().unwrap();
    let time = phy.transaction_time_ns(first.global_version);
    let latest = phy.latest_global_version();
    let payloads = payloads(path);
    assert!(payloads.last().unwrap().starts_with(b"CVAPUB01"));
    assert_eq!(memory_envelopes(path).len(), 2);
    (first, latest, time)
}

#[test]
fn truncated_final_memory_envelope_recovers_only_prior_state_and_clock() {
    let scratch = Scratch::new();
    let source = scratch.file("crash-source.phy");
    let truncated = scratch.file("crash-tail.phy");
    let (first, latest, time) = prepare_two_phy_envelopes(&source);
    let bytes = file_bytes(&source);
    let mut container = Container::open(&source).unwrap();
    let start = container.chunks().unwrap().last().unwrap().offset() as usize;
    drop(container);
    for retained in start..bytes.len() {
        std::fs::write(&truncated, &bytes[..retained]).unwrap();
        let mut recovered = Phylactery::open(&truncated).unwrap();
        assert_eq!(recovered.memories.records().len(), 1, "prefix {retained}");
        assert_eq!(recovered.memory_version(), 1);
        assert_eq!(recovered.latest_global_version(), first.global_version);
        assert_eq!(recovered.transaction_time_ns(first.global_version), time);
        assert_eq!(recovered.memory(first.id).unwrap().revision, 1);
        assert!(recovered.latest_global_version() < latest);
        drop(recovered);
        assert_eq!(memory_envelopes(&truncated).len(), 1);
        assert_no_legacy_publication_sidecars(&truncated);
        assert_eq!(file_bytes(&truncated), bytes[..start]);
    }
}
