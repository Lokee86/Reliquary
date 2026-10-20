use super::*;
use crate::memory_compact_codec::publication::{self, MemoryPublication};
use crate::memory_compact_contract::MemoryDelta;
use crate::{Cva, MemoryId};
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "memory-envelope-reconcile-{}",
                uuid::Uuid::new_v4()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn base(path: &std::path::Path) -> MemoryId {
    let mut owner = Cva::create(path).unwrap();
    let draft = crate::entity_candidate_test_support::memory_draft(
        "envelope-base",
        "Stable title",
        "Stable body",
    );
    let id = owner.publish_memory(None, 0, draft).unwrap().0.id;
    owner.sync().unwrap();
    id
}

fn append_delta(path: &std::path::Path, id: MemoryId) -> (usize, Vec<u8>) {
    let owner = Cva::open(path).unwrap();
    let previous = owner.memories.records().last().unwrap().clone();
    assert_eq!(previous.id, id);
    let mut next = previous.clone();
    next.revision += 1;
    next.memory_version += 1;
    next.global_version = owner.latest_global_version() + 1;
    next.scope = "personal".into();
    next.mutation_id = "envelope-revision".into();
    next.updated_at_ns += 1;
    let body = publication::encode(&MemoryPublication::Delta(
        MemoryDelta::between(&previous, &next).unwrap(),
    ))
    .unwrap();
    drop(owner);
    let mut container = crate::Container::open(path).unwrap();
    let reserved = container
        .allocate_version_with_transaction_time(Some(-123))
        .unwrap();
    assert_eq!(reserved, next.global_version);
    container
        .allocate_version_with_transaction_time(Some(500))
        .unwrap();
    let index = container.chunks().unwrap().len();
    let framed = crate::container::publication_envelope::encode(
        crate::container::publication_envelope::PublicationEnvelope {
            global_version: reserved,
            clock: crate::container::publication_envelope::PublicationClock::BindReserved,
            body: &body,
        },
    )
    .unwrap();
    let object = container.append(&framed).unwrap();
    let payload = container.read(object).unwrap();
    container.sync().unwrap();
    (index, payload)
}

#[test]
fn envelope_range_and_import_interval_replay_one_revision_with_original_time() {
    let f = Fixture::new();
    let source = f.path("source.rel");
    let target = f.path("target.rel");
    let id = base(&source);
    fs::copy(&source, &target).unwrap();
    let (index, payload) = append_delta(&source, id);
    let mut source = Cva::open(&source).unwrap();
    let mut target = Cva::open(&target).unwrap();
    super::validate_replay_interval(&mut source, index).unwrap();
    let mut coordinates =
        crate::imports::reconciliation_interval::Coordinates::shared(&mut source, index).unwrap();
    crate::imports::reconciliation_interval::replay_frame(
        &mut source,
        &mut target,
        index,
        &payload,
        &mut coordinates,
    )
    .unwrap();
    assert_eq!(coordinates.memory_revisions, 1);
    assert_eq!(coordinates.duplicate_memory_revisions, 0);
    let revision = target.memory_revision(id, 2).unwrap();
    assert_eq!(revision.scope, "personal");
    assert_eq!(
        target
            .container
            .transaction_time_ns(revision.global_version),
        Some(-123)
    );
    let (global, local) = publication::coordinates(&payload).unwrap().unwrap();
    let evidence =
        super::canonical_memory_publication(&mut source, global, local, &payload).unwrap();
    let mut expected = source.memories.records().last().unwrap().clone();
    expected.global_version = 0;
    expected.memory_version = 0;
    assert_eq!(
        evidence,
        crate::memory_codec::encode_record(&expected).unwrap()
    );
}

#[test]
fn divergent_reconciliation_retains_envelope_memory_revision() {
    let f = Fixture::new();
    let left = f.path("left.rel");
    let right = f.path("right.rel");
    let output = f.path("output.rel");
    let id = base(&left);
    fs::copy(&left, &right).unwrap();
    append_delta(&right, id);
    let mut left_owner = Cva::open(&left).unwrap();
    left_owner
        .append_node(
            "left-only".into(),
            "left-conversation".into(),
            None,
            "user".into(),
            50,
            "Unrelated branch.",
        )
        .unwrap();
    left_owner.sync().unwrap();
    drop(left_owner);
    let result = Cva::reconcile(&left, &right, &output).unwrap();
    assert_eq!(result.replayed_memory_revisions, 1);
    let mut owner = Cva::open(&output).unwrap();
    let revision = owner.memory_revision(id, 2).unwrap();
    assert_eq!(revision.scope, "personal");
    assert_eq!(
        owner.container.transaction_time_ns(revision.global_version),
        Some(-123)
    );
}

#[test]
fn envelope_terminal_canonical_evidence_matches_existing_terminal_facts() {
    let f = Fixture::new();
    let path = f.path("terminal.rel");
    let id = base(&path);
    let mut container = crate::Container::open(&path).unwrap();
    let body = publication::encode(&MemoryPublication::Tombstone {
        id,
        revision: 2,
        memory_version: 2,
    })
    .unwrap();
    let (global, object) = container.append_publication_envelope(&body).unwrap();
    let payload = container.read(object).unwrap();
    container.sync().unwrap();
    drop(container);
    let mut owner = Cva::open(&path).unwrap();
    assert_eq!(
        super::canonical_memory_publication(&mut owner, global, 2, &payload).unwrap(),
        crate::memory_codec::encode_tombstone(crate::memory_codec::MemoryTombstone {
            id,
            revision: 2
        })
    );
    // Terminal replay was already unsupported by this revision replay API.
    assert!(crate::cva_reconcile_memory::read_memory_range(&mut owner, 0, usize::MAX).is_err());
}

#[test]
fn import_classifier_rejects_unknown_and_malformed_envelopes() {
    use crate::container::publication_envelope::{PublicationClock, PublicationEnvelope, encode};
    let unknown = encode(PublicationEnvelope {
        global_version: 1,
        clock: PublicationClock::BindReserved,
        body: b"UNKNOWN1opaque",
    })
    .unwrap();
    let truncated_memory = encode(PublicationEnvelope {
        global_version: 1,
        clock: PublicationClock::BindReserved,
        body: b"CVAMD002",
    })
    .unwrap();
    for payload in [b"CVAPUB02".to_vec(), unknown, truncated_memory] {
        let f = Fixture::new();
        let path = f.path("bad-envelope.rel");
        let mut owner = Cva::create(&path).unwrap();
        let boundary = owner.container.chunks().unwrap().len();
        owner.container.append(&payload).unwrap();
        assert!(super::validate_replay_interval(&mut owner, boundary).is_err());
    }
}
