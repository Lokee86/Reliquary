//! Step188: convert accepted V1 Entity links to reserved publication envelopes.
use super::*;
use crate::container::publication_envelope::{self, PublicationClock};
use crate::entity_codec::{
    DirectEntityTombstone, EntityVersion, decode_version, encode_direct_tombstone, encode_version,
};
use crate::entity_compact_codec::publication::{self, EntityPublication};
use crate::entity_model::EntityRecord;
use crate::historical_entity_fingerprint::entity_cut;
use crate::{
    Container, ContainerError, Cva, EntityGlobalId, EntityId, FileKind, ObjectRef, Phylactery,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

fn append_raw_frame(bytes: &mut Vec<u8>, payload: &[u8]) -> ObjectRef {
    let offset = bytes.len() as u64;
    bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(payload);
    let mut reference = [0; 16];
    reference[..8].copy_from_slice(&offset.to_le_bytes());
    reference[8..].copy_from_slice(&(payload.len() as u64).to_le_bytes());
    ObjectRef::from_legacy_bytes(reference)
}

fn append_direct_terminal(path: &Path, id: EntityId, revision: u64, global: u64, local: u64) {
    let mut bytes = fs::read(path).unwrap();
    let carrier = append_raw_frame(
        &mut bytes,
        &encode_direct_tombstone(DirectEntityTombstone { id, revision }),
    );
    append_raw_frame(
        &mut bytes,
        &encode_version(EntityVersion {
            global_version: global,
            entity_version: local,
            record: carrier,
        }),
    );
    fs::write(path, bytes).unwrap();
}

fn source_with_both_terminals(path: &Path, kind: FileKind) -> (EntityId, EntityId) {
    let (retired, survivor) = create_v1_fixture(path, kind);
    append_direct_terminal(path, survivor, 2, 8, 8);
    (retired, survivor)
}

fn append_resurrection(path: &Path, retired: EntityId) {
    let mut bytes = fs::read(path).unwrap();
    let mut ticket = b"CVAVERS2".to_vec();
    ticket.extend_from_slice(&9_u64.to_le_bytes());
    ticket.extend_from_slice(&900_i64.to_le_bytes());
    append_raw_frame(&mut bytes, &ticket);
    let record = EntityRecord {
        id: retired,
        global_id: Some(EntityGlobalId([0xEF; 16])),
        revision: 7,
        canonical_name: "Legacy project entity".into(),
        aliases: vec!["Legacy alias".into()],
        kind: "project".into(),
        summary: "Stable semantic evidence".into(),
        mutation_id: "invalid-resurrection".into(),
        created_at_ns: 10,
        updated_at_ns: 10,
        global_version: 0,
        entity_version: 0,
    };
    let carrier = append_raw_frame(
        &mut bytes,
        &crate::entity_codec::encode_record(&record).unwrap(),
    );
    append_raw_frame(
        &mut bytes,
        &encode_version(EntityVersion {
            global_version: 9,
            entity_version: 9,
            record: carrier,
        }),
    );
    fs::write(path, bytes).unwrap();
}

fn owner_payloads(path: &Path, v1: bool) -> Vec<(ObjectRef, Vec<u8>)> {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut payloads = Vec::new();
    container
        .visit_payloads::<ContainerError>(|object, payload| {
            payloads.push((object, payload.to_vec()));
            Ok(())
        })
        .unwrap();
    payloads
}

fn assert_converted_payloads(source: &Path, output: &Path) {
    let source_payloads = owner_payloads(source, true);
    let output_payloads = owner_payloads(output, false);
    let mut accepted_carriers = Vec::new();
    let mut source_container = Container::open_v1_read_only(source).unwrap();
    for (_, payload) in &source_payloads {
        if let Some(version) = decode_version(payload).unwrap() {
            accepted_carriers.push(source_container.read(version.record).unwrap());
        }
    }
    let orphan = source_payloads
        .iter()
        .find(|(_, payload)| {
            crate::entity_codec::decode_record(payload)
                .ok()
                .flatten()
                .is_some_and(|record| record.mutation_id == "unpublished-carrier")
        })
        .map(|(_, payload)| payload.clone())
        .expect("fixture includes an unmarked Entity carrier");
    let output_bytes = output_payloads
        .iter()
        .map(|(_, payload)| payload)
        .collect::<Vec<_>>();
    assert!(
        output_bytes
            .iter()
            .any(|payload| payload.as_slice() == orphan.as_slice())
    );
    for carrier in &accepted_carriers {
        assert!(
            output_bytes
                .iter()
                .all(|payload| payload.as_slice() != carrier.as_slice()),
            "accepted V1 Entity carrier is omitted"
        );
    }
    assert!(output_bytes.iter().all(|payload| {
        !payload.starts_with(b"CVAENTV1")
            && !payload.starts_with(b"CVAENTT1")
            && !payload.starts_with(b"CVAENTT2")
    }));

    let retained_raw = output_bytes
        .iter()
        .filter(|payload| payload.starts_with(b"CVAENTR1") || payload.starts_with(b"CVAENTR2"))
        .map(|payload| payload.as_slice())
        .collect::<Vec<_>>();
    assert_eq!(
        retained_raw,
        vec![orphan.as_slice()],
        "only the unaccepted raw carrier remains"
    );

    let mut envelopes = Vec::new();
    for payload in &output_bytes {
        if let Some(envelope) = publication_envelope::decode(payload).unwrap() {
            let body = publication::decode(envelope.body, envelope.global_version)
                .unwrap()
                .expect("Entity envelope body");
            envelopes.push((envelope.global_version, envelope.clock, body));
        }
    }
    envelopes.sort_by_key(|(global, _, _)| *global);
    assert_eq!(
        envelopes
            .iter()
            .map(|(global, _, _)| *global)
            .collect::<Vec<_>>(),
        (1..=8).collect::<Vec<_>>()
    );
    assert!(
        envelopes
            .iter()
            .all(|(_, clock, _)| *clock == PublicationClock::BindReserved)
    );
    assert!(matches!(envelopes[0].2, EntityPublication::Delta(_)));
    assert!(matches!(envelopes[1].2, EntityPublication::Creation(_)));
    assert!(matches!(
        &envelopes[6].2,
        EntityPublication::MergeTombstone { .. }
    ));
    assert!(matches!(
        &envelopes[7].2,
        EntityPublication::DirectTombstone { .. }
    ));
}

fn assert_history_parity(source: &Path, output: &Path, kind: FileKind) {
    let mut source_container = Container::open_v1_read_only(source).unwrap();
    let mut output_container = Container::open(output).unwrap();
    match kind {
        FileKind::Reliquary => {
            let source_owner = Cva::open_v1_read_only(source).unwrap();
            let output_owner = Cva::open(output).unwrap();
            assert_eq!(
                source_owner.container.identity(),
                output_owner.container.identity()
            );
            assert_eq!(source_owner.owner_uuid(), output_owner.owner_uuid());
            assert_eq!(source_owner.entity_version(), 8);
            assert_eq!(output_owner.entity_version(), 8);
            assert_eq!(source_owner.latest_global_version(), 8);
            assert_eq!(output_owner.latest_global_version(), 8);
            assert_eq!(
                source_owner.entities.records(),
                output_owner.entities.records()
            );
            for cut in 0..=8 {
                let before =
                    entity_cut(&source_owner.container, &source_owner.entities, cut).unwrap();
                let after =
                    entity_cut(&output_owner.container, &output_owner.entities, cut).unwrap();
                assert_eq!(before.fingerprint, after.fingerprint, "Entity cut {cut}");
                assert_eq!(before.current, after.current, "current Entity set at {cut}");
                assert_eq!(
                    before.replacements, after.replacements,
                    "terminals at {cut}"
                );
            }
            for global in 0..=8 {
                assert_eq!(
                    source_container.transaction_time_ns(global),
                    output_container.transaction_time_ns(global),
                    "global ticket {global}"
                );
            }
            for id in [EntityId([0x61; 32]), EntityId([0x62; 32])] {
                assert_eq!(
                    source_owner.entity_retention_descriptor(id).unwrap(),
                    output_owner.entity_retention_descriptor(id).unwrap()
                );
            }
        }
        FileKind::Phylactery => {
            let source_owner = Phylactery::open_v1_read_only(source).unwrap();
            let output_owner = Phylactery::open(output).unwrap();
            assert_eq!(
                source_owner.container.identity(),
                output_owner.container.identity()
            );
            assert_eq!(source_owner.owner_uuid(), output_owner.owner_uuid());
            assert_eq!(source_owner.entity_version(), 8);
            assert_eq!(output_owner.entity_version(), 8);
            assert_eq!(source_owner.latest_global_version(), 8);
            assert_eq!(output_owner.latest_global_version(), 8);
            assert_eq!(
                source_owner.entities.records(),
                output_owner.entities.records()
            );
            for cut in 0..=8 {
                let before =
                    entity_cut(&source_owner.container, &source_owner.entities, cut).unwrap();
                let after =
                    entity_cut(&output_owner.container, &output_owner.entities, cut).unwrap();
                assert_eq!(before.fingerprint, after.fingerprint, "Entity cut {cut}");
                assert_eq!(before.current, after.current, "current Entity set at {cut}");
                assert_eq!(
                    before.replacements, after.replacements,
                    "terminals at {cut}"
                );
            }
            for global in 0..=8 {
                assert_eq!(
                    source_container.transaction_time_ns(global),
                    output_container.transaction_time_ns(global),
                    "global ticket {global}"
                );
            }
            for id in [EntityId([0x61; 32]), EntityId([0x62; 32])] {
                assert_eq!(
                    source_owner.entity_retention_descriptor(id).unwrap(),
                    output_owner.entity_retention_descriptor(id).unwrap()
                );
            }
        }
    }
}

fn exercise(kind: FileKind) {
    let fixture = Fixture::new();
    let source = fixture.0.join(match kind {
        FileKind::Reliquary => "source.rel",
        FileKind::Phylactery => "source.phy",
    });
    source_with_both_terminals(&source, kind);
    let before = fs::read(&source).unwrap();
    let before_hash = Sha256::digest(&before);
    let before_modified = fs::metadata(&source).unwrap().modified().unwrap();
    let request = StorageUpgradeRequest {
        source: source.clone(),
        destination: fixture.0.join("output.must-not-exist"),
    };
    let plan = plan_storage_upgrade(request.clone()).unwrap();
    let staged = crate::storage_upgrade::stage_entity_envelope_storage_upgrade(&plan).unwrap();
    assert_history_parity(&source, staged.test_path(), kind);
    assert_converted_payloads(&source, staged.test_path());
    assert_eq!(fs::read(&source).unwrap(), before);
    assert_eq!(Sha256::digest(fs::read(&source).unwrap()), before_hash);
    assert_eq!(
        fs::metadata(&source).unwrap().modified().unwrap(),
        before_modified
    );
    assert!(!request.destination.exists());
}

#[test]
fn rel_entity_links_become_reserved_envelopes_with_history_preserved() {
    exercise(FileKind::Reliquary);
}

#[test]
fn phy_entity_links_become_reserved_envelopes_with_history_preserved() {
    exercise(FileKind::Phylactery);
}

#[test]
fn invalid_entity_dangling_marker_and_resurrection_never_return_a_stage() {
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
        for resurrection in [false, true] {
            let fixture = Fixture::new();
            let source = fixture.0.join("invalid-source.cva");
            let (retired, _) = create_v1_fixture(&source, kind);
            append_direct_terminal(&source, EntityId([0x62; 32]), 2, 8, 8);
            if resurrection {
                append_resurrection(&source, retired);
            } else {
                let mut container = Container::open_v1_read_only(&source).unwrap();
                let mut changed = None;
                for object in container.chunks().unwrap() {
                    let payload = container.read(object).unwrap();
                    if let Some(version) = decode_version(&payload).unwrap() {
                        changed = Some((object.offset() + 8, version));
                        break;
                    }
                }
                drop(container);
                let (offset, mut version) = changed.unwrap();
                version.record = ObjectRef::from_legacy_bytes([0; 16]);
                let marker = encode_version(version);
                let mut file = fs::OpenOptions::new().write(true).open(&source).unwrap();
                file.seek(SeekFrom::Start(offset)).unwrap();
                file.write_all(&marker).unwrap();
                file.sync_all().unwrap();
            }
            let before = fs::read(&source).unwrap();
            let modified = fs::metadata(&source).unwrap().modified().unwrap();
            let request = StorageUpgradeRequest {
                source: source.clone(),
                destination: fixture.0.join("invalid-output.must-not-exist"),
            };
            let plan = plan_storage_upgrade(request.clone()).unwrap();
            assert!(crate::storage_upgrade::stage_entity_envelope_storage_upgrade(&plan).is_err());
            assert_eq!(fs::read(&source).unwrap(), before);
            assert_eq!(fs::metadata(&source).unwrap().modified().unwrap(), modified);
            assert!(!request.destination.exists());
            assert!(fs::read_dir(&fixture.0).unwrap().all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".storage-upgrade-")
            }));
        }
    }
}
