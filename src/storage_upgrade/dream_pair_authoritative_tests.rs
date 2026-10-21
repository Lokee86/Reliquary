use super::{StorageUpgradeRequest, plan_storage_upgrade, stage_graph_envelope_storage_upgrade};
use crate::{Container, Cva, CvaError, Phylactery};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

fn raw_bytes(path: &std::path::Path, legacy: bool) -> (usize, usize, usize) {
    let mut scan = if legacy {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let (mut mappings, mut pairs, mut payload_bytes) = (0, 0, 0);
    scan.visit_payloads::<CvaError>(|_, payload| {
        if payload.starts_with(b"CVADRM01") {
            mappings += 1;
            payload_bytes += payload.len();
        }
        if payload.starts_with(b"CVADRP01") || payload.starts_with(b"CVADRP02") {
            if !legacy {
                assert!(!payload.starts_with(b"CVADRP01"));
            }
            pairs += 1;
            payload_bytes += payload.len();
        }
        Ok(())
    })
    .unwrap();
    (mappings, pairs, payload_bytes)
}

#[test]
fn authoritative_dream_pair_upgrades_preserve_rel_phy_sets_proofs_and_raw_measurements() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let baseline: serde_json::Value = serde_json::from_slice(
        &fs::read(repo.join("benchmarks/authoritative-history-rewrite-step62-2026-10-18.json"))
            .unwrap(),
    )
    .unwrap();
    let root = PathBuf::from(baseline["fixture_root"].as_str().unwrap());
    let directory = repo
        .join(".test-tmp")
        .join(format!("dream-pair-authoritative-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    let mut rows = Vec::new();
    for corpus in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ] {
        for (kind, file) in [("rel", "project.prj.rel"), ("phy", "user.phy")] {
            let key = format!("{corpus}/{file}");
            let path = root.join(&key);
            let digest = format!("{:x}", Sha256::digest(fs::read(&path).unwrap()));
            assert_eq!(
                digest,
                baseline["authoritative_hashes"][&key].as_str().unwrap()
            );
            let (expected, dream) = if kind == "rel" {
                let source = Cva::open_v1_read_only(&path).unwrap();
                (
                    crate::historical_dream_pair_fingerprint::reliquary_dream_pairs(&source)
                        .unwrap(),
                    Some(
                        crate::historical_dream_proof_fingerprint::dream_proof_state(&source)
                            .unwrap(),
                    ),
                )
            } else {
                let source = Phylactery::open_v1_read_only(&path).unwrap();
                (
                    crate::historical_dream_pair_fingerprint::phylactery_dream_pairs(&source)
                        .unwrap(),
                    None,
                )
            };
            let plan = plan_storage_upgrade(StorageUpgradeRequest {
                source: path.clone(),
                destination: directory.join(format!("{corpus}.{kind}")),
            })
            .unwrap();
            let stage = stage_graph_envelope_storage_upgrade(&plan).unwrap();
            let actual = if kind == "rel" {
                let output = Cva::open(stage.test_path()).unwrap();
                assert_eq!(
                    Some(
                        crate::historical_dream_proof_fingerprint::dream_proof_state(&output)
                            .unwrap()
                    ),
                    dream
                );
                crate::historical_dream_pair_fingerprint::reliquary_dream_pairs(&output).unwrap()
            } else {
                let output = Phylactery::open(stage.test_path()).unwrap();
                crate::historical_dream_pair_fingerprint::phylactery_dream_pairs(&output).unwrap()
            };
            assert_eq!(actual.pairs, expected.pairs);
            assert_eq!(actual.fingerprint, expected.fingerprint);
            let old = raw_bytes(&path, true);
            let new = raw_bytes(stage.test_path(), false);
            assert_eq!(new.1, expected.pairs.len());
            assert_eq!(
                format!("{:x}", Sha256::digest(fs::read(&path).unwrap())),
                digest
            );
            rows.push(serde_json::json!({"fixture":key,"sha256":digest,"source_unchanged":true,
                "legacy_pair_records":old.1,"current_pair_records":new.1,"mapping_records":new.0,
                "legacy_payload_bytes":old.2,"current_payload_bytes":new.2,
                "legacy_framed_bytes":old.2+(old.0+old.1)*8,"current_framed_bytes":new.2+(new.0+new.1)*8,
                "pair_fingerprint_equal":true,"dream_proof_equal":if kind == "rel" { Some(true) } else { None::<bool> }}));
            println!(
                "Dream pair migration {key}: payload {} -> {}, mappings={}, pairs={}",
                old.2, new.2, new.0, new.1
            );
        }
    }
    fs::write(repo.join("benchmarks/dream-pair-storage-2026-10-21.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"schema":1,"date":"2026-10-21","family":"Dream pair",
            "compression_included":false,"reference_table_included":true,"fixtures":rows,
            "synthetic":[{"memories":2,"pairs":1,"legacy_payload_bytes":72,"current_payload_bytes":104,"legacy_framed_bytes":80,"current_framed_bytes":128},
                         {"memories":4,"pairs":6,"legacy_payload_bytes":432,"current_payload_bytes":272,"legacy_framed_bytes":480,"current_framed_bytes":352},
                         {"memories":16,"pairs":120,"legacy_payload_bytes":8640,"current_payload_bytes":2624,"legacy_framed_bytes":9600,"current_framed_bytes":3712}]})).unwrap()).unwrap();
    fs::remove_dir_all(&directory).unwrap();
}
