use super::shell::stage_ego_storage_upgrade;
use super::{StorageUpgradeRequest, plan_storage_upgrade};
use crate::ego_codec::{EgoRecord, encode};
use crate::historical_ego_fingerprint::EgoHistory;
use crate::{
    Container, ContainerError, Cva, EgoAnchorId, EgoAnchorPriority, EgoIdentityId, FileKind,
    Phylactery,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("ego-upgrade-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
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

fn tickets(container: &mut Container) {
    assert_eq!(
        container
            .allocate_version_with_transaction_time(Some(40_101))
            .unwrap(),
        1
    );
    assert_eq!(
        container
            .allocate_version_with_transaction_time(Some(40_202))
            .unwrap(),
        2
    );
}

fn rel_records() -> Vec<EgoRecord> {
    let id = EgoAnchorId([0x81; 16]);
    vec![
        EgoRecord::Anchor {
            ego_version: 1,
            id,
            revision: 1,
            deleted: false,
            priority: EgoAnchorPriority::Low,
            text: "REL anchor".into(),
        },
        EgoRecord::Synthesis {
            ego_version: 2,
            revision: 1,
            source_memory_version: 0,
            text: "REL synthesis".into(),
        },
        // Identical postimage is still an accepted raw publication.
        EgoRecord::Anchor {
            ego_version: 3,
            id,
            revision: 2,
            deleted: false,
            priority: EgoAnchorPriority::Low,
            text: "REL anchor".into(),
        },
        EgoRecord::Synthesis {
            ego_version: 4,
            revision: 2,
            source_memory_version: 0,
            text: "REL synthesis".into(),
        },
        // The terminal priority is intentionally different from the live one.
        EgoRecord::Anchor {
            ego_version: 5,
            id,
            revision: 3,
            deleted: true,
            priority: EgoAnchorPriority::High,
            text: String::new(),
        },
    ]
}

fn phy_records() -> Vec<EgoRecord> {
    let legacy_id = crate::ego_store::LEGACY_IDENTITY_ID;
    let work_id = EgoIdentityId([0x82; 16]);
    let anchor_id = EgoAnchorId([0x83; 16]);
    vec![
        EgoRecord::LegacyIdentity {
            ego_version: 1,
            revision: 1,
            text: "legacy default".into(),
        },
        EgoRecord::Identity {
            ego_version: 2,
            id: work_id,
            revision: 1,
            deleted: false,
            activate: false,
            name: "Work".into(),
            text: "first work".into(),
        },
        // Independent activation remains an explicit admitted legacy carrier.
        EgoRecord::IdentityActive {
            ego_version: 3,
            id: work_id,
        },
        EgoRecord::Identity {
            ego_version: 4,
            id: work_id,
            revision: 2,
            deleted: false,
            activate: true,
            name: "Work".into(),
            text: "revised work".into(),
        },
        // A repeated raw postimage, including activate=true, still publishes.
        EgoRecord::Identity {
            ego_version: 5,
            id: work_id,
            revision: 3,
            deleted: false,
            activate: true,
            name: "Work".into(),
            text: "revised work".into(),
        },
        EgoRecord::IdentityActive {
            ego_version: 6,
            id: legacy_id,
        },
        EgoRecord::Identity {
            ego_version: 7,
            id: work_id,
            revision: 4,
            deleted: true,
            activate: false,
            name: String::new(),
            text: String::new(),
        },
        EgoRecord::Personality {
            ego_version: 8,
            revision: 1,
            source_memory_version: 0,
            text: "clear and direct".into(),
        },
        EgoRecord::Personality {
            ego_version: 9,
            revision: 2,
            source_memory_version: 0,
            text: "clear and direct".into(),
        },
        EgoRecord::Anchor {
            ego_version: 10,
            id: anchor_id,
            revision: 1,
            deleted: false,
            priority: EgoAnchorPriority::Low,
            text: "PHY anchor".into(),
        },
        EgoRecord::Anchor {
            ego_version: 11,
            id: anchor_id,
            revision: 2,
            deleted: true,
            priority: EgoAnchorPriority::High,
            text: String::new(),
        },
        EgoRecord::Synthesis {
            ego_version: 12,
            revision: 1,
            source_memory_version: 0,
            text: "PHY synthesis".into(),
        },
        EgoRecord::Synthesis {
            ego_version: 13,
            revision: 2,
            source_memory_version: 0,
            text: "PHY synthesis".into(),
        },
    ]
}

fn create_v1_fixture(path: &Path, kind: FileKind, records: &[EgoRecord]) {
    match kind {
        FileKind::Reliquary => {
            let mut owner = Cva::create(path).unwrap();
            tickets(&mut owner.container);
            for record in records {
                owner.container.append(&encode(record).unwrap()).unwrap();
            }
            owner.sync().unwrap();
        }
        FileKind::Phylactery => {
            let mut owner = Phylactery::create(path).unwrap();
            tickets(&mut owner.container);
            for record in records {
                owner.container.append(&encode(record).unwrap()).unwrap();
            }
            owner.sync().unwrap();
        }
    }
    // Container::append above produced genuine frame checksums. The Ego bytes
    // are the admitted V1 raw record family; only the storage header version
    // is set to V1, matching the historical format's identical header layout.
    downgrade_header_to_v1(path);
}

fn is_ego_payload(payload: &[u8]) -> bool {
    [
        b"CVAEIDN".as_slice(),
        b"CVAEIDA",
        b"CVAEPER",
        b"CVAEANC",
        b"CVAESYN",
        b"CVAECR",
        b"CVAEDL",
        b"CVAETM",
    ]
    .iter()
    .any(|magic| payload.starts_with(magic))
}

fn ego_payload_measurement(path: &Path, v1: bool) -> (u64, usize, usize) {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open_compact(path).unwrap()
    };
    let mut bytes = 0;
    let mut frames = 0;
    let mut legacy_activation_frames = 0;
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            if is_ego_payload(payload) {
                bytes += payload.len() as u64;
                frames += 1;
                legacy_activation_frames += usize::from(payload.starts_with(b"CVAEIDA1"));
            }
            Ok(())
        })
        .unwrap();
    (bytes, frames, legacy_activation_frames)
}

