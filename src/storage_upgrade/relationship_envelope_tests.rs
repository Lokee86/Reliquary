//! Step 198: Relationship accepted links convert to reserved envelopes.
use super::*;
use crate::ObjectRef;
use crate::container::publication_envelope::{self, PublicationClock};
use crate::historical_relationship_fingerprint::relationship_cut;
use crate::relationship_compact_codec::publication::{self, RelationshipPublication};
use crate::relationship_compact_contract::{RelationshipCreation, RelationshipDelta};
use crate::relationship_model::RelationshipRecord;

fn append_legacy_publication(container: &mut Container, record: &RelationshipRecord) -> ObjectRef {
    let carrier = container.append(&encode_record(record).unwrap()).unwrap();
    container
        .append(&encode_version(RelationshipVersion {
            global_version: record.global_version,
            relationship_version: record.relationship_version,
            record: carrier,
        }))
        .unwrap();
    carrier
}

fn append_compact_creation(container: &mut Container, record: &RelationshipRecord) -> ObjectRef {
    let bytes = crate::relationship_compact_codec::encode_creation(
        &RelationshipCreation::new(record.clone()).unwrap(),
    )
    .unwrap();
    let carrier = container.append(&bytes).unwrap();
    container
        .append(&encode_version(RelationshipVersion {
            global_version: record.global_version,
            relationship_version: record.relationship_version,
            record: carrier,
        }))
        .unwrap();
    carrier
}

fn append_direct_terminal(
    container: &mut Container,
    id: RelationshipId,
    revision: u64,
    global: u64,
    local: u64,
) -> ObjectRef {
    let carrier = container
        .append(&encode_tombstone(RelationshipTombstone { id, revision }))
        .unwrap();
    container
        .append(&encode_version(RelationshipVersion {
            global_version: global,
            relationship_version: local,
            record: carrier,
        }))
        .unwrap();
    carrier
}

fn reverse_history(path: &Path) -> (RelationshipRecord, RelationshipRecord, Vec<u8>) {
    let mut owner = Cva::create(path).unwrap();
    let result = append_reverse_history(&mut owner.container);
    owner.sync().unwrap();
    downgrade_header_to_v1(path);
    result
}

fn append_reverse_history(
    container: &mut Container,
) -> (RelationshipRecord, RelationshipRecord, Vec<u8>) {
    for (version, time) in [(1, 71_001), (2, 71_002), (3, 71_003), (4, 71_004)] {
        assert_eq!(
            container
                .allocate_version_with_transaction_time(Some(time))
                .unwrap(),
            version
        );
    }
    let mut first = record(1, 2, 1, "envelope-create");
    first.participants.sort();
    append_legacy_publication(container, &first);

    let mut second = first.clone();
    second.revision = 2;
    second.global_version = 1;
    second.relationship_version = 2;
    second.mutation_id = "envelope-field-change".into();
    second.kind = "updated kind".into();
    second.participants[0].role = Some("reviewer".into());
    second.evidence.push(MemoryRef {
        owner_id: "additional-evidence-owner".into(),
        memory_id: MemoryId([0x34; 32]),
    });
    second.evidence.sort();
    second.summary = "changed description".into();
    second.updated_at_ns = 202;
    second.participants.sort();
    append_legacy_publication(container, &second);

    let mut third = second.clone();
    third.revision = 3;
    third.global_version = 4;
    third.relationship_version = 3;
    third.mutation_id = "envelope-zero-mask".into();
    assert!(
        RelationshipDelta::between(&second, &third)
            .unwrap()
            .fields
            .mask()
            == 0
    );
    append_legacy_publication(container, &third);

    append_direct_terminal(container, first.id, 4, 3, 4);
    let mut orphan = record(1, 0, 0, "unpublished-relationship-carrier");
    orphan.id = RelationshipId([0x54; 32]);
    let orphan_bytes = encode_record(&orphan).unwrap();
    container.append(&orphan_bytes).unwrap();
    (first, second, orphan_bytes)
}

