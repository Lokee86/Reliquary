//! Step178: migrate accepted legacy Memory links into publication envelopes.
//!
//! The private stage preserves the original publication frontier and tickets.

use super::{StorageUpgradeRequest, plan_storage_upgrade, stage_memory_envelope_storage_upgrade};
use crate::container::publication_envelope::{self, PublicationClock};
use crate::historical_memory_fingerprint::MemoryHistory;
use crate::memory_codec::{
    MemoryTombstone, MemoryVersion, encode_body, encode_record, encode_tombstone, encode_version,
};
use crate::memory_compact_codec::publication::{self, MemoryPublication};
use crate::memory_model::{
    MemoryBodyId, MemoryId, MemoryRecord, memory_body_bytes, memory_body_id,
};
use crate::{Container, ContainerError, Cva, FileKind, Phylactery};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("memory-envelope-upgrade-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn request(&self, kind: FileKind) -> StorageUpgradeRequest {
        StorageUpgradeRequest {
            source: self.0.join(match kind {
                FileKind::Reliquary => "source.rel",
                FileKind::Phylactery => "source.phy",
            }),
            destination: self.0.join("output.must-not-exist"),
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn record(revision: u64, mutation: &str, scope: &str, body_id: MemoryBodyId) -> MemoryRecord {
    MemoryRecord {
        id: MemoryId([0x31; 32]),
        revision,
        body_id,
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: if revision == 1 {
            "current"
        } else {
            "historical"
        }
        .into(),
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
        source_ref: None,
        temporal_inference: None,
        mutation_id: mutation.into(),
        created_at_ns: 100,
        updated_at_ns: 100 + (revision as i64 - 1) * 10,
        // Legacy postimages do not carry these two coordinates; CVAMEMV1 does.
        global_version: 0,
        memory_version: 0,
    }
}

fn downgrade_header_to_v1(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

fn append_legacy_link(
    container: &mut Container,
    global_version: u64,
    memory_version: u64,
    record_ref: crate::ObjectRef,
) {
    // Link global tickets were reserved independently and may be referenced in
    // an order different from their numeric order.
    assert!(global_version > 0 && memory_version > 0);
    container
        .append(&encode_version(MemoryVersion {
            global_version,
            memory_version,
            record: record_ref,
        }))
        .unwrap();
}

fn build_v1_source(path: &Path, kind: FileKind) -> Vec<(u64, Vec<u8>)> {
    let mut owner = match kind {
        FileKind::Reliquary => Owner::Rel(Cva::create(path).unwrap()),
        FileKind::Phylactery => Owner::Phy(Phylactery::create(path).unwrap()),
    };
    let container = owner.container_mut();

    let title = "Legacy body";
    let content = "The payload and its history survive envelope conversion.";
    let body_bytes = memory_body_bytes(title, content);
    let body_id = memory_body_id(title, content);
    container
        .append(&encode_body(body_id, &body_bytes).unwrap())
        .unwrap();

    // Times are deliberately nonmonotonic, and ticket 2 has no timestamp.
    // Ticket 3 is later referenced by the first Memory link while ticket 1 is
    // referenced by the second, exercising reverse global coordinates.
    for time in [Some(900), None, Some(-25), Some(40)] {
        container
            .allocate_version_with_transaction_time(time)
            .unwrap();
    }
    container
        .append(b"interleaved-inert-owner-payload")
        .unwrap();

    let first = record(1, "legacy-birth", "one", body_id);
    let second = record(2, "legacy-revision", "two", body_id);
    let first_bytes = encode_record(&first).unwrap();
    let second_bytes = encode_record(&second).unwrap();

    // Append postimage carriers in reverse order. Markers then establish the
    // accepted owner-local sequence, with descending global-ticket links.
    let second_ref = container.append(&second_bytes).unwrap();
    let first_ref = container.append(&first_bytes).unwrap();
    let orphan = record(9, "inert-orphan", "orphan", MemoryBodyId([0x99; 32]));
    let orphan_bytes = encode_record(&orphan).unwrap();
    container.append(&orphan_bytes).unwrap();
    append_legacy_link(container, 3, 1, first_ref);
    container.append(b"between-memory-publications").unwrap();
    append_legacy_link(container, 1, 2, second_ref);

    let terminal = MemoryTombstone {
        id: first.id,
        revision: 3,
    };
    let terminal_bytes = encode_tombstone(terminal);
    let terminal_ref = container.append(&terminal_bytes).unwrap();
    append_legacy_link(container, 4, 3, terminal_ref);
    owner.sync();

    let accepted_carriers = vec![(3, first_bytes), (1, second_bytes), (4, terminal_bytes)];
    drop(owner);
    downgrade_header_to_v1(path);
    // Keep the exact orphan fixture available to the assertions by checking
    // it independently from the linked carrier list.
    let mut container = Container::open_v1_read_only(path).unwrap();
    let mut found_orphan = false;
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            found_orphan |= payload == orphan_bytes;
            Ok(())
        })
        .unwrap();
    assert!(found_orphan);
    accepted_carriers
}

enum Owner {
    Rel(Cva),
    Phy(Phylactery),
}

impl Owner {
    fn container_mut(&mut self) -> &mut Container {
        match self {
            Self::Rel(owner) => &mut owner.container,
            Self::Phy(owner) => &mut owner.container,
        }
    }

    fn sync(&self) {
        match self {
            Self::Rel(owner) => owner.sync().unwrap(),
            Self::Phy(owner) => owner.sync().unwrap(),
        }
    }
}

fn assert_converted(path: &Path, original_carriers: &[(u64, Vec<u8>)]) {
    let mut container = Container::open(path).unwrap();
    let mut payloads = Vec::new();
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            payloads.push(payload.to_vec());
            Ok(())
        })
        .unwrap();

    let mut publications = Vec::new();
    for payload in &payloads {
        if let Some(envelope) = publication_envelope::decode(payload).unwrap() {
            assert_eq!(envelope.clock, PublicationClock::BindReserved);
            let publication = publication::decode(envelope.body, envelope.global_version)
                .unwrap()
                .expect("Memory publication envelope body");
            publications.push((envelope.global_version, publication));
        }
    }
    assert_eq!(
        publications
            .iter()
            .map(|(global, _)| *global)
            .collect::<Vec<_>>(),
        vec![3, 1, 4]
    );
    let creation_slot = payloads
        .iter()
        .position(|payload| {
            publication_envelope::decode(payload)
                .unwrap()
                .is_some_and(|envelope| envelope.global_version == 3)
        })
        .unwrap();
    let separator_slot = payloads
        .iter()
        .position(|payload| payload == b"between-memory-publications")
        .unwrap();
    let delta_slot = payloads
        .iter()
        .position(|payload| {
            publication_envelope::decode(payload)
                .unwrap()
                .is_some_and(|envelope| envelope.global_version == 1)
        })
        .unwrap();
    assert!(creation_slot < separator_slot && separator_slot < delta_slot);
    publications.sort_by_key(|(global, _)| *global);
    assert_eq!(
        publications
            .iter()
            .map(|(global, _)| *global)
            .collect::<Vec<_>>(),
        vec![1, 3, 4]
    );
    assert!(matches!(publications[0].1, MemoryPublication::Delta(_)));
    assert!(matches!(publications[1].1, MemoryPublication::Creation(_)));
    assert!(matches!(
        publications[2].1,
        MemoryPublication::Tombstone { .. }
    ));

    for (_, carrier) in original_carriers {
        assert!(
            !payloads.iter().any(|payload| payload == carrier),
            "accepted legacy Memory carrier was retained"
        );
    }
    assert!(
        payloads
            .iter()
            .all(|payload| !payload.starts_with(b"CVAMEMV1") && !payload.starts_with(b"CVAMEMT1"))
    );
    assert!(
        payloads
            .iter()
            .any(|payload| payload == b"interleaved-inert-owner-payload")
    );
    let orphan = encode_record(&record(
        9,
        "inert-orphan",
        "orphan",
        MemoryBodyId([0x99; 32]),
    ))
    .unwrap();
    assert!(
        payloads.iter().any(|payload| payload == &orphan),
        "unlinked legacy Memory postimage remains inert and retained"
    );
}

