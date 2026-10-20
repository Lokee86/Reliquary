#![cfg(feature = "history-oracle-fixtures")]

use super::shell::stage_relationship_storage_upgrade;
use super::{StorageUpgradeRequest, plan_storage_upgrade};
use crate::relationship_model::RelationshipRecord;
use crate::{Container, ContainerError, Cva, FileKind, Phylactery, RelationshipId};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const ACCEPTED_HASHES: &str = "benchmarks/authoritative-history-rewrite-step62-2026-10-18.json";
const CORPORA: [&str; 3] = [
    "chatgpt-first14d",
    "chatgpt-first28d",
    "rhelm-david-r-ellis-pre-relationship",
];

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "relationship-authoritative-upgrade-{}",
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
struct RelationshipPayloadBytes {
    legacy_records: u64,
    legacy_record_count: u64,
    compact_creations: u64,
    compact_creation_count: u64,
    compact_deltas: u64,
    compact_delta_count: u64,
    terminal_carriers: u64,
    terminal_count: u64,
    relationship_version_frames: u64,
    total_payload_bytes: u64,
    total_file_bytes: u64,
}

fn payload_bytes(path: &Path, v1: bool) -> RelationshipPayloadBytes {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut result = RelationshipPayloadBytes {
        total_file_bytes: fs::metadata(path).unwrap().len(),
        ..RelationshipPayloadBytes::default()
    };
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            result.total_payload_bytes += payload.len() as u64;
            if payload.starts_with(b"CVARELR1") {
                result.legacy_records += payload.len() as u64;
                result.legacy_record_count += 1;
            } else if payload.starts_with(b"CVARNC01") {
                result.compact_creations += payload.len() as u64;
                result.compact_creation_count += 1;
            } else if payload.starts_with(b"CVARND01") {
                result.compact_deltas += payload.len() as u64;
                result.compact_delta_count += 1;
            } else if payload.starts_with(b"CVARELT1") {
                result.terminal_carriers += payload.len() as u64;
                result.terminal_count += 1;
            } else if payload.starts_with(b"CVARELV1") {
                result.relationship_version_frames += payload.len() as u64;
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
    assert_eq!(after, original_bytes, "source bytes changed");
    assert_eq!(after.len(), original_bytes.len(), "source size changed");
    assert_eq!(sha256(&after), original_sha256, "source SHA-256 changed");
    assert_eq!(
        metadata.modified().unwrap(),
        original_modified,
        "source mtime changed"
    );
}

fn terminal_descriptors_rel(
    owner: &Cva,
) -> BTreeMap<RelationshipId, crate::RelationshipRetentionDescriptor> {
    owner
        .relationships
        .records()
        .iter()
        .map(|record| record.id)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|id| {
            matches!(
                owner.relationship_lifecycle_state(*id),
                Ok(crate::owner_lifecycle::LifecycleState::Tombstoned(_))
            )
        })
        .map(|id| (id, owner.relationship_retention_descriptor(id).unwrap()))
        .collect()
}

fn terminal_descriptors_phy(
    owner: &Phylactery,
) -> BTreeMap<RelationshipId, crate::RelationshipRetentionDescriptor> {
    owner
        .relationships
        .records()
        .iter()
        .map(|record| record.id)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|id| {
            matches!(
                owner.relationship_lifecycle_state(*id),
                Ok(crate::owner_lifecycle::LifecycleState::Tombstoned(_))
            )
        })
        .map(|id| (id, owner.relationship_retention_descriptor(id).unwrap()))
        .collect()
}

