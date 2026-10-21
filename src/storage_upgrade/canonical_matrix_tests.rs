//! One latest-converter matrix; source oracles are frozen before reader retirement.
use super::*;
use crate::historical_fingerprint::HistoricalFingerprint;
use crate::{Container, Cva, Phylactery};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use crate::historical_current_corpus_tests::{
    archive_points, global_points, hex, mismatch, phy, rel,
};

fn frames(path: &Path, legacy: bool) -> Value {
    let mut owner = if legacy {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut families = BTreeMap::<String, (u64, u64)>::new();
    owner
        .visit_payloads::<crate::ContainerError>(|_, bytes| {
            let tag = if bytes.iter().all(|v| *v == 0) {
                "zero physical extent".into()
            } else {
                String::from_utf8_lossy(&bytes[..bytes.len().min(8)]).into_owned()
            };
            let item = families.entry(tag).or_default();
            item.0 += 1;
            item.1 += bytes.len() as u64 + 8;
            Ok(())
        })
        .unwrap();
    let total = fs::metadata(path).unwrap().len();
    assert_eq!(families.values().map(|v| v.1).sum::<u64>() + 40, total);
    json!({"total_raw_file_bytes":total,"header_bytes":40,"families":families})
}

fn retained_evidence(path: &Path, legacy: bool) -> Vec<Vec<u8>> {
    let mut scan = if legacy {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut values = Vec::new();
    scan.visit_payloads::<crate::MemoryError>(|_, bytes| {
        if legacy {
            if crate::entity_resolution_codec::retained::is_source(bytes) {
                values.push(bytes.to_vec());
            }
        } else {
            assert!(!crate::entity_resolution_codec::retained::is_source(bytes));
            if let Some(raw) = crate::entity_resolution_codec::retained::decode(bytes)? {
                values.push(raw.to_vec());
            }
        }
        Ok(())
    })
    .unwrap();
    values
}

#[test]
fn canonical_six_source_conversion_and_owner_admission() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let baseline: Value = serde_json::from_slice(
        &fs::read(repo.join("benchmarks/authoritative-history-rewrite-step62-2026-10-18.json"))
            .unwrap(),
    )
    .unwrap();
    let source_root = PathBuf::from(baseline["fixture_root"].as_str().unwrap());
    let directory = repo
        .join(".test-tmp")
        .join(format!("canonical-converted-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    let mut rows = Vec::new();
    for (key, digest) in baseline["authoritative_hashes"].as_object().unwrap() {
        println!("standalone canonical conversion start {key}");
        let path = source_root.join(key);
        let original = fs::read(&path).unwrap();
        assert_eq!(hex(&Sha256::digest(&original)), digest.as_str().unwrap());
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let destination = directory.join(key);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        let plan = plan_storage_upgrade(StorageUpgradeRequest {
            source: path.clone(),
            destination: destination.clone(),
        })
        .unwrap();
        let start = Instant::now();
        let stage = stage_graph_envelope_storage_upgrade(&plan).unwrap();
        let migration_ms = start.elapsed().as_millis();
        assert_eq!(
            retained_evidence(&path, true),
            retained_evidence(stage.test_path(), false)
        );
        let heads = if key.ends_with(".rel") {
            let old = Cva::open_v1_read_only(&path).unwrap();
            let mut current = Cva::open(stage.test_path()).unwrap();
            current.validate_historical_import_settlements().unwrap();
            current.validate_import_completion_settlements().unwrap();
            assert_eq!(old.owner_uuid(), current.owner_uuid());
            let old_heads = (
                old.latest_global_version(),
                old.archive_version(),
                old.memory_version(),
                old.entities.entity_version(),
                old.relationships.relationship_version(),
                old.graph_version(),
                old.memory_graph_version(),
                old.vector_version(),
                old.rel_turn_count(),
            );
            let current_heads = (
                current.latest_global_version(),
                current.archive_version(),
                current.memory_version(),
                current.entities.entity_version(),
                current.relationships.relationship_version(),
                current.graph_version(),
                current.memory_graph_version(),
                current.vector_version(),
                current.rel_turn_count(),
            );
            assert_eq!(old_heads, current_heads);
            json!(old_heads)
        } else {
            let old = Phylactery::open_v1_read_only(&path).unwrap();
            let current = Phylactery::open(stage.test_path()).unwrap();
            assert_eq!(old.owner_uuid(), current.owner_uuid());
            let old_heads = (
                old.latest_global_version(),
                old.memory_version(),
                old.entities.entity_version(),
                old.relationships.relationship_version(),
                old.graph_version(),
                old.memory_graph_version(),
            );
            let current_heads = (
                current.latest_global_version(),
                current.memory_version(),
                current.entities.entity_version(),
                current.relationships.relationship_version(),
                current.graph_version(),
                current.memory_graph_version(),
            );
            assert_eq!(old_heads, current_heads);
            json!(old_heads)
        };
        let source_frames = frames(&path, true);
        let canonical_frames = frames(stage.test_path(), false);
        for retired in [
            "CVAFRS03", "CVAFRS04", "CVAFRS05", "CVAILEG1", "CVAILEG2", "CVAERS02",
        ] {
            assert!(canonical_frames["families"].get(retired).is_none());
        }
        let canonical = fs::read(stage.test_path()).unwrap();
        fs::write(&destination, &canonical).unwrap();
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
        rows.push(json!({"fixture":key,"source_sha256":digest,"canonical_sha256":hex(&Sha256::digest(&canonical)),"canonical_copy":destination,"source_unchanged":true,"owner_admission":true,"heads":heads,"migration_ms":migration_ms,"source_raw":source_frames,"canonical_raw":canonical_frames}));
        let report = json!({"schema":1,"date":"2026-10-21","historical_equivalence_complete":false,"authoritative_promotion":false,"compression_included":false,"fixtures":rows});
        fs::write(
            repo.join("benchmarks/canonical-history-conversion-2026-10-21.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        println!("standalone canonical conversion admitted {key} in {migration_ms}ms");
    }
    assert_eq!(rows.len(), 6);
}

#[test]
fn canonical_six_source_migration_and_historical_equivalence() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let baseline: Value = serde_json::from_slice(
        &fs::read(repo.join("benchmarks/authoritative-history-rewrite-step62-2026-10-18.json"))
            .unwrap(),
    )
    .unwrap();
    let source_root = PathBuf::from(baseline["fixture_root"].as_str().unwrap());
    let run = std::env::var("RELIQUARY_CANONICAL_MATRIX_RUN")
        .unwrap_or_else(|_| format!("canonical-verified-{}", uuid::Uuid::new_v4()));
    assert!(
        run.starts_with("canonical-verified-")
            && run.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    );
    let directory = repo.join(".test-tmp").join(run);
    fs::create_dir_all(&directory).unwrap();
    println!("canonical matrix directory {}", directory.display());
    let mut rows: Vec<Value> = if directory.join("progress.json").exists() {
        let progress: Value =
            serde_json::from_slice(&fs::read(directory.join("progress.json")).unwrap()).unwrap();
        assert_eq!(progress["schema"], 2);
        progress["fixtures"].as_array().unwrap().clone()
    } else {
        Vec::new()
    };
    for (key, digest) in baseline["authoritative_hashes"].as_object().unwrap() {
        println!("canonical migration start {key}");
        let path = source_root.join(key);
        let original = fs::read(&path).unwrap();
        assert_eq!(hex(&Sha256::digest(&original)), digest.as_str().unwrap());
        if let Some(row) = rows.iter().find(|row| row["fixture"] == key.as_str()) {
            assert_eq!(row["source_sha256"], *digest);
            let output = directory.join("canonical-v2").join(key);
            let frozen = directory.join("frozen-v1").join(key);
            assert_eq!(
                hex(&Sha256::digest(fs::read(output).unwrap())),
                row["canonical_sha256"].as_str().unwrap()
            );
            assert_eq!(
                hex(&Sha256::digest(fs::read(frozen).unwrap())),
                digest.as_str().unwrap()
            );
            println!("resumed verified {key}");
            continue;
        }
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let legacy = directory.join("frozen-v1").join(key);
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, &original).unwrap();
        let destination = directory.join("canonical-v2").join(key);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        let plan = plan_storage_upgrade(StorageUpgradeRequest {
            source: path.clone(),
            destination: destination.clone(),
        })
        .unwrap();
        let start = Instant::now();
        let stage = stage_graph_envelope_storage_upgrade(&plan).unwrap();
        let migration_ms = start.elapsed().as_millis();
        assert_eq!(
            retained_evidence(&path, true),
            retained_evidence(stage.test_path(), false),
            "literal retained Entity Resolution evidence for {key}"
        );
        let (expected, actual, points) = if key.ends_with(".rel") {
            let mut old = Cva::open_v1_read_only(&path).unwrap();
            let mut new = Cva::open(stage.test_path()).unwrap();
            super::archive_envelope_authoritative_tests::compare_archive(&mut old, &mut new);
            let points = global_points(&mut old);
            let archives = archive_points(&mut old);
            let expected = rel(&mut old, &points, &archives);
            let actual = rel(&mut new, &points, &archives);
            (expected, actual, points)
        } else {
            let mut old = Phylactery::open_v1_read_only(&path).unwrap();
            let mut new = Phylactery::open(stage.test_path()).unwrap();
            (phy(&mut old), phy(&mut new), Vec::new())
        };
        if let Some(difference) = mismatch(&expected, &actual, "") {
            panic!("owner-history mismatch for {key} at {difference}");
        }
        let old_frames = frames(&path, true);
        let new_frames = frames(stage.test_path(), false);
        let canonical = fs::read(stage.test_path()).unwrap();
        let hash = hex(&Sha256::digest(&canonical));
        fs::write(&destination, &canonical).unwrap();
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
        rows.push(json!({"fixture":key,"source_sha256":digest,"canonical_sha256":hash,"source_unchanged":true,"frozen_v1_copy":legacy,"verified_canonical_copy":destination,"migration_ms":migration_ms,"source_raw":old_frames,"canonical_raw":new_frames,"global_projection_points":points,"oracle":expected}));
        let report = json!({"schema":2,"date":"2026-10-21","compression_included":false,"authoritative_sources_unchanged":true,"all_owner_domain_mismatches":0,"fixtures":rows});
        fs::write(
            directory.join("progress.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        println!("canonical migration verified {key}");
    }
    fs::copy(
        directory.join("progress.json"),
        repo.join("benchmarks/canonical-history-equivalence-2026-10-21.json"),
    )
    .unwrap();
}

#[test]
fn canonical_six_frozen_reconciliation_policy() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let report: Value = serde_json::from_slice(
        &fs::read(repo.join("benchmarks/canonical-history-equivalence-2026-10-21.json")).unwrap(),
    )
    .unwrap();
    let mut rows = Vec::new();
    for row in report["fixtures"].as_array().unwrap() {
        let key = row["fixture"].as_str().unwrap();
        if !key.ends_with(".rel") {
            continue;
        }
        let old = Cva::open_v1_read_only(row["frozen_v1_copy"].as_str().unwrap()).unwrap();
        let new = Cva::open(row["verified_canonical_copy"].as_str().unwrap()).unwrap();
        let frozen = format!(
            "{:?}",
            crate::cva_reconcile_freshness::validate_freshness(&old, &old)
        );
        let current = format!(
            "{:?}",
            crate::cva_reconcile_freshness::validate_freshness(&new, &new)
        );
        assert_eq!(
            frozen, current,
            "reconciliation domain policy changed for {key}"
        );
        rows.push(json!({"fixture":key,"source_sha256":row["source_sha256"],"canonical_sha256":row["canonical_sha256"],"frozen_v1_freshness_rebase_decision":frozen,"current_v2_freshness_rebase_decision":current}));
        println!("frozen reconciliation policy agrees {key}");
    }
    fs::write(
        repo.join("benchmarks/canonical-history-frozen-reconciliation-policy-2026-10-21.json"),
        serde_json::to_vec_pretty(&json!({"schema":1,"fixtures":rows})).unwrap(),
    )
    .unwrap();
}