fn assert_history_parity(source_path: &Path, output_path: &Path, kind: FileKind) {
    let (mut source, mut output) = match kind {
        FileKind::Reliquary => (
            Owner::Rel(Cva::open_v1_read_only(source_path).unwrap()),
            Owner::Rel(Cva::open(output_path).unwrap()),
        ),
        FileKind::Phylactery => (
            Owner::Phy(Phylactery::open_v1_read_only(source_path).unwrap()),
            Owner::Phy(Phylactery::open(output_path).unwrap()),
        ),
    };
    let (source_container, source_store, output_container, output_store) =
        match (&mut source, &mut output) {
            (Owner::Rel(source), Owner::Rel(output)) => (
                &mut source.container,
                &source.memories,
                &mut output.container,
                &output.memories,
            ),
            (Owner::Phy(source), Owner::Phy(output)) => (
                &mut source.container,
                &source.memories,
                &mut output.container,
                &output.memories,
            ),
            _ => unreachable!(),
        };
    let source_history = MemoryHistory::read(source_container, source_store).unwrap();
    let output_history = MemoryHistory::read(output_container, output_store).unwrap();
    for cut in 0..=source_store.memory_version() {
        let source_at = source_history.at(cut).unwrap();
        let output_at = output_history.at(cut).unwrap();
        assert_eq!(source_at.fingerprint, output_at.fingerprint, "cut {cut}");
        assert_eq!(
            source_at.current, output_at.current,
            "current membership at cut {cut}"
        );
    }
    for global in 0..=source_container.latest_version() {
        assert_eq!(
            source_container.transaction_time_ns(global),
            output_container.transaction_time_ns(global),
            "transaction time at global ticket {global}"
        );
    }
    let head = match &source {
        Owner::Rel(owner) => owner.latest_global_version(),
        Owner::Phy(owner) => owner.latest_global_version(),
    };
    for global in 0..=head {
        let (before, after) = match (&source, &output) {
            (Owner::Rel(source), Owner::Rel(output)) => (
                crate::historical_global_cuts::reliquary_global_cuts(source, global).unwrap(),
                crate::historical_global_cuts::reliquary_global_cuts(output, global).unwrap(),
            ),
            (Owner::Phy(source), Owner::Phy(output)) => (
                crate::historical_global_cuts::phylactery_global_cuts(source, global).unwrap(),
                crate::historical_global_cuts::phylactery_global_cuts(output, global).unwrap(),
            ),
            _ => unreachable!(),
        };
        assert_eq!(
            before, after,
            "owner-local frontiers at global cut {global}"
        );
    }
}