fn reverse_fixture(
    path: &Path,
    kind: FileKind,
) -> (RelationshipRecord, RelationshipRecord, Vec<u8>) {
    match kind {
        FileKind::Reliquary => reverse_history(path),
        FileKind::Phylactery => {
            let mut owner = Phylactery::create(path).unwrap();
            let expected = append_reverse_history(&mut owner.container);
            owner.sync().unwrap();
            downgrade_header_to_v1(path);
            expected
        }
    }
}

fn assert_reverse_envelopes(source: &Path, output: &Path, orphan: &[u8]) {
    let mut source_container = Container::open_v1_read_only(source).unwrap();
    let mut output_container = Container::open(output).unwrap();
    let source_payloads = source_container
        .chunks()
        .unwrap()
        .into_iter()
        .map(|object| (object, source_container.read(object).unwrap()))
        .collect::<Vec<_>>();
    let output_payloads = output_container
        .chunks()
        .unwrap()
        .into_iter()
        .map(|object| (object, output_container.read(object).unwrap()))
        .collect::<Vec<_>>();
    let mut accepted_carriers = Vec::new();
    for (_, payload) in &source_payloads {
        if let Some(version) = decode_version(payload).unwrap() {
            accepted_carriers.push(source_container.read(version.record).unwrap());
        }
    }
    assert!(
        output_payloads
            .iter()
            .any(|(_, payload)| payload.as_slice() == orphan)
    );
    for carrier in &accepted_carriers {
        assert!(
            output_payloads
                .iter()
                .all(|(_, payload)| payload != carrier),
            "accepted carrier omitted"
        );
    }

    let mut publications = Vec::new();
    for (_, payload) in &output_payloads {
        let Some(envelope) = publication_envelope::decode(payload).unwrap() else {
            continue;
        };
        let publication = publication::decode(envelope.body, envelope.global_version)
            .unwrap()
            .expect("Relationship envelope body");
        publications.push((envelope.global_version, envelope.clock, publication));
    }
    publications.sort_by_key(|(global, _, _)| *global);
    assert_eq!(publications.len(), 4);
    assert!(
        publications
            .iter()
            .all(|(_, clock, _)| *clock == PublicationClock::BindReserved)
    );
    assert!(
        output_payloads
            .iter()
            .all(|(_, payload)| !payload.starts_with(b"CVAAREC1"))
    );
    assert_eq!(
        publications
            .iter()
            .map(|(global, _, _)| *global)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert!(matches!(
        publications[0].2,
        RelationshipPublication::Delta(_)
    ));
    assert!(matches!(
        publications[1].2,
        RelationshipPublication::Creation(_)
    ));
    assert!(matches!(
        &publications[2].2,
        RelationshipPublication::Tombstone { .. }
    ));
    let RelationshipPublication::Delta(delta) = &publications[3].2 else {
        panic!("zero-mask delta must be retained")
    };
    assert_eq!(delta.fields.mask(), 0);
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
            assert_eq!(source_owner.relationship_version(), 4);
            assert_eq!(output_owner.relationship_version(), 4);
            assert_eq!(source_owner.latest_global_version(), 4);
            assert_eq!(output_owner.latest_global_version(), 4);
            assert_eq!(
                source_owner.relationships.records(),
                output_owner.relationships.records()
            );
            for cut in 0..=4 {
                let before =
                    relationship_cut(&source_owner.container, &source_owner.relationships, cut)
                        .unwrap();
                let after =
                    relationship_cut(&output_owner.container, &output_owner.relationships, cut)
                        .unwrap();
                assert_eq!(
                    before.fingerprint, after.fingerprint,
                    "Relationship cut {cut}"
                );
                assert_eq!(
                    before.current, after.current,
                    "current Relationship set at {cut}"
                );
            }
            for global in 0..=4 {
                assert_eq!(
                    crate::historical_global_cuts::reliquary_global_cuts(&source_owner, global)
                        .unwrap()
                        .relationship,
                    crate::historical_global_cuts::reliquary_global_cuts(&output_owner, global)
                        .unwrap()
                        .relationship,
                    "Relationship local cut at global frontier {global}"
                );
            }
            assert_eq!(
                source_owner
                    .relationship_retention_descriptor(RelationshipId([0x45; 32]))
                    .unwrap(),
                output_owner
                    .relationship_retention_descriptor(RelationshipId([0x45; 32]))
                    .unwrap()
            );
        }
        FileKind::Phylactery => {
            let source_owner = Phylactery::open_v1_read_only(source).unwrap();
            let output_owner = Phylactery::open(output).unwrap();
            assert_eq!(
                source_owner.container.identity(),
                output_owner.container.identity()
            );
            assert_eq!(source_owner.owner_uuid(), output_owner.owner_uuid());
            assert_eq!(source_owner.relationship_version(), 4);
            assert_eq!(output_owner.relationship_version(), 4);
            assert_eq!(source_owner.latest_global_version(), 4);
            assert_eq!(output_owner.latest_global_version(), 4);
            assert_eq!(
                source_owner.relationships.records(),
                output_owner.relationships.records()
            );
            for cut in 0..=4 {
                let before =
                    relationship_cut(&source_owner.container, &source_owner.relationships, cut)
                        .unwrap();
                let after =
                    relationship_cut(&output_owner.container, &output_owner.relationships, cut)
                        .unwrap();
                assert_eq!(
                    before.fingerprint, after.fingerprint,
                    "Relationship cut {cut}"
                );
                assert_eq!(
                    before.current, after.current,
                    "current Relationship set at {cut}"
                );
            }
            for global in 0..=4 {
                assert_eq!(
                    crate::historical_global_cuts::phylactery_global_cuts(&source_owner, global)
                        .unwrap()
                        .relationship,
                    crate::historical_global_cuts::phylactery_global_cuts(&output_owner, global)
                        .unwrap()
                        .relationship,
                    "Relationship local cut at global frontier {global}"
                );
            }
            assert_eq!(
                source_owner
                    .relationship_retention_descriptor(RelationshipId([0x45; 32]))
                    .unwrap(),
                output_owner
                    .relationship_retention_descriptor(RelationshipId([0x45; 32]))
                    .unwrap()
            );
        }
    }
    for global in 0..=4 {
        assert_eq!(
            source_container.transaction_time_ns(global),
            output_container.transaction_time_ns(global),
            "global ticket time {global}"
        );
    }
}