fn compare_history(
    source_container: &Container,
    source_records: &[RelationshipRecord],
    source_owner: &crate::relationship_store::RelationshipStore,
    staged_container: &Container,
    staged_records: &[RelationshipRecord],
    staged_owner: &crate::relationship_store::RelationshipStore,
) {
    assert_eq!(
        source_records, staged_records,
        "retained Relationship records"
    );
    assert_eq!(
        source_owner.relationship_version(),
        staged_owner.relationship_version(),
        "Relationship owner-local head"
    );
    let source_terminals = source_records
        .iter()
        .map(|record| record.id)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter_map(|id| {
            source_owner
                .retention_descriptor(id)
                .ok()
                .map(|descriptor| (id, descriptor))
        })
        .collect::<BTreeMap<_, _>>();
    let staged_terminals = staged_records
        .iter()
        .map(|record| record.id)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter_map(|id| {
            staged_owner
                .retention_descriptor(id)
                .ok()
                .map(|descriptor| (id, descriptor))
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        source_terminals, staged_terminals,
        "terminal retention descriptors"
    );

    for cut in 0..=source_owner.relationship_version() {
        let source_at = crate::historical_relationship_fingerprint::relationship_cut(
            source_container,
            source_owner,
            cut,
        )
        .unwrap_or_else(|error| panic!("source Relationship cut {cut}: {error}"));
        let staged_at = crate::historical_relationship_fingerprint::relationship_cut(
            staged_container,
            staged_owner,
            cut,
        )
        .unwrap_or_else(|error| panic!("staged Relationship cut {cut}: {error}"));
        assert_eq!(
            source_at.fingerprint, staged_at.fingerprint,
            "historical Relationship fingerprint at owner-local cut {cut}"
        );
        assert_eq!(
            source_at.current, staged_at.current,
            "current Relationship revisions at owner-local cut {cut}"
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
    let source = Cva::open_v1_read_only(source_path).unwrap();
    let staged = Cva::open(staged_path).unwrap();
    assert_eq!(source.owner_uuid(), staged.owner_uuid());
    assert_eq!(source.container.identity(), staged.container.identity());
    assert_eq!(
        source.latest_global_version(),
        staged.latest_global_version()
    );
    assert_eq!(source.relationship_version(), staged.relationship_version());
    let source_records = source.relationships.records().to_vec();
    let staged_records = staged.relationships.records().to_vec();
    assert_eq!(
        terminal_descriptors_rel(&source),
        terminal_descriptors_rel(&staged),
        "REL Relationship terminal retention descriptors"
    );
    compare_history(
        &source.container,
        &source_records,
        &source.relationships,
        &staged.container,
        &staged_records,
        &staged.relationships,
    );
}

fn compare_phy(source_path: &Path, staged_path: &Path) {
    let source = Phylactery::open_v1_read_only(source_path).unwrap();
    let staged = Phylactery::open(staged_path).unwrap();
    assert_eq!(source.owner_uuid(), staged.owner_uuid());
    assert_eq!(source.container.identity(), staged.container.identity());
    assert_eq!(
        source.latest_global_version(),
        staged.latest_global_version()
    );
    assert_eq!(source.relationship_version(), staged.relationship_version());
    let source_records = source.relationships.records().to_vec();
    let staged_records = staged.relationships.records().to_vec();
    assert_eq!(
        terminal_descriptors_phy(&source),
        terminal_descriptors_phy(&staged),
        "PHY Relationship terminal retention descriptors"
    );
    compare_history(
        &source.container,
        &source_records,
        &source.relationships,
        &staged.container,
        &staged_records,
        &staged.relationships,
    );
}

#[test]
fn authoritative_v1_relationship_histories_migrate_without_semantic_or_source_changes() {
    let root = PathBuf::from(
        std::env::var_os("RELIQUARY_HISTORY_FIXTURES")
            .expect("RELIQUARY_HISTORY_FIXTURES must name accepted fixture root"),
    );
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let accepted: Value =
        serde_json::from_slice(&fs::read(manifest.join(ACCEPTED_HASHES)).unwrap()).unwrap();
    let scratch = ScratchDirectory::new();
    let mut measurements = Vec::new();
    let mut zero_relationship_cases = 0_u64;
    let mut raw_carrier_bytes_before_total = 0_u64;
    let mut raw_carrier_bytes_after_total = 0_u64;

    for corpus in CORPORA {
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
            let staged = stage_relationship_storage_upgrade(&plan)
                .unwrap_or_else(|error| panic!("stage {key}: {error}"));
            let after = payload_bytes(staged.test_path(), false);
            let staged_sha256 = sha256(&fs::read(staged.test_path()).unwrap());

            match file_kind {
                FileKind::Reliquary => compare_rel(&source_path, staged.test_path()),
                FileKind::Phylactery => compare_phy(&source_path, staged.test_path()),
            }
            assert_source_unchanged(&source_path, &original_bytes, &original_sha256, modified);
            let source_relationship_count = before.legacy_record_count;
            let staged_relationship_count =
                after.compact_creation_count + after.compact_delta_count;
            assert_eq!(source_relationship_count, staged_relationship_count);
            let raw_before = before.legacy_records + before.terminal_carriers;
            let raw_after =
                after.compact_creations + after.compact_deltas + after.terminal_carriers;
            zero_relationship_cases += u64::from(source_relationship_count == 0);
            raw_carrier_bytes_before_total += raw_before;
            raw_carrier_bytes_after_total += raw_after;
            measurements.push(serde_json::json!({
                "fixture": key,
                "owner_kind": kind,
                "source_sha256": original_sha256,
                "staged_sha256": staged_sha256,
                "source_unchanged": true,
                "source_size_bytes": original_bytes.len(),
                "relationship_revision_count": source_relationship_count,
                "terminal_count_before": before.terminal_count,
                "terminal_count_after": after.terminal_count,
                "before": before,
                "after": after,
                "raw_relationship_carrier_bytes_before": raw_before,
                "raw_relationship_carrier_bytes_after": raw_after,
                "raw_relationship_carrier_bytes_delta": raw_after as i64 - raw_before as i64,
                "canonical_corpus_promotion": false,
                "semantic_mismatches": 0
            }));

            drop(staged);
            assert!(!destination.exists(), "staging must not publish {key}");
        }
    }

    println!(
        "RELATIONSHIP_MIGRATION_MEASUREMENT {}",
        serde_json::json!({
            "schema": 1,
            "step": 124,
            "date": "2026-10-20",
            "fixture_root": root,
            "authoritative_sources_unchanged": true,
            "records": measurements,
            "zero_relationship_cases": zero_relationship_cases,
            "raw_relationship_carrier_bytes_before_total": raw_carrier_bytes_before_total,
            "raw_relationship_carrier_bytes_after_total": raw_carrier_bytes_after_total,
            "raw_relationship_carrier_bytes_delta_total": raw_carrier_bytes_after_total as i64 - raw_carrier_bytes_before_total as i64,
            "scope": "raw Relationship record/creation/delta/terminal carriers; no compression; six frozen REL and PHY sources"
        })
    );
}
