use serde_json::Value;
use std::{env, path::PathBuf, process::Command};

#[test]
fn external_root_crate_consumer_reopens_the_exported_authored_cohort() {
    let Ok(export_dir) = env::var("G4_EXPORT_DIR") else {
        eprintln!(
            "G4_EXPORT_DIR is unset; run scripts/run_g4_acceptance.ps1 for the authored-cohort process witness"
        );
        return;
    };
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
    assert_eq!(report["rel_lantern"]["conversations"], 8);
    assert_eq!(report["rel_cascade"]["conversations"], 4);
    assert!(report["rel_lantern"]["turns"].as_u64().unwrap() >= 32);
    assert!(report["rel_cascade"]["turns"].as_u64().unwrap() >= 16);
    assert!(report["phy_a"]["memory_count"].as_u64().unwrap() > 0);
    assert!(report["phy_b"]["memory_count"].as_u64().unwrap() > 0);
    assert_eq!(report["source_bytes_unchanged"], true);
}
