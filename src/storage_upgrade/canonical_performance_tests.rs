//! Isolated, temporary pre-retirement measurements; the parent samples process memory.
use super::*;
use crate::{Container, Cva, Phylactery};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::PathBuf, time::Instant};

fn memory_queries(container: &mut Container, memories: &crate::memory_store::MemoryStore) -> Value {
    let start = Instant::now();
    let history =
        crate::historical_memory_fingerprint::MemoryHistory::read(container, memories).unwrap();
    let prepare_ms = start.elapsed().as_secs_f64() * 1000.0;
    let head = memories.memory_version();
    let cuts = BTreeSet::from([head.min(1), head / 2, head]);
    let mut queries = Vec::new();
    for cut in cuts {
        let mut times = Vec::new();
        let mut fingerprint = None;
        for _ in 0..3 {
            let start = Instant::now();
            let value = history.at(cut).unwrap().fingerprint;
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            if let Some(previous) = &fingerprint {
                assert_eq!(previous, &value);
            }
            fingerprint = Some(value);
        }
        let fingerprint = fingerprint.unwrap();
        queries.push(json!({"local_cut":cut,"lookup_ms":times,"rows":fingerprint.rows,"sha256":fingerprint.sha256.iter().map(|byte|format!("{byte:02x}")).collect::<String>()}));
    }
    json!({"query":"preloaded historical Memory snapshot fingerprint","preparation_ms":prepare_ms,"head":head,"queries":queries})
}

#[test]
#[ignore = "isolated performance driver supplies pinned fixture, mode and output"]
fn canonical_performance_child() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let key = std::env::var("RELIQUARY_PERFORMANCE_FIXTURE").unwrap();
    let mode = std::env::var("RELIQUARY_PERFORMANCE_MODE").unwrap();
    assert!(["source", "canonical", "migrate"].contains(&mode.as_str()));
    let report_name = std::env::var("RELIQUARY_PERFORMANCE_REPORT").unwrap();
    assert!(
        report_name.starts_with("canonical-performance-child-")
            && report_name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
    );
    let conversion: Value = serde_json::from_slice(
        &fs::read(repo.join("benchmarks/canonical-history-conversion-2026-10-21.json")).unwrap(),
    )
    .unwrap();
    let fixture = conversion["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["fixture"] == key)
        .unwrap();
    let baseline: Value = serde_json::from_slice(
        &fs::read(repo.join("benchmarks/authoritative-history-rewrite-step62-2026-10-18.json"))
            .unwrap(),
    )
    .unwrap();
    let source = PathBuf::from(baseline["fixture_root"].as_str().unwrap()).join(&key);
    let path = if mode == "canonical" {
        PathBuf::from(fixture["canonical_copy"].as_str().unwrap())
    } else {
        source.clone()
    };
    let result = if mode == "migrate" {
        let root = repo
            .join(".test-tmp")
            .join(format!("performance-migration-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let start = Instant::now();
        let plan = plan_storage_upgrade(StorageUpgradeRequest {
            source,
            destination: root.join("candidate"),
        })
        .unwrap();
        let plan_ms = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        let stage = stage_graph_envelope_storage_upgrade(&plan).unwrap();
        let stage_ms = start.elapsed().as_secs_f64() * 1000.0;
        let bytes = fs::metadata(stage.test_path()).unwrap().len();
        drop(stage);
        assert!(root.is_absolute() && root.parent() == Some(repo.join(".test-tmp").as_path()));
        fs::remove_dir_all(root).unwrap();
        json!({"plan_ms":plan_ms,"stage_ms":stage_ms,"total_migration_ms":plan_ms+stage_ms,"source_bytes":fixture["source_raw"]["total_raw_file_bytes"],"canonical_bytes":bytes})
    } else {
        macro_rules! opened {
            ($open:expr) => {{
                let start=Instant::now();
                let mut owner=$open;
                let first=start.elapsed().as_secs_f64()*1000.0;
                let mut warm=Vec::new();
                for _ in 0..3 {
                    drop(owner);
                    let start=Instant::now();
                    owner=$open;
                    warm.push(start.elapsed().as_secs_f64()*1000.0);
                }
                let queries=memory_queries(&mut owner.container,&owner.memories);
                json!({"first_open_in_fresh_process_ms":first,"warm_reopen_ms":warm,"memory_history":queries})
            }};
        }
        if key.ends_with(".rel") {
            opened!(if mode == "source" {
                Cva::open_v1_read_only(&path).unwrap()
            } else {
                Cva::open(&path).unwrap()
            })
        } else {
            opened!(if mode == "source" {
                Phylactery::open_v1_read_only(&path).unwrap()
            } else {
                Phylactery::open(&path).unwrap()
            })
        }
    };
    let report = json!({"schema":1,"fixture":key,"mode":mode,"measurement":result});
    fs::write(
        repo.join(".test-tmp").join(format!("{report_name}.json")),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    // Let the parent collect the OS lifetime peak before the process handle exits.
    std::thread::sleep(std::time::Duration::from_millis(20));
}
