use super::EgoHistory;
use crate::ego_codec::{self, EgoRecord, compact};
use crate::ego_compact_contract::{EgoCreation, EgoDelta, EgoTerminal};
use crate::{Cva, EgoAnchorId, EgoAnchorPriority, EgoIdentityId, Phylactery};
use std::fs;
use std::path::PathBuf;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("historical-ego-compact-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn identity(
    ego_version: u64,
    id: EgoIdentityId,
    revision: u64,
    deleted: bool,
    activate: bool,
    name: &str,
    text: &str,
) -> EgoRecord {
    EgoRecord::Identity {
        ego_version,
        id,
        revision,
        deleted,
        activate,
        name: name.into(),
        text: text.into(),
    }
}

fn anchor(
    ego_version: u64,
    id: EgoAnchorId,
    revision: u64,
    deleted: bool,
    priority: EgoAnchorPriority,
    text: &str,
) -> EgoRecord {
    EgoRecord::Anchor {
        ego_version,
        id,
        revision,
        deleted,
        priority,
        text: text.into(),
    }
}

fn phy_records() -> Vec<EgoRecord> {
    let first = EgoIdentityId([0x11; 16]);
    let second = EgoIdentityId([0x12; 16]);
    let anchor_id = EgoAnchorId([0x21; 16]);
    vec![
        identity(
            1,
            first,
            1,
            false,
            true,
            "  Default  ",
            "  first identity\n",
        ),
        EgoRecord::Personality {
            ego_version: 2,
            revision: 1,
            source_memory_version: 0,
            text: "  concise  ".into(),
        },
        identity(
            3,
            first,
            2,
            false,
            true,
            "  Default  ",
            "  revised identity\n",
        ),
        identity(4, second, 1, false, false, "  Work  ", "  work identity  "),
        // Activation metadata changes while the document fields remain exact.
        identity(5, second, 2, false, true, "  Work  ", "  work identity  "),
        identity(6, first, 3, true, false, "", ""),
        anchor(
            7,
            anchor_id,
            1,
            false,
            EgoAnchorPriority::High,
            "  personal anchor  ",
        ),
        anchor(
            8,
            anchor_id,
            2,
            false,
            EgoAnchorPriority::Low,
            "  revised anchor  ",
        ),
        // A raw identical postimage still has its own publication/revision.
        anchor(
            9,
            anchor_id,
            3,
            false,
            EgoAnchorPriority::Low,
            "  revised anchor  ",
        ),
        anchor(10, anchor_id, 4, true, EgoAnchorPriority::Low, ""),
        EgoRecord::Personality {
            ego_version: 11,
            revision: 2,
            source_memory_version: 0,
            text: "  concise  ".into(),
        },
    ]
}

fn rel_records() -> Vec<EgoRecord> {
    let anchor_id = EgoAnchorId([0x31; 16]);
    vec![
        anchor(
            1,
            anchor_id,
            1,
            false,
            EgoAnchorPriority::Normal,
            "  shared context  ",
        ),
        anchor(
            2,
            anchor_id,
            2,
            false,
            EgoAnchorPriority::High,
            "  revised context  ",
        ),
        anchor(
            3,
            anchor_id,
            3,
            false,
            EgoAnchorPriority::High,
            "  revised context  ",
        ),
        anchor(4, anchor_id, 4, true, EgoAnchorPriority::High, ""),
        EgoRecord::Synthesis {
            ego_version: 5,
            revision: 1,
            source_memory_version: 0,
            text: "  first synthesis  ".into(),
        },
        // The source watermark decreases, while the document text is unchanged.
        EgoRecord::Synthesis {
            ego_version: 6,
            revision: 2,
            source_memory_version: 0,
            text: "  first synthesis  ".into(),
        },
    ]
}

fn same_document(a: &EgoRecord, b: &EgoRecord) -> bool {
    match (a, b) {
        (EgoRecord::Identity { id: a, .. }, EgoRecord::Identity { id: b, .. }) => a == b,
        (EgoRecord::Anchor { id: a, .. }, EgoRecord::Anchor { id: b, .. }) => a == b,
        (EgoRecord::Personality { .. }, EgoRecord::Personality { .. }) => true,
        (EgoRecord::Synthesis { .. }, EgoRecord::Synthesis { .. }) => true,
        _ => false,
    }
}

