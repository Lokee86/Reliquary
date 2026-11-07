use serde_json::Value;
use std::{env, path::PathBuf, process::Command};

#[test]
#[ignore = "requires completed cohort; executed by scripts/run_g4_acceptance.ps1"]
fn external_root_crate_consumer_reopens_the_exported_authored_cohort() {
    let export_dir = env::var("G4_EXPORT_DIR")
        .expect("run scripts/run_g4_acceptance.ps1 to generate the completed cohort");
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("g4-native-rel21-v1.json");
    let output = Command::new(env!("CARGO_BIN_EXE_reliquary-external-consumer-check"))
        .arg(export_dir)
        .arg(manifest)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "external consumer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["rel_lantern"]["conversations"].as_u64().unwrap() >= 8);
    assert!(report["rel_cascade"]["conversations"].as_u64().unwrap() >= 4);
    assert_eq!(report["authored_turns_checked"], 48);
    assert!(report["rel_lantern"]["turns"].as_u64().unwrap() >= 32);
    assert!(report["rel_cascade"]["turns"].as_u64().unwrap() >= 16);
    assert!(report["phy_a"]["memory_count"].as_u64().unwrap() > 0);
    assert!(report["phy_b"]["memory_count"].as_u64().unwrap() > 0);
    assert_eq!(report["phy_privacy"]["private_routing_verified"], true);
    assert_eq!(
        report["rel_lantern"]["committed_queue_archive_verified"],
        true
    );
    assert_eq!(report["rel_lantern"]["uncommitted_queue_text_absent"], true);
    assert_eq!(report["rel_lantern"]["reopen_observations_match"], true);
    assert_eq!(report["rel_lantern"]["manifest_provenance_verified"], true);
    assert_eq!(report["source_bytes_unchanged"], true);
}
