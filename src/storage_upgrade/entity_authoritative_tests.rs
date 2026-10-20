#![cfg(feature = "history-oracle-fixtures")]

use super::{StorageUpgradeRequest, plan_storage_upgrade, stage_entity_storage_upgrade};
use crate::entity_codec::{decode_direct_tombstone, decode_tombstone, decode_version};
use crate::entity_model::EntityRecord;
use crate::entity_store::EntityStore;
use crate::{Container, ContainerError, Cva, FileKind, Phylactery};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const ACCEPTED_HASHES: &str = "benchmarks/authoritative-history-rewrite-step62-2026-10-18.json";

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "entity-authoritative-upgrade-{}",
                uuid::Uuid::new_v4()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn destination(&self, fixture: &str) -> PathBuf {
        self.0.join(format!("{fixture}.not-published"))
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Default, serde::Serialize)]
struct EntityPayloadBytes {
    legacy_records: u64,
    legacy_record_count: u64,
    compact_creations: u64,
    compact_creation_count: u64,
    compact_deltas: u64,
    compact_delta_count: u64,
    proof_frames: u64,
    entity_version_frames: u64,
    total_payload_bytes: u64,
    total_file_bytes: u64,
}

fn payload_bytes(path: &Path, v1: bool) -> EntityPayloadBytes {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut result = EntityPayloadBytes {
        total_file_bytes: fs::metadata(path).unwrap().len(),
        ..EntityPayloadBytes::default()
    };
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            result.total_payload_bytes += payload.len() as u64;
            if payload.starts_with(b"CVAENTR") {
                result.legacy_records += payload.len() as u64;
                result.legacy_record_count += 1;
            } else if payload.starts_with(b"CVAENC01") {
                result.compact_creations += payload.len() as u64;
                result.compact_creation_count += 1;
            } else if payload.starts_with(b"CVAEND01") {
                result.compact_deltas += payload.len() as u64;
                result.compact_delta_count += 1;
            } else if payload.starts_with(b"CVAHPR01") {
                result.proof_frames += payload.len() as u64;
            } else if payload.starts_with(b"CVAENTV1") {
                result.entity_version_frames += payload.len() as u64;
            }
            Ok(())
        })
        .unwrap();
    result
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn assert_source_unchanged(
    path: &Path,
    original_bytes: &[u8],
    original_sha256: &str,
    original_modified: std::time::SystemTime,
) {
    let after = fs::read(path).unwrap();
    let metadata = fs::metadata(path).unwrap();
    assert_eq!(
        after.len(),
        original_bytes.len(),
        "source file size changed"
    );
    assert_eq!(sha256(&after), original_sha256, "source SHA-256 changed");
    assert_eq!(after, original_bytes, "source bytes changed");
    assert_eq!(
        metadata.modified().unwrap(),
        original_modified,
        "source mtime changed"
    );
}

#[derive(Default)]
struct TerminalCuts {
    by_id: HashMap<crate::EntityId, u64>,
}

fn terminal_cuts(container: &mut Container) -> TerminalCuts {
    let mut result = TerminalCuts::default();
    for object in container.chunks().unwrap() {
        let payload = container.read(object).unwrap();
        let Some(version) = decode_version(&payload).unwrap() else {
            continue;
        };
        let carrier = container.read(version.record).unwrap();
        if let Some(tombstone) = decode_direct_tombstone(&carrier).unwrap() {
            result.by_id.insert(tombstone.id, version.entity_version);
        } else if let Some(tombstone) = decode_tombstone(&carrier).unwrap() {
            result.by_id.insert(tombstone.id, version.entity_version);
        }
    }
    result
}

fn assert_entity_history_equal(
    source_container: &mut Container,
    source_store: &EntityStore,
    source_records: &[EntityRecord],
    source_terminal: &TerminalCuts,
    staged_container: &mut Container,
    staged_store: &EntityStore,
    staged_records: &[EntityRecord],
    staged_terminal: &TerminalCuts,
    source_entity_version: u64,
) {
    assert_eq!(source_records, staged_records, "retained Entity record set");
    assert_eq!(
        source_terminal.by_id, staged_terminal.by_id,
        "terminal Entity map"
    );
    for cut in 0..=source_entity_version {
        let source_at =
            crate::historical_entity_fingerprint::entity_cut(source_container, source_store, cut)
                .unwrap_or_else(|error| panic!("source Entity cut {cut}: {error}"));
        let staged_at =
            crate::historical_entity_fingerprint::entity_cut(staged_container, staged_store, cut)
                .unwrap_or_else(|error| panic!("staged Entity cut {cut}: {error}"));
        assert_eq!(
            source_at.fingerprint, staged_at.fingerprint,
            "historical Entity fingerprint at owner-local cut {cut}"
        );
        assert_eq!(
            source_at.current, staged_at.current,
            "current Entity revisions at owner-local cut {cut}"
        );
        assert_eq!(
            source_at.replacements, staged_at.replacements,
            "terminal Entity replacements at owner-local cut {cut}"
        );
    }
    for global in 0..=source_container.latest_version() {
        assert_eq!(
            source_container.transaction_time_ns(global),
            staged_container.transaction_time_ns(global),
            "global transaction time at ticket {global}"
        );
    }
}