fn is_terminal(record: &EgoRecord) -> bool {
    matches!(
        record,
        EgoRecord::Identity { deleted: true, .. } | EgoRecord::Anchor { deleted: true, .. }
    )
}

fn compact_payloads(records: &[EgoRecord]) -> Vec<Vec<u8>> {
    let mut prior: Vec<EgoRecord> = Vec::new();
    records
        .iter()
        .map(|record| {
            let previous = prior
                .iter()
                .find(|candidate| same_document(candidate, record));
            let payload = if is_terminal(record) {
                compact::encode_terminal(
                    &EgoTerminal::new(record.clone()).expect("well-shaped terminal"),
                )
                .unwrap()
            } else if let Some(previous) = previous {
                compact::encode_delta(&EgoDelta::between(previous, record).unwrap()).unwrap()
            } else {
                compact::encode_creation(
                    &EgoCreation::new(record.clone()).expect("well-shaped creation"),
                )
                .unwrap()
            };
            if let Some(previous) = prior
                .iter_mut()
                .find(|candidate| same_document(candidate, record))
            {
                *previous = record.clone();
            } else {
                prior.push(record.clone());
            }
            payload
        })
        .collect()
}

fn write_payloads(path: &PathBuf, compact_frames: bool, records: &[EgoRecord]) {
    let mut owner = crate::Container::open(path).unwrap();
    let payloads = if compact_frames {
        compact_payloads(records)
    } else {
        records
            .iter()
            .map(|record| ego_codec::encode(record).unwrap())
            .collect()
    };
    for payload in payloads {
        owner.append(&payload).unwrap();
    }
    owner.sync().unwrap();
}

fn assert_same_history(legacy: &EgoHistory, compact: &EgoHistory, head: u64) {
    for cut in 0..=head {
        let left = legacy.at(cut).unwrap();
        let right = compact.at(cut).unwrap();
        assert_eq!(
            left.fingerprint, right.fingerprint,
            "fingerprint at cut {cut}"
        );
        assert_eq!(left.identities, right.identities, "identities at cut {cut}");
        assert_eq!(
            left.active_identity, right.active_identity,
            "active identity at cut {cut}"
        );
        assert_eq!(left.anchors, right.anchors, "anchors at cut {cut}");
        assert_eq!(
            left.personality, right.personality,
            "personality at cut {cut}"
        );
        assert_eq!(left.synthesis, right.synthesis, "synthesis at cut {cut}");
    }
}

#[test]
fn legacy_and_compact_reliquary_ego_histories_match_at_every_cut() {
    let scratch = Scratch::new();
    let base_path = scratch.path("base.rel");
    drop(Cva::create(&base_path).unwrap());
    let legacy_path = scratch.path("legacy.rel");
    let compact_path = scratch.path("compact.rel");
    fs::copy(&base_path, &legacy_path).unwrap();
    fs::copy(&base_path, &compact_path).unwrap();

    let records = rel_records();
    write_payloads(&legacy_path, false, &records);
    write_payloads(&compact_path, true, &records);

    let legacy_owner = Cva::open(&legacy_path).unwrap();
    let compact_owner = Cva::open(&compact_path).unwrap();
    let legacy = EgoHistory::read(&legacy_owner.container, &legacy_owner.ego).unwrap();
    let compact = EgoHistory::read(&compact_owner.container, &compact_owner.ego).unwrap();
    assert_same_history(&legacy, &compact, records.len() as u64);
}

#[test]
fn legacy_and_compact_phylactery_ego_histories_match_at_every_cut() {
    let scratch = Scratch::new();
    let base_path = scratch.path("base.phy");
    drop(Phylactery::create(&base_path).unwrap());
    let legacy_path = scratch.path("legacy.phy");
    let compact_path = scratch.path("compact.phy");
    fs::copy(&base_path, &legacy_path).unwrap();
    fs::copy(&base_path, &compact_path).unwrap();

    let records = phy_records();
    write_payloads(&legacy_path, false, &records);
    write_payloads(&compact_path, true, &records);

    let legacy_owner = Phylactery::open(&legacy_path).unwrap();
    let compact_owner = Phylactery::open(&compact_path).unwrap();
    let legacy = EgoHistory::read(&legacy_owner.container, &legacy_owner.ego).unwrap();
    let compact = EgoHistory::read(&compact_owner.container, &compact_owner.ego).unwrap();
    assert_same_history(&legacy, &compact, records.len() as u64);
}