fn exercise(kind: FileKind) {
    let fixture = Fixture::new();
    let source = fixture.0.join(match kind {
        FileKind::Reliquary => "source.rel",
        FileKind::Phylactery => "source.phy",
    });
    let (_, _, orphan) = reverse_fixture(&source, kind);
    let source_bytes = fs::read(&source).unwrap();
    let source_hash = Sha256::digest(&source_bytes);
    let source_mtime = fs::metadata(&source).unwrap().modified().unwrap();
    let request = StorageUpgradeRequest {
        source: source.clone(),
        destination: fixture.0.join("output.must-not-be-published"),
    };
    let plan = plan_storage_upgrade(request.clone()).unwrap();
    let staged =
        crate::storage_upgrade::stage_relationship_envelope_storage_upgrade(&plan).unwrap();
    assert_history_parity(&source, staged.test_path(), kind);
    assert_reverse_envelopes(&source, staged.test_path(), &orphan);
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    assert_eq!(Sha256::digest(fs::read(&source).unwrap()), source_hash);
    assert_eq!(
        fs::metadata(&source).unwrap().modified().unwrap(),
        source_mtime
    );
    assert!(!request.destination.exists());
    drop(staged);
    assert!(!request.destination.exists());
    assert!(fs::read_dir(&fixture.0).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".storage-upgrade-")
    }));
}

#[test]
fn rel_relationship_links_become_reserved_envelopes_with_history_preserved() {
    exercise(FileKind::Reliquary);
}

#[test]
fn phy_relationship_links_become_reserved_envelopes_with_history_preserved() {
    exercise(FileKind::Phylactery);
}

