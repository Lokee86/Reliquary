use super::*;
use crate::archive_codec::{
    ArchiveRecord,
    publication::{self, ArchivePublication},
};
use crate::{Container, Cva, FileKind, ObjectRef, Phylactery};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect()
}

#[derive(Default, serde::Serialize)]
struct Frames {
    count: u64,
    framed_bytes: u64,
}
fn add(map: &mut BTreeMap<String, Frames>, family: String, bytes: usize) {
    let item = map.entry(family).or_default();
    item.count += 1;
    item.framed_bytes += bytes as u64 + 8;
}
fn family(payload: &[u8]) -> String {
    String::from_utf8_lossy(&payload[..8]).into_owned()
}

fn orphan_carriers(
    container: &mut Container,
    accepted: &BTreeSet<ObjectRef>,
) -> BTreeMap<String, usize> {
    let mut result = BTreeMap::new();
    for object in container.chunks().unwrap() {
        let bytes = container.read(object).unwrap();
        if !matches!(
            crate::archive_codec::decode_record(&bytes).unwrap(),
            ArchiveRecord::Other | ArchiveRecord::Content(..)
        ) && !accepted.contains(&object)
        {
            *result.entry(sha(&bytes)).or_default() += 1;
        }
    }
    result
}

fn dependencies(record: &ArchiveRecord, ids: &mut BTreeSet<[u8; 32]>) {
    match record {
        ArchiveRecord::Node(node) => {
            ids.insert(node.content_id.0);
        }
        ArchiveRecord::File(file) => {
            ids.insert(file.content_id.0);
        }
        ArchiveRecord::IngestedTurn(turn) => {
            ids.insert(turn.node.content_id.0);
            ids.extend(turn.attachments.iter().map(|f| f.content_id.0));
        }
        _ => {}
    }
}

fn compare_archive(old: &mut Cva, new: &mut Cva) -> serde_json::Value {
    assert_eq!(old.owner_uuid(), new.owner_uuid());
    assert_eq!(old.archive_version(), new.archive_version());
    assert_eq!(old.latest_global_version(), new.latest_global_version());
    let mut carriers = BTreeMap::new();
    let mut links = Frames::default();
    let mut envelopes = BTreeMap::new();
    let mut accepted = BTreeSet::new();
    let mut content_ids = BTreeSet::new();
    for object in old.container.chunks().unwrap() {
        let payload = old.container.read(object).unwrap();
        if let Some(version) =
            crate::archive_history_codec::decode_record_version(&payload).unwrap()
        {
            links.count += 1;
            links.framed_bytes += payload.len() as u64 + 8;
            if accepted.insert(version.record) {
                let bytes = old.container.read(version.record).unwrap();
                add(&mut carriers, family(&bytes), bytes.len());
            }
        }
    }
    assert_eq!(links.count, old.archive_version());
    for local in 1..=old.archive_version() {
        let a = *old.archive.record_version(local).unwrap();
        let b = *new.archive.record_version(local).unwrap();
        assert_eq!(a.global_version, b.global_version);
        let (record, origin) = old.archive.read_publication(&mut old.container, a).unwrap();
        dependencies(&record, &mut content_ids);
        let expected = if origin.archive_version == local {
            ArchivePublication::Record {
                archive_version: local,
                record,
            }
        } else {
            ArchivePublication::Repeat {
                archive_version: local,
                first_applying_archive_version: origin.archive_version,
            }
        };
        let payload = new.container.read(b.record).unwrap();
        let envelope = crate::container::publication_envelope::decode(&payload)
            .unwrap()
            .expect("Archive envelope at its own reference");
        assert_eq!(envelope.global_version, a.global_version);
        assert_eq!(
            envelope.clock,
            crate::container::publication_envelope::PublicationClock::BindReserved
        );
        assert_eq!(envelope.body, publication::encode(&expected).unwrap());
        add(&mut envelopes, family(envelope.body), payload.len());
        let (_, actual_origin) = new.archive.read_publication(&mut new.container, b).unwrap();
        assert_eq!(
            (origin.archive_version, origin.global_version),
            (actual_origin.archive_version, actual_origin.global_version)
        );
        assert_eq!(
            old.transaction_time_ns(a.global_version),
            new.transaction_time_ns(b.global_version)
        );
    }
    for id in &content_ids {
        let a = old
            .archive
            .content_bytes(&mut old.container, crate::ContentId(*id));
        let b = new
            .archive
            .content_bytes(&mut new.container, crate::ContentId(*id));
        match (a, b) {
            (Ok(a), Ok(b)) => assert_eq!(a, b),
            (
                Err(crate::ArchiveError::MissingContent),
                Err(crate::ArchiveError::MissingContent),
            ) => {}
            (a, b) => panic!("Archive dependency availability differs: {a:?} / {b:?}"),
        }
    }
    assert_eq!(
        old.archive.nodes.iter().collect::<Vec<_>>(),
        new.archive.nodes.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        old.archive.branches.iter().collect::<Vec<_>>(),
        new.archive.branches.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        old.archive
            .conversations
            .iter()
            .map(|v| (&v.conversation_id, v))
            .collect::<BTreeMap<_, _>>(),
        new.archive
            .conversations
            .iter()
            .map(|v| (&v.conversation_id, v))
            .collect::<BTreeMap<_, _>>()
    );
    assert_eq!(
        old.archive.fragments.iter().collect::<Vec<_>>(),
        new.archive.fragments.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        old.archive.episodes.iter().collect::<Vec<_>>(),
        new.archive.episodes.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        old.archive.files.iter().collect::<Vec<_>>(),
        new.archive.files.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        old.archive.file_memory_links.iter().collect::<Vec<_>>(),
        new.archive.file_memory_links.iter().collect::<Vec<_>>()
    );
    assert_eq!(old.archive.import_receipts, new.archive.import_receipts);
    assert_eq!(
        old.archive.conversation_tombstones,
        new.archive.conversation_tombstones
    );
    assert_eq!(old.rel_turn_count(), new.rel_turn_count());
    for global in 0..=old.latest_global_version() {
        assert_eq!(
            old.transaction_time_ns(global),
            new.transaction_time_ns(global)
        );
    }
    let old_orphans = orphan_carriers(&mut old.container, &accepted);
    let new_orphans = orphan_carriers(&mut new.container, &BTreeSet::new());
    assert_eq!(old_orphans, new_orphans);
    for object in new.container.chunks().unwrap() {
        assert!(!new.container.read(object).unwrap().starts_with(b"CVAAREC1"));
    }
    serde_json::json!({"legacy_accepted_carriers": carriers, "legacy_links": links, "native_envelopes": envelopes,
        "inert_orphan_carriers": old_orphans.values().sum::<usize>(), "content_dependencies_checked": content_ids.len(), "archive_head": old.archive_version()})
}

