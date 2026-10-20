#![cfg(feature = "history-oracle-fixtures")]

use super::shell::stage_ego_storage_upgrade;
use super::{StorageUpgradeRequest, plan_storage_upgrade};
use crate::{Container, ContainerError, Cva, FileKind, Phylactery};
use serde_json::Value;
use sha2::{Digest, Sha256};
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
                "ego-authoritative-upgrade-{}",
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
struct EgoPayloadBytes {
    legacy_identity_records: u64,
    legacy_identity_record_count: u64,
    legacy_identity_active_records: u64,
    legacy_identity_active_count: u64,
    legacy_personality_records: u64,
    legacy_personality_count: u64,
    legacy_anchor_records: u64,
    legacy_anchor_count: u64,
    legacy_synthesis_records: u64,
    legacy_synthesis_count: u64,
    compact_creations: u64,
    compact_creation_count: u64,
    compact_deltas: u64,
    compact_delta_count: u64,
    compact_terminals: u64,
    compact_terminal_count: u64,
    total_payload_bytes: u64,
    total_file_bytes: u64,
    raw_ego_payload_bytes: u64,
    framed_ego_bytes: u64,
    ego_publication_count: u64,
}

impl EgoPayloadBytes {
    fn add_ego_frame(&mut self, bytes: usize) {
        self.raw_ego_payload_bytes += bytes as u64;
        // The physical Container frame consists of an eight-byte length plus
        // the owner payload. This counts per-Ego-frame bytes, not whole-file overhead.
        self.framed_ego_bytes += bytes as u64 + 8;
        self.ego_publication_count += 1;
    }
}