fn assert_cuts_equal(before: &EgoHistory, after: &EgoHistory, head: u64) {
    for cut in 0..=head {
        assert_eq!(
            before.at(cut).unwrap().fingerprint,
            after.at(cut).unwrap().fingerprint,
            "Ego cut {cut}"
        );
    }
}

fn assert_owner_clock_parity(kind: FileKind, path: &Path, owner_uuid: [u8; 16]) {
    match kind {
        FileKind::Reliquary => {
            let owner = Cva::open(path).unwrap();
            assert_eq!(owner.owner_uuid(), Some(owner_uuid));
            assert_eq!(owner.latest_global_version(), 2);
            assert_eq!(owner.transaction_time_ns(1), Some(40_101));
            assert_eq!(owner.transaction_time_ns(2), Some(40_202));
        }
        FileKind::Phylactery => {
            let owner = Phylactery::open(path).unwrap();
            assert_eq!(owner.owner_uuid(), Some(owner_uuid));
            assert_eq!(owner.latest_global_version(), 2);
            assert_eq!(owner.transaction_time_ns(1), Some(40_101));
            assert_eq!(owner.transaction_time_ns(2), Some(40_202));
        }
    }
}

#[test]
fn ego_upgrade_preserves_all_legacy_cuts_and_owner_clocks_for_rel_and_phy() {
    let mut measurements = Vec::new();
    for (kind, records) in [
        (FileKind::Reliquary, rel_records()),
        (FileKind::Phylactery, phy_records()),
    ] {
        let fixture = Fixture::new();
        let source = fixture.0.join(if kind == FileKind::Reliquary {
            "source.rel"
        } else {
            "source.phy"
        });
        let destination = fixture.0.join("not-promoted");
        create_v1_fixture(&source, kind, &records);
        let original_bytes = fs::read(&source).unwrap();
        let original_hash = Sha256::digest(&original_bytes);
        let original_modified = fs::metadata(&source).unwrap().modified().unwrap();
        let before_measure = ego_payload_measurement(&source, true);

        let (source_history, owner_uuid, global_head, ego_head) = match kind {
            FileKind::Reliquary => {
                let owner = Cva::open_v1_read_only(&source).unwrap();
                (
                    EgoHistory::read(&owner.container, &owner.ego).unwrap(),
                    owner.owner_uuid().unwrap(),
                    owner.latest_global_version(),
                    owner.ego_version(),
                )
            }
            FileKind::Phylactery => {
                let owner = Phylactery::open_v1_read_only(&source).unwrap();
                (
                    EgoHistory::read(&owner.container, &owner.ego).unwrap(),
                    owner.owner_uuid().unwrap(),
                    owner.latest_global_version(),
                    owner.ego_version(),
                )
            }
        };
        assert_eq!(ego_head as usize, records.len());
        assert_eq!(global_head, 2);
        let plan = plan_storage_upgrade(StorageUpgradeRequest {
            source: source.clone(),
            destination: destination.clone(),
        })
        .unwrap();
        let staged = stage_ego_storage_upgrade(&plan).unwrap();
        let staged_path = staged.test_path();
        let after_measure = ego_payload_measurement(staged_path, false);
        let legacy_activation_carriers = after_measure.2;
        if kind == FileKind::Phylactery {
            assert_eq!(
                legacy_activation_carriers,
                records
                    .iter()
                    .filter(|record| matches!(record, EgoRecord::IdentityActive { .. }))
                    .count(),
                "independent IdentityActive remains represented"
            );
        }
        match kind {
            FileKind::Reliquary => {
                let owner = Cva::open(staged_path).unwrap();
                assert_eq!(owner.ego_version(), ego_head);
                assert_eq!(owner.owner_uuid(), Some(owner_uuid));
                assert_eq!(owner.latest_global_version(), global_head);
                let history = EgoHistory::read(&owner.container, &owner.ego).unwrap();
                assert_cuts_equal(&source_history, &history, ego_head);
            }
            FileKind::Phylactery => {
                let owner = Phylactery::open(staged_path).unwrap();
                assert_eq!(owner.ego_version(), ego_head);
                assert_eq!(owner.owner_uuid(), Some(owner_uuid));
                assert_eq!(owner.latest_global_version(), global_head);
                let history = EgoHistory::read(&owner.container, &owner.ego).unwrap();
                assert_cuts_equal(&source_history, &history, ego_head);
            }
        }
        assert_owner_clock_parity(kind, staged_path, owner_uuid);
        assert_eq!(fs::read(&source).unwrap(), original_bytes);
        assert_eq!(Sha256::digest(fs::read(&source).unwrap()), original_hash);
        assert_eq!(
            fs::metadata(&source).unwrap().modified().unwrap(),
            original_modified
        );
        assert!(!destination.exists());
        measurements.push(json!({
            "owner": format!("{kind:?}"),
            "legacy_raw_ego_payload_bytes": before_measure.0,
            "compact_ego_payload_bytes_including_legacy_activation_carrier": after_measure.0,
            "legacy_framed_ego_bytes": before_measure.0 + 8 * before_measure.1 as u64,
            "compact_framed_ego_bytes": after_measure.0 + 8 * after_measure.1 as u64,
            "legacy_ego_frame_count": before_measure.1,
            "compact_ego_frame_count": after_measure.1,
            "retained_legacy_identity_activation_carriers": legacy_activation_carriers,
            "bytes_saved": i128::from(before_measure.0) - i128::from(after_measure.0),
        }));
        drop(staged);
        assert!(!destination.exists());
    }
    let artifact = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join("ego-upgrade-size.json");
    fs::create_dir_all(artifact.parent().unwrap()).unwrap();
    fs::write(
        &artifact,
        serde_json::to_vec_pretty(&json!({
            "format": "raw Ego payload lengths and separately framed bytes; no compression",
            "measurements": measurements,
        }))
        .unwrap(),
    )
    .unwrap();
}

