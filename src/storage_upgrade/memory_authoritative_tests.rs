#![cfg(feature = "history-oracle-fixtures")]

use super::{StorageUpgradeRequest, plan_storage_upgrade, stage_memory_storage_upgrade};
use crate::historical_memory_fingerprint::MemoryHistory;
use crate::{Container, ContainerError, Cva, FileKind, Phylactery};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const AUTHORITATIVE_BASELINE: &str =
    "benchmarks/authoritative-history-rewrite-step62-2026-10-18.json";

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "memory-authoritative-upgrade-{}",
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
struct PayloadBytes {
    legacy_memory_records: u64,
    compact_creations: u64,
    compact_deltas: u64,
    memory_bodies: u64,
    grouped_completion_payloads: u64,
    external_publication_payloads: u64,
    memory_publication_metadata: u64,
    memory_routing_metadata: u64,
    other_payloads: u64,
    total_payloads: u64,
    total_file: u64,
}

fn payload_bytes(path: &Path, v1: bool) -> PayloadBytes {
    let total_file = fs::metadata(path).unwrap().len();
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut result = PayloadBytes {
        total_file,
        ..PayloadBytes::default()
    };
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            let len = payload.len() as u64;
            result.total_payloads += len;
            if payload.starts_with(b"CVAMEMR") {
                result.legacy_memory_records += len;
            } else if payload.starts_with(b"CVAMC001") {
                result.compact_creations += len;
            } else if payload.starts_with(b"CVAMD001") {
                result.compact_deltas += len;
            } else if payload.starts_with(b"CVAMBDY1") {
                result.memory_bodies += len;
            } else if payload.starts_with(b"CVAINSC") {
                result.grouped_completion_payloads += len;
            } else if payload.starts_with(b"CVAXMEM") {
                result.external_publication_payloads += len;
            } else if payload.starts_with(b"CVAMEM") {
                result.memory_publication_metadata += len;
            } else if payload.starts_with(b"CVAMRTE") {
                result.memory_routing_metadata += len;
            } else {
                result.other_payloads += len;
            }
            Ok(())
        })
        .unwrap();
    result
}

fn print_measurement(fixture: &str, before: &PayloadBytes, after: &PayloadBytes) {
    println!(
        "MEMORY_MIGRATION_MEASUREMENT {}",
        serde_json::json!({
            "fixture": fixture, "semantic_mismatches": 0,
            "before": before, "after": after,
            "scope": "raw payloads; grouped/external frames include their atomic bodies, routing and receipts; no compression"
        })
    );
}