fn invalid_missing_base_fixture(path: &Path, kind: FileKind) {
    match kind {
        FileKind::Reliquary => {
            let mut owner = Cva::create(path).unwrap();
            append_missing_base(&mut owner.container);
            owner.sync().unwrap();
        }
        FileKind::Phylactery => {
            let mut owner = Phylactery::create(path).unwrap();
            append_missing_base(&mut owner.container);
            owner.sync().unwrap();
        }
    }
    downgrade_header_to_v1(path);
}

fn append_missing_base(container: &mut Container) {
    for (global, time) in [(1, 81_001), (2, 81_002)] {
        assert_eq!(
            container
                .allocate_version_with_transaction_time(Some(time))
                .unwrap(),
            global
        );
    }
    let mut unrelated = record(1, 1, 1, "unrelated-seed");
    unrelated.id = RelationshipId([0x51; 32]);
    unrelated.participants.sort();
    unrelated.evidence.clear();
    append_compact_creation(container, &unrelated);

    let mut previous = record(1, 1, 1, "missing-base-old");
    previous.id = RelationshipId([0x52; 32]);
    previous.participants.sort();
    previous.evidence.sort();
    let mut next = previous.clone();
    next.revision = 2;
    next.global_version = 2;
    next.relationship_version = 2;
    next.mutation_id = "missing-base-delta".into();
    next.summary = "delta has no accepted base".into();
    let delta = RelationshipDelta::between(&previous, &next).unwrap();
    let carrier = container
        .append(&crate::relationship_compact_codec::encode_delta(&delta).unwrap())
        .unwrap();
    container
        .append(&encode_version(RelationshipVersion {
            global_version: 2,
            relationship_version: 2,
            record: carrier,
        }))
        .unwrap();
}

fn resurrection_fixture(path: &Path, kind: FileKind) {
    match kind {
        FileKind::Reliquary => {
            let mut owner = Cva::create(path).unwrap();
            append_resurrection(&mut owner.container);
            owner.sync().unwrap();
        }
        FileKind::Phylactery => {
            let mut owner = Phylactery::create(path).unwrap();
            append_resurrection(&mut owner.container);
            owner.sync().unwrap();
        }
    }
    downgrade_header_to_v1(path);
}

fn append_resurrection(container: &mut Container) {
    for (global, time) in [(1, 82_001), (2, 82_002), (3, 82_003)] {
        assert_eq!(
            container
                .allocate_version_with_transaction_time(Some(time))
                .unwrap(),
            global
        );
    }
    let mut first = record(1, 1, 1, "resurrection-create");
    first.id = RelationshipId([0x53; 32]);
    first.participants.sort();
    first.evidence.sort();
    append_compact_creation(container, &first);
    append_direct_terminal(container, first.id, 2, 2, 2);
    let mut resurrected = first;
    resurrected.revision = 3;
    resurrected.global_version = 3;
    resurrected.relationship_version = 3;
    resurrected.mutation_id = "resurrection-attempt".into();
    append_legacy_publication(container, &resurrected);
}

#[test]
fn missing_base_and_post_terminal_resurrection_reject_without_publishing_stage() {
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
        for invalid in ["missing-base", "resurrection"] {
            let fixture = Fixture::new();
            let source = fixture.0.join("invalid-source.cva");
            match invalid {
                "missing-base" => invalid_missing_base_fixture(&source, kind),
                _ => resurrection_fixture(&source, kind),
            }
            let bytes = fs::read(&source).unwrap();
            let digest = Sha256::digest(&bytes);
            let modified = fs::metadata(&source).unwrap().modified().unwrap();
            let request = StorageUpgradeRequest {
                source: source.clone(),
                destination: fixture.0.join("invalid-output.must-not-be-published"),
            };
            let plan = plan_storage_upgrade(request.clone()).unwrap();
            assert!(
                crate::storage_upgrade::stage_relationship_envelope_storage_upgrade(&plan).is_err()
            );
            assert_eq!(fs::read(&source).unwrap(), bytes);
            assert_eq!(Sha256::digest(fs::read(&source).unwrap()), digest);
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