#[test]
fn six_pinned_sources_preserve_archive_history_and_measure_exact_owner_frames() {
    let root = PathBuf::from(
        std::env::var_os("RELIQUARY_HISTORY_FIXTURES").expect("accepted fixture root required"),
    );
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("archive-pinned-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    let baseline: serde_json::Value = serde_json::from_str(include_str!(
        "../../benchmarks/history-fixture-baseline-2026-10-19.json"
    ))
    .unwrap();
    let mut measurements = Vec::new();
    for corpus in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ] {
        for (name, kind) in [
            ("project.prj.rel", FileKind::Reliquary),
            ("user.phy", FileKind::Phylactery),
        ] {
            let key = format!("{corpus}/{name}");
            let source = root.join(&key);
            let original = fs::read(&source).unwrap();
            let digest = sha(&original);
            let modified = fs::metadata(&source).unwrap().modified().unwrap();
            assert_eq!(
                baseline["fixtures"][&key]["sha256"].as_str(),
                Some(digest.as_str()),
                "pinned source {key}"
            );
            let destination = directory.join(format!("{corpus}-{name}"));
            let plan = plan_storage_upgrade(StorageUpgradeRequest {
                source: source.clone(),
                destination: destination.clone(),
            })
            .unwrap();
            let staged = stage_archive_envelope_storage_upgrade(&plan)
                .unwrap_or_else(|e| panic!("{key}: {e}"));
            let frames = if kind == FileKind::Reliquary {
                let mut old = Cva::open_v1_read_only(&source).unwrap();
                let mut new = Cva::open(staged.test_path()).unwrap();
                compare_archive(&mut old, &mut new)
            } else {
                let old = Container::open_v1_read_only(&source).unwrap();
                let mut new = Phylactery::open(staged.test_path()).unwrap();
                assert_eq!(old.owner_uuid(), new.owner_uuid());
                assert_eq!(old.latest_version(), new.latest_global_version());
                let mut archive = 0;
                for object in new.container.chunks().unwrap() {
                    let payload = new.container.read(object).unwrap();
                    if let Some(envelope) =
                        crate::container::publication_envelope::decode(&payload).unwrap()
                    {
                        if publication::decode(envelope.body, envelope.global_version)
                            .unwrap()
                            .is_some()
                        {
                            archive += 1;
                        }
                    }
                }
                assert_eq!(archive, 0);
                for global in 0..=old.latest_version() {
                    assert_eq!(
                        old.transaction_time_ns(global),
                        new.transaction_time_ns(global)
                    );
                }
                serde_json::json!({"archive_head": 0, "native_archive_envelopes": 0})
            };
            assert_eq!(fs::read(&source).unwrap(), original);
            assert_eq!(fs::metadata(&source).unwrap().modified().unwrap(), modified);
            let row = serde_json::json!({"fixture": key, "source_sha256": digest, "source_unchanged": true,
                "semantic_mismatches": 0, "frames": frames, "scope": "Archive owner frames including 8-byte chunk framing; reservations, other owners and grouped payloads excluded", "destination_published": false});
            println!("ARCHIVE_ENVELOPE_MIGRATION_MEASUREMENT {row}");
            measurements.push(row);
            drop(staged);
            assert!(!destination.exists());
        }
    }
    if let Some(path) = std::env::var_os("RELIQUARY_ARCHIVE_MEASUREMENT_OUT") {
        fs::write(
            Path::new(&path),
            serde_json::to_vec_pretty(&measurements).unwrap(),
        )
        .unwrap();
    }
    fs::remove_dir_all(directory).unwrap();
}
