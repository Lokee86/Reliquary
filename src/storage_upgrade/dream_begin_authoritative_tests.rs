use super::{StorageUpgradeRequest, plan_storage_upgrade, stage_graph_envelope_storage_upgrade};
use crate::{Container, Cva, CvaError};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

#[test]
fn authoritative_dream_begin_migrations_preserve_proofs_and_measure_raw_bytes() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let baseline: serde_json::Value = serde_json::from_slice(
        &fs::read(repo.join("benchmarks/authoritative-history-rewrite-step62-2026-10-18.json"))
            .unwrap(),
    )
    .unwrap();
    let root = PathBuf::from(baseline["fixture_root"].as_str().unwrap());
    let directory = repo
        .join(".test-tmp")
        .join(format!("dream-authoritative-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    let mut rows = Vec::new();
    for corpus in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ] {
        let key = format!("{corpus}/project.prj.rel");
        let path = root.join(&key);
        let digest = format!("{:x}", Sha256::digest(fs::read(&path).unwrap()));
        assert_eq!(
            digest,
            baseline["authoritative_hashes"][&key].as_str().unwrap()
        );
        let before = Cva::open_v1_read_only(&path).unwrap();
        let expected =
            crate::historical_dream_proof_fingerprint::dream_proof_state(&before).unwrap();
        let expected_completed = before.historical_completed_initial_dream_passes().unwrap();
        let plan = plan_storage_upgrade(StorageUpgradeRequest {
            source: path.clone(),
            destination: directory.join(format!("{corpus}.rel")),
        })
        .unwrap();
        let staged = stage_graph_envelope_storage_upgrade(&plan).unwrap();
        let after = Cva::open(staged.test_path()).unwrap();
        assert_eq!(
            crate::historical_dream_proof_fingerprint::dream_proof_state(&after).unwrap(),
            expected
        );
        assert_eq!(
            after.historical_completed_initial_dream_passes().unwrap(),
            expected_completed
        );
        let mut source = Container::open_v1_read_only(&path).unwrap();
        let mut source_count = 0_usize;
        let mut source_bytes = 0_usize;
        source
            .visit_payloads::<CvaError>(|_, payload| {
                if payload.starts_with(b"CVDREAM1") && payload.get(24) == Some(&0) {
                    source_count += 1;
                    source_bytes += payload.len();
                }
                Ok(())
            })
            .unwrap();
        let mut destination = Container::open(staged.test_path()).unwrap();
        let (mut compact, mut exceptions, mut output_bytes) = (0_usize, 0_usize, 0_usize);
        destination
            .visit_payloads::<CvaError>(|_, payload| {
                assert!(!(payload.starts_with(b"CVDREAM1") && payload.get(24) == Some(&0)));
                if payload.starts_with(b"CVDREAM2") {
                    output_bytes += payload.len();
                    match crate::dream_begin_codec::decode(payload)
                        .unwrap()
                        .population
                    {
                        crate::dream_begin_codec::BeginPopulation::CompactCut { .. } => {
                            compact += 1
                        }
                        crate::dream_begin_codec::BeginPopulation::ExplicitSet(_) => {
                            exceptions += 1
                        }
                    }
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(compact + exceptions, source_count);
        assert_eq!(
            format!("{:x}", Sha256::digest(fs::read(&path).unwrap())),
            digest
        );
        rows.push(
            serde_json::json!({"fixture":key,"sha256":digest,"begin_records":source_count,
            "legacy_payload_bytes":source_bytes,"current_payload_bytes":output_bytes,
            "compact_cut_records":compact,"explicit_exception_records":exceptions,
            "proof_fingerprint_equal":true,"completed_proofs_equal":true,"source_unchanged":true}),
        );
        println!(
            "Dream migration accepted {corpus}: {source_bytes} -> {output_bytes}; compact={compact}, exceptions={exceptions}"
        );
    }
    fs::write(
        repo.join("benchmarks/dream-begin-authoritative-upgrade-2026-10-21.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"schema":1,"date":"2026-10-21",
            "family":"Dream Begin","compression_included":false,"fixtures":rows}))
        .unwrap(),
    )
    .unwrap();
    fs::remove_dir_all(&directory).unwrap();
}