fn assert_memory_history_equal(
    source: &mut Container,
    source_store: &crate::memory_store::MemoryStore,
    migrated: &mut Container,
    migrated_store: &crate::memory_store::MemoryStore,
) {
    assert_eq!(source_store.records(), migrated_store.records());
    assert_eq!(source_store.current_ids(), migrated_store.current_ids());
    assert_eq!(
        source_store.tombstoned_ids(),
        migrated_store.tombstoned_ids()
    );
    assert_eq!(
        source_store.memory_version(),
        migrated_store.memory_version()
    );

    let source_history = MemoryHistory::read(source, source_store).unwrap();
    let migrated_history = MemoryHistory::read(migrated, migrated_store).unwrap();
    let head = source_store.memory_version();
    for cut in 0..=head {
        let source_at = source_history.at(cut).unwrap();
        let migrated_at = migrated_history.at(cut).unwrap();
        assert_eq!(
            source_at.fingerprint, migrated_at.fingerprint,
            "Memory fingerprint at owner-local cut {cut}"
        );
        assert_eq!(
            source_at.current, migrated_at.current,
            "current Memory membership at owner-local cut {cut}"
        );
    }
    for record in source_store.records() {
        assert_eq!(
            source_history.revision(record.id, record.revision).unwrap(),
            migrated_history
                .revision(record.id, record.revision)
                .unwrap(),
            "Memory body/routing/revision fingerprint for {} revision {}",
            hex(&record.id.0),
            record.revision
        );
    }

    for global in 0..=source.latest_version() {
        assert_eq!(
            source.transaction_time_ns(global),
            migrated.transaction_time_ns(global),
            "global transaction time at version {global}"
        );
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn assert_source_unchanged(
    path: &Path,
    bytes: &[u8],
    digest: &str,
    modified: std::time::SystemTime,
) {
    let after = fs::read(path).unwrap();
    let metadata = fs::metadata(path).unwrap();
    assert_eq!(
        after.len(),
        bytes.len(),
        "authoritative source size changed"
    );
    assert_eq!(sha256(&after), digest, "authoritative source hash changed");
    assert_eq!(after, bytes, "authoritative source bytes changed");
    assert_eq!(
        metadata.modified().unwrap(),
        modified,
        "source mtime changed"
    );
}

fn compare_rel(source_path: &Path, migrated_path: &Path) {
    let mut source = Cva::open_v1_read_only(source_path).unwrap();
    let mut migrated = Cva::open(migrated_path).unwrap();
    assert_eq!(source.owner_uuid(), migrated.owner_uuid());
    assert_eq!(source.container.identity(), migrated.container.identity());
    assert_eq!(
        source.latest_global_version(),
        migrated.latest_global_version()
    );
    assert_eq!(
        source.legacy_import_epochs(),
        migrated.legacy_import_epochs()
    );
    assert_eq!(
        source.import_epoch_statuses(),
        migrated.import_epoch_statuses()
    );
    let mut freshness_cuts = std::collections::BTreeSet::from([0, source.rel_turn_count()]);
    for settlement in source.freshness.import_settlements() {
        freshness_cuts.insert(settlement.base_cut);
        freshness_cuts.insert(settlement.settlement_cut);
    }
    for cut in freshness_cuts {
        assert_eq!(
            crate::historical_freshness_fingerprint::freshness_at_turn(&source, cut)
                .unwrap()
                .fingerprint,
            crate::historical_freshness_fingerprint::freshness_at_turn(&migrated, cut)
                .unwrap()
                .fingerprint,
            "canonical Freshness settlement/history proof at activity cut {cut}"
        );
    }
    assert_memory_history_equal(
        &mut source.container,
        &source.memories,
        &mut migrated.container,
        &migrated.memories,
    );
    for id in source.memories.tombstoned_ids() {
        assert_eq!(
            source.memory_retention_descriptor(id).unwrap(),
            migrated.memory_retention_descriptor(id).unwrap(),
            "REL tombstone retention descriptor for {}",
            hex(&id.0)
        );
    }
}

fn compare_phy(source_path: &Path, migrated_path: &Path) {
    let mut source = Phylactery::open_v1_read_only(source_path).unwrap();
    let mut migrated = Phylactery::open(migrated_path).unwrap();
    assert_eq!(source.owner_uuid(), migrated.owner_uuid());
    assert_eq!(source.container.identity(), migrated.container.identity());
    assert_eq!(
        source.latest_global_version(),
        migrated.latest_global_version()
    );
    assert_memory_history_equal(
        &mut source.container,
        &source.memories,
        &mut migrated.container,
        &migrated.memories,
    );
    for id in source.memories.tombstoned_ids() {
        assert_eq!(
            source.memory_retention_descriptor(id).unwrap(),
            migrated.memory_retention_descriptor(id).unwrap(),
            "PHY tombstone retention descriptor for {}",
            hex(&id.0)
        );
    }
}

#[test]
fn authoritative_v1_memory_histories_migrate_without_semantic_or_source_changes() {
    let root = PathBuf::from(
        std::env::var_os("RELIQUARY_HISTORY_FIXTURES")
            .expect("RELIQUARY_HISTORY_FIXTURES must name the accepted fixture root"),
    );
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let accepted: Value = serde_json::from_slice(
        &fs::read(manifest.join(AUTHORITATIVE_BASELINE)).expect("accepted source hashes"),
    )
    .unwrap();
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
            let source_path = root.join(corpus).join(name);
            let source_bytes = fs::read(&source_path).unwrap();
            let source_digest = sha256(&source_bytes);
            let expected = accepted["authoritative_hashes"][&key]
                .as_str()
                .unwrap_or_else(|| panic!("missing accepted fixture hash for {key}"));
            assert_eq!(
                source_digest, expected,
                "authoritative fixture hash for {key}"
            );
            let source_modified = fs::metadata(&source_path).unwrap().modified().unwrap();
            let before = payload_bytes(&source_path, true);
            let destination_path = scratch.destination(&format!("{corpus}-{kind}"));
            let plan = plan_storage_upgrade(StorageUpgradeRequest {
                source: source_path.clone(),
                destination: destination_path.clone(),
            })
            .unwrap_or_else(|error| panic!("plan {key}: {error}"));
            let staged = stage_memory_storage_upgrade(&plan)
                .unwrap_or_else(|error| panic!("stage {key}: {error}"));
            let after = payload_bytes(staged.test_path(), false);

            match file_kind {
                FileKind::Reliquary => compare_rel(&source_path, staged.test_path()),
                FileKind::Phylactery => compare_phy(&source_path, staged.test_path()),
            }
            assert_source_unchanged(&source_path, &source_bytes, &source_digest, source_modified);
            print_measurement(&key, &before, &after);

            drop(staged);
            assert!(
                !destination_path.exists(),
                "staging must not publish destination for {key}"
            );
        }
    }
}