fn compare_rel(source_path: &Path, staged_path: &Path) {
    let mut source = Cva::open_v1_read_only(source_path).unwrap();
    let mut staged = Cva::open(staged_path).unwrap();
    assert_eq!(source.owner_uuid(), staged.owner_uuid());
    assert_eq!(source.container.identity(), staged.container.identity());
    assert_eq!(
        source.latest_global_version(),
        staged.latest_global_version()
    );
    assert_eq!(source.entity_version(), staged.entity_version());
    let source_records = source.entities.records().to_vec();
    let staged_records = staged.entities.records().to_vec();
    let source_entity_version = source.entity_version();
    let source_terminal = terminal_cuts(&mut source.container);
    let staged_terminal = terminal_cuts(&mut staged.container);
    assert_entity_history_equal(
        &mut source.container,
        &source.entities,
        &source_records,
        &source_terminal,
        &mut staged.container,
        &staged.entities,
        &staged_records,
        &staged_terminal,
        source_entity_version,
    );
    for id in source_terminal.by_id.keys().copied() {
        assert_eq!(
            source.entities.retention_descriptor(id).unwrap(),
            staged.entities.retention_descriptor(id).unwrap(),
            "REL Entity retention descriptor for {:?}",
            id
        );
    }
}

fn compare_phy(source_path: &Path, staged_path: &Path) {
    let mut source = Phylactery::open_v1_read_only(source_path).unwrap();
    let mut staged = Phylactery::open(staged_path).unwrap();
    assert_eq!(source.owner_uuid(), staged.owner_uuid());
    assert_eq!(source.container.identity(), staged.container.identity());
    assert_eq!(
        source.latest_global_version(),
        staged.latest_global_version()
    );
    assert_eq!(source.entity_version(), staged.entity_version());
    let source_records = source.entities.records().to_vec();
    let staged_records = staged.entities.records().to_vec();
    let source_entity_version = source.entity_version();
    let source_terminal = terminal_cuts(&mut source.container);
    let staged_terminal = terminal_cuts(&mut staged.container);
    assert_entity_history_equal(
        &mut source.container,
        &source.entities,
        &source_records,
        &source_terminal,
        &mut staged.container,
        &staged.entities,
        &staged_records,
        &staged_terminal,
        source_entity_version,
    );
    for id in source_terminal.by_id.keys().copied() {
        assert_eq!(
            source.entities.retention_descriptor(id).unwrap(),
            staged.entities.retention_descriptor(id).unwrap(),
            "PHY Entity retention descriptor for {:?}",
            id
        );
    }
}

#[test]
fn authoritative_v1_entity_histories_migrate_without_semantic_or_source_changes() {
    let root = PathBuf::from(
        std::env::var_os("RELIQUARY_HISTORY_FIXTURES")
            .expect("RELIQUARY_HISTORY_FIXTURES must name accepted fixture root"),
    );
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let accepted: Value =
        serde_json::from_slice(&fs::read(manifest.join(ACCEPTED_HASHES)).unwrap()).unwrap();
    let scratch = ScratchDirectory::new();

    for corpus in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ] {
        for (kind, name, file_kind) in [
            ("rel", "project.prj.rel", FileKind::Reliquary),
            ("phy", "user.phy", FileKind::Phylactery),
        ] {
            let key = format!("{corpus}/{name}");
            let source_path = root.join(&key);
            let original_bytes = fs::read(&source_path).unwrap();
            let original_sha256 = sha256(&original_bytes);
            assert_eq!(
                original_sha256,
                accepted["authoritative_hashes"][&key].as_str().unwrap(),
                "accepted authoritative source hash for {key}"
            );
            let modified = fs::metadata(&source_path).unwrap().modified().unwrap();
            let before = payload_bytes(&source_path, true);
            let destination = scratch.destination(&format!("{corpus}-{kind}"));
            let plan = plan_storage_upgrade(StorageUpgradeRequest {
                source: source_path.clone(),
                destination: destination.clone(),
            })
            .unwrap_or_else(|error| panic!("plan {key}: {error}"));
            let staged = stage_entity_storage_upgrade(&plan)
                .unwrap_or_else(|error| panic!("stage {key}: {error}"));
            let after = payload_bytes(staged.test_path(), false);

            match file_kind {
                FileKind::Reliquary => compare_rel(&source_path, staged.test_path()),
                FileKind::Phylactery => compare_phy(&source_path, staged.test_path()),
            }
            assert_source_unchanged(&source_path, &original_bytes, &original_sha256, modified);
            println!(
                "ENTITY_MIGRATION_MEASUREMENT {}",
                serde_json::json!({
                    "fixture": key,
                    "semantic_mismatches": 0,
                    "before": before,
                    "after": after,
                    "scope": "raw Entity carrier bytes; whole-file bytes include Memory/Entity conversion and import-proof transport; no compression",
                    "source_sha256": original_sha256,
                    "source_unchanged": true,
                    "canonical_corpus_promotion": false,
                    "ordinary_frame_checksum": false,
                    "description_proof_frame_checksum": true
                })
            );

            drop(staged);
            assert!(
                !destination.exists(),
                "staging must not publish destination for {key}"
            );
        }
    }
}