fn payload_bytes(path: &Path, v1: bool) -> EgoPayloadBytes {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut result = EgoPayloadBytes {
        total_file_bytes: fs::metadata(path).unwrap().len(),
        ..EgoPayloadBytes::default()
    };
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            result.total_payload_bytes += payload.len() as u64;
            if payload.starts_with(b"CVAEIDN1") || payload.starts_with(b"CVAEIDN2") {
                result.add_ego_frame(payload.len());
                result.legacy_identity_records += payload.len() as u64;
                result.legacy_identity_record_count += 1;
            } else if payload.starts_with(b"CVAEIDA1") {
                result.add_ego_frame(payload.len());
                result.legacy_identity_active_records += payload.len() as u64;
                result.legacy_identity_active_count += 1;
            } else if payload.starts_with(b"CVAEPER1") {
                result.add_ego_frame(payload.len());
                result.legacy_personality_records += payload.len() as u64;
                result.legacy_personality_count += 1;
            } else if payload.starts_with(b"CVAEANC1") {
                result.add_ego_frame(payload.len());
                result.legacy_anchor_records += payload.len() as u64;
                result.legacy_anchor_count += 1;
            } else if payload.starts_with(b"CVAESYN1") {
                result.add_ego_frame(payload.len());
                result.legacy_synthesis_records += payload.len() as u64;
                result.legacy_synthesis_count += 1;
            } else if payload.starts_with(b"CVAECR01") {
                result.add_ego_frame(payload.len());
                result.compact_creations += payload.len() as u64;
                result.compact_creation_count += 1;
            } else if payload.starts_with(b"CVAEDL01") {
                result.add_ego_frame(payload.len());
                result.compact_deltas += payload.len() as u64;
                result.compact_delta_count += 1;
            } else if payload.starts_with(b"CVAETM01") {
                result.add_ego_frame(payload.len());
                result.compact_terminals += payload.len() as u64;
                result.compact_terminal_count += 1;
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
    assert_eq!(
        after.len(),
        original_bytes.len(),
        "source file size changed"
    );
    assert_eq!(sha256(&after), original_sha256, "source SHA-256 changed");
    assert_eq!(
        metadata.modified().unwrap(),
        original_modified,
        "source mtime changed"
    );
}

fn assert_ego_history_equal(
    source_container: &Container,
    source_store: &crate::ego_store::EgoStore,
    staged_container: &Container,
    staged_store: &crate::ego_store::EgoStore,
    head: u64,
) {
    let source =
        crate::historical_ego_fingerprint::EgoHistory::read(source_container, source_store)
            .unwrap_or_else(|error| panic!("read source Ego history: {error}"));
    let staged =
        crate::historical_ego_fingerprint::EgoHistory::read(staged_container, staged_store)
            .unwrap_or_else(|error| panic!("read staged Ego history: {error}"));
    for cut in 0..=head {
        let source_at = source
            .at(cut)
            .unwrap_or_else(|error| panic!("source Ego cut {cut}: {error}"));
        let staged_at = staged
            .at(cut)
            .unwrap_or_else(|error| panic!("staged Ego cut {cut}: {error}"));
        assert_eq!(
            source_at.fingerprint, staged_at.fingerprint,
            "Ego fingerprint at cut {cut}"
        );
        assert_eq!(
            source_at.identities, staged_at.identities,
            "Ego identities at cut {cut}"
        );
        assert_eq!(
            source_at.active_identity, staged_at.active_identity,
            "active Ego identity at cut {cut}"
        );
        assert_eq!(
            source_at.anchors, staged_at.anchors,
            "Ego anchors at cut {cut}"
        );
        assert_eq!(
            source_at.personality, staged_at.personality,
            "Ego Personality at cut {cut}"
        );
        assert_eq!(
            source_at.synthesis, staged_at.synthesis,
            "Ego Synthesis at cut {cut}"
        );
    }
}

fn compare_rel(source_path: &Path, staged_path: &Path) -> u64 {
    let source = Cva::open_v1_read_only(source_path).unwrap();
    let staged = Cva::open(staged_path).unwrap();
    assert_eq!(source.owner_uuid(), staged.owner_uuid());
    assert_eq!(source.container.identity(), staged.container.identity());
    assert_eq!(
        source.latest_global_version(),
        staged.latest_global_version()
    );
    assert_eq!(source.ego_version(), staged.ego_version());
    assert_ego_history_equal(
        &source.container,
        &source.ego,
        &staged.container,
        &staged.ego,
        source.ego_version(),
    );
    for global in 0..=source.container.latest_version() {
        assert_eq!(
            source.container.transaction_time_ns(global),
            staged.container.transaction_time_ns(global),
            "REL global transaction time at ticket {global}"
        );
    }
    source.ego_version()
}

fn compare_phy(source_path: &Path, staged_path: &Path) -> u64 {
    let source = Phylactery::open_v1_read_only(source_path).unwrap();
    let staged = Phylactery::open(staged_path).unwrap();
    assert_eq!(source.owner_uuid(), staged.owner_uuid());
    assert_eq!(source.container.identity(), staged.container.identity());
    assert_eq!(
        source.latest_global_version(),
        staged.latest_global_version()
    );
    assert_eq!(source.ego_version(), staged.ego_version());
    assert_ego_history_equal(
        &source.container,
        &source.ego,
        &staged.container,
        &staged.ego,
        source.ego_version(),
    );
    for global in 0..=source.container.latest_version() {
        assert_eq!(
            source.container.transaction_time_ns(global),
            staged.container.transaction_time_ns(global),
            "PHY global transaction time at ticket {global}"
        );
    }
    source.ego_version()
}

#[test]
fn authoritative_v1_ego_histories_stage_with_semantic_and_source_parity() {
    let root = PathBuf::from(
        std::env::var_os("RELIQUARY_HISTORY_FIXTURES")
            .expect("RELIQUARY_HISTORY_FIXTURES must name accepted fixture root"),
    );
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let accepted: Value =
        serde_json::from_slice(&fs::read(manifest.join(ACCEPTED_HASHES)).unwrap()).unwrap();
    let scratch = ScratchDirectory::new();
    let mut measurements = Vec::new();
    let mut zero_ego_cases = 0_u64;
    let mut nonempty_ego_cases = 0_u64;
    let mut raw_ego_payload_bytes_before_total = 0_u64;
    let mut raw_ego_payload_bytes_after_total = 0_u64;
    let mut framed_ego_bytes_before_total = 0_u64;
    let mut framed_ego_bytes_after_total = 0_u64;

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
            let staged = stage_ego_storage_upgrade(&plan)
                .unwrap_or_else(|error| panic!("stage {key}: {error}"));
            let after = payload_bytes(staged.test_path(), false);
            let staged_sha256 = sha256(&fs::read(staged.test_path()).unwrap());

            let ego_head = match file_kind {
                FileKind::Reliquary => compare_rel(&source_path, staged.test_path()),
                FileKind::Phylactery => compare_phy(&source_path, staged.test_path()),
            };
            assert_source_unchanged(&source_path, &original_bytes, &original_sha256, modified);
            assert_eq!(before.ego_publication_count, ego_head);
            assert_eq!(after.ego_publication_count, ego_head);
            zero_ego_cases += u64::from(ego_head == 0);
            nonempty_ego_cases += u64::from(ego_head > 0);
            raw_ego_payload_bytes_before_total += before.raw_ego_payload_bytes;
            raw_ego_payload_bytes_after_total += after.raw_ego_payload_bytes;
            framed_ego_bytes_before_total += before.framed_ego_bytes;
            framed_ego_bytes_after_total += after.framed_ego_bytes;
            measurements.push(serde_json::json!({
                "fixture": key,
                "owner_kind": kind,
                "source_sha256": original_sha256,
                "staged_sha256": staged_sha256,
                "source_unchanged": true,
                "source_size_bytes": original_bytes.len(),
                "ego_publication_count_before": before.ego_publication_count,
                "ego_publication_count_after": after.ego_publication_count,
                "ego_owner_local_head": ego_head,
                "before": before,
                "after": after,
                "raw_ego_payload_bytes_delta": after.raw_ego_payload_bytes as i64 - before.raw_ego_payload_bytes as i64,
                "framed_ego_bytes_delta": after.framed_ego_bytes as i64 - before.framed_ego_bytes as i64,
                "canonical_corpus_promotion": false,
                "semantic_mismatches": 0
            }));

            drop(staged);
            assert!(!destination.exists(), "staging must not publish {key}");
        }
    }

    println!(
        "EGO_MIGRATION_MEASUREMENT {}",
        serde_json::json!({
            "schema": 1,
            "fixture_root": root,
            "authoritative_sources_unchanged": true,
            "records": measurements,
            "zero_ego_cases": zero_ego_cases,
            "nonempty_ego_cases": nonempty_ego_cases,
            "raw_ego_payload_bytes_before_total": raw_ego_payload_bytes_before_total,
            "raw_ego_payload_bytes_after_total": raw_ego_payload_bytes_after_total,
            "framed_ego_bytes_before_total": framed_ego_bytes_before_total,
            "framed_ego_bytes_after_total": framed_ego_bytes_after_total,
            "coverage_note": "Per-file Ego counts are reported. Zero-count fixtures provide no non-empty Ego migration coverage.",
            "scope": "raw Ego publication payloads and their eight-byte Container chunk-length frames; six pinned V1 REL and PHY sources"
        })
    );
}