fn invalid_records(case: &str) -> (FileKind, Vec<EgoRecord>) {
    match case {
        "sequence" => (
            FileKind::Reliquary,
            vec![EgoRecord::Anchor {
                ego_version: 2,
                id: EgoAnchorId([0x91; 16]),
                revision: 1,
                deleted: false,
                priority: EgoAnchorPriority::Normal,
                text: "sequence gap".into(),
            }],
        ),
        "terminal-resurrection" => {
            let id = EgoAnchorId([0x92; 16]);
            (
                FileKind::Reliquary,
                vec![
                    EgoRecord::Anchor {
                        ego_version: 1,
                        id,
                        revision: 1,
                        deleted: false,
                        priority: EgoAnchorPriority::Low,
                        text: "live".into(),
                    },
                    EgoRecord::Anchor {
                        ego_version: 2,
                        id,
                        revision: 2,
                        deleted: true,
                        priority: EgoAnchorPriority::High,
                        text: String::new(),
                    },
                    EgoRecord::Anchor {
                        ego_version: 3,
                        id,
                        revision: 3,
                        deleted: false,
                        priority: EgoAnchorPriority::High,
                        text: "resurrect".into(),
                    },
                ],
            )
        }
        "wrong-owner" => (
            FileKind::Reliquary,
            vec![EgoRecord::Identity {
                ego_version: 1,
                id: EgoIdentityId([0x93; 16]),
                revision: 1,
                deleted: false,
                activate: true,
                name: "Default".into(),
                text: "PHY only".into(),
            }],
        ),
        "missing-revision" => {
            let id = EgoAnchorId([0x94; 16]);
            (
                FileKind::Reliquary,
                vec![
                    EgoRecord::Anchor {
                        ego_version: 1,
                        id,
                        revision: 1,
                        deleted: false,
                        priority: EgoAnchorPriority::Low,
                        text: "first".into(),
                    },
                    EgoRecord::Anchor {
                        ego_version: 2,
                        id,
                        revision: 3,
                        deleted: false,
                        priority: EgoAnchorPriority::High,
                        text: "skipped revision".into(),
                    },
                ],
            )
        }
        _ => unreachable!(),
    }
}