#[test]
fn authoritative_memory_staging_matches_accepted_measurements_without_untranslated_proofs() {
    let root = PathBuf::from(std::env::var_os("RELIQUARY_HISTORY_FIXTURES").unwrap());
    let report: Value = serde_json::from_slice(
        &fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("benchmarks/lossless-memory-migration-step87-2026-10-19.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let scratch = ScratchDirectory::new();
    let fixtures = report["fixtures"].as_array().unwrap();
    assert_eq!(fixtures.len(), 6);
    for row in fixtures {
        let fixture = row["fixture"].as_str().unwrap();
        let source_path = root.join(fixture);
        let source_bytes = fs::read(&source_path).unwrap();
        let source_digest = sha256(&source_bytes);
        assert_eq!(
            source_digest,
            row["accepted_source_sha256"].as_str().unwrap()
        );
        let modified = fs::metadata(&source_path).unwrap().modified().unwrap();
        let mut source = Container::open_v1_read_only(&source_path).unwrap();
        let identity = source.identity().unwrap();
        let owner = source.owner_uuid();
        let head = source.latest_version();
        source
            .visit_payloads::<ContainerError>(|_, payload| {
                assert!(
                    !payload.starts_with(b"CVAHPR01"),
                    "{fixture} needs a proof converter"
                );
                Ok(())
            })
            .unwrap();
        drop(source);
        let plan = plan_storage_upgrade(StorageUpgradeRequest {
            source: source_path.clone(),
            destination: scratch.destination(&fixture.replace('/', "-")),
        })
        .unwrap();
        let staged = stage_memory_storage_upgrade(&plan).unwrap();
        assert_eq!(
            serde_json::to_value(payload_bytes(staged.test_path(), false)).unwrap(),
            row["after"]
        );
        match identity.file_kind {
            FileKind::Reliquary => {
                let reopened = Cva::open(staged.test_path()).unwrap();
                assert_eq!(reopened.owner_uuid(), owner);
                assert_eq!(reopened.latest_global_version(), head);
            }
            FileKind::Phylactery => {
                let reopened = Phylactery::open(staged.test_path()).unwrap();
                assert_eq!(reopened.owner_uuid(), owner);
                assert_eq!(reopened.latest_global_version(), head);
            }
        }
        assert_source_unchanged(&source_path, &source_bytes, &source_digest, modified);
    }
}