fn exercise(kind: FileKind) {
    let scratch = Scratch::new();
    let request = scratch.request(kind);
    let original_carriers = build_v1_source(&request.source, kind);
    let source_bytes = fs::read(&request.source).unwrap();
    let source_hash = Sha256::digest(&source_bytes);
    let source_modified = fs::metadata(&request.source).unwrap().modified().unwrap();

    let plan = plan_storage_upgrade(request.clone()).unwrap();
    let staged = stage_memory_envelope_storage_upgrade(&plan).unwrap();
    assert_history_parity(&request.source, staged.test_path(), kind);
    assert_converted(staged.test_path(), &original_carriers);

    let after = fs::read(&request.source).unwrap();
    let metadata = fs::metadata(&request.source).unwrap();
    assert_eq!(after, source_bytes, "V1 source bytes remain unchanged");
    assert_eq!(Sha256::digest(&after), source_hash);
    assert_eq!(metadata.len(), source_bytes.len() as u64);
    assert_eq!(metadata.modified().unwrap(), source_modified);
    assert!(!request.destination.exists());
}

#[test]
fn rel_memory_legacy_links_become_reserved_envelopes_without_changing_history() {
    exercise(FileKind::Reliquary);
}

#[test]
fn phy_memory_legacy_links_become_reserved_envelopes_without_changing_history() {
    exercise(FileKind::Phylactery);
}

#[path = "memory_envelope_failure_tests.rs"]
mod failure;