#[test]
fn invalid_ego_upgrade_sequences_leave_sources_and_staging_clean() {
    let fixture = Fixture::new();
    for case in [
        "sequence",
        "terminal-resurrection",
        "wrong-owner",
        "missing-revision",
    ] {
        let case_dir = fixture.0.join(case);
        fs::create_dir_all(&case_dir).unwrap();
        let (kind, records) = invalid_records(case);
        let source = case_dir.join("source.rel");
        let destination = case_dir.join("must-not-promote");
        create_v1_fixture(&source, kind, &records);
        let source_bytes = fs::read(&source).unwrap();
        let source_mtime = fs::metadata(&source).unwrap().modified().unwrap();
        let plan = plan_storage_upgrade(StorageUpgradeRequest {
            source: source.clone(),
            destination: destination.clone(),
        })
        .unwrap();
        assert!(
            stage_ego_storage_upgrade(&plan).is_err(),
            "case {case} should fail closed"
        );
        assert_eq!(
            fs::read(&source).unwrap(),
            source_bytes,
            "case {case} modified source"
        );
        assert_eq!(
            fs::metadata(&source).unwrap().modified().unwrap(),
            source_mtime
        );
        assert!(!destination.exists());
        assert!(
            fs::read_dir(&case_dir).unwrap().all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".storage-upgrade-")
            }),
            "case {case} leaked staged output"
        );
    }
}
