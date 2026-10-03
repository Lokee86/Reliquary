//! Bounded, test-only measurement and mixed-format ledger validation.
//! REL_ACTIVITY_FIXTURES is a semicolon-separated list of frozen REL paths.
//! Without it, use a disposable 512-turn fixture; no timing threshold is a test gate.
use super::ArchiveActivityIndex;
use crate::archive_codec::{ArchiveRecord, decode_record};
use crate::archive_record_index::NodeIndex;
use crate::{Cva, IncomingTurn, Node};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn digest(path: &Path) -> String {
    let mut file = fs::File::open(path).unwrap();
    let mut hash = Sha256::new();
    let mut bytes = [0; 65536];
    loop {
        let count = file.read(&mut bytes).unwrap();
        if count == 0 {
            break;
        }
        hash.update(&bytes[..count]);
    }
    format!("{:x}", hash.finalize())
}
fn median(values: &mut [u128]) -> u128 {
    values.sort_unstable();
    values[values.len() / 2]
}

// Same NodeIndex preparation/insertion on both sides. Only the second side adds
// canonical activity preflight/accounting. Exclude Node cloning and destruction.
fn replay_cost(rows: &[(u64, Node)], with_activity: bool) -> (u128, ArchiveActivityIndex) {
    let input = rows.to_vec();
    let mut nodes = NodeIndex::default();
    let mut activity = ArchiveActivityIndex::default();
    let start = Instant::now();
    for (version, node) in input {
        if with_activity {
            let prepared = activity.prepare_node(&mut nodes, &node).unwrap().unwrap();
            assert!(nodes.insert(node).unwrap());
            activity.commit_new(prepared, version);
        } else {
            assert!(nodes.prepare_insert(&node).unwrap());
            assert!(nodes.insert(node).unwrap());
        }
    }
    let ns = start.elapsed().as_nanos();
    std::hint::black_box(&nodes);
    (ns, activity)
}

fn profile(path: &Path) {
    let before = digest(path);
    // Cva::open requests write access. Never relax a frozen source permission.
    let dir = std::env::temp_dir().join(format!("activity-read-copy-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let scratch = Scratch(dir);
    let inspected = scratch.0.join("fixture.rel");
    fs::copy(path, &inspected).unwrap();
    let mut permissions = fs::metadata(&inspected).unwrap().permissions();
    permissions.set_readonly(false);
    fs::set_permissions(&inspected, permissions).unwrap();
    assert_eq!(digest(&inspected), before);
    let mut opens = Vec::new();
    for _ in 0..3 {
        let start = Instant::now();
        let cva = Cva::open(&inspected).unwrap();
        opens.push(start.elapsed().as_nanos());
        std::hint::black_box(cva.rel_turn_count());
    }
    let mut cva = Cva::open(&inspected).unwrap();
    let versions = cva.archive.record_versions().to_vec();
    let mut identities = BTreeMap::<(String, String), (Node, Option<u64>)>::new();
    let mut roles = BTreeMap::<String, usize>::new();
    let mut formats = BTreeMap::<String, usize>::new();
    let mut rows = Vec::new();
    let mut count = 0_u64;
    for version in versions {
        let bytes = cva.container.read(version.record).unwrap();
        let node = match decode_record(&bytes).unwrap() {
            ArchiveRecord::Node(node) => {
                *formats
                    .entry(String::from_utf8(bytes[..8].to_vec()).unwrap())
                    .or_default() += 1;
                Some(node)
            }
            ArchiveRecord::IngestedTurn(turn) => {
                *formats
                    .entry(String::from_utf8(bytes[..8].to_vec()).unwrap())
                    .or_default() += 1;
                Some(turn.node)
            }
            _ => None,
        };
        if let Some(node) = node {
            let key = (node.conversation_id.clone(), node.id.clone());
            if let Some((original, _)) = identities.get(&key) {
                assert_eq!(original, &node);
            } else {
                // Independent reference definition: exact normalized roles only.
                let position = match node.role.as_str() {
                    "user" | "assistant" => {
                        count += 1;
                        Some(count)
                    }
                    _ => None,
                };
                *roles.entry(node.role.clone()).or_default() += 1;
                rows.push((version.archive_version, node.clone()));
                identities.insert(key, (node, position));
            }
        }
        assert_eq!(
            cva.activity_cut_at_archive_version(version.archive_version)
                .unwrap(),
            count
        );
    }
    assert_eq!(cva.activity_cut_at_archive_version(0).unwrap(), 0);
    assert_eq!(cva.rel_turn_count(), count);
    assert_eq!(cva.archive.nodes.len(), identities.len());
    for ((conversation, id), (_, position)) in &identities {
        assert_eq!(cva.activity_position_for_turn(conversation, id), *position);
    }
    let index = &cva.archive.activity;
    let used_bytes =
        index.first_activity_position_by_node.len() * 8 + index.activity_changes.len() * 16;
    let capacity_bytes = index.first_activity_position_by_node.capacity() * 8
        + index.activity_changes.capacity() * 16;
    let mut baseline = Vec::new();
    let mut indexed = Vec::new();
    for run in 0..9 {
        // Alternate order to reduce systematic warm-cache bias.
        if run % 2 == 0 {
            baseline.push(replay_cost(&rows, false).0);
            indexed.push(replay_cost(&rows, true).0);
        } else {
            indexed.push(replay_cost(&rows, true).0);
            baseline.push(replay_cost(&rows, false).0);
        }
    }
    let baseline_ns = median(&mut baseline);
    let indexed_ns = median(&mut indexed);
    let (_, rebuilt) = replay_cost(&rows, true);
    assert_eq!(rebuilt.current_count, index.current_count);
    assert_eq!(rebuilt.activity_changes, index.activity_changes);
    assert_eq!(
        rebuilt.first_activity_position_by_node,
        index.first_activity_position_by_node
    );
    let result = serde_json::json!({
        "path":path, "sha256":before, "inspected_verified_copy":true,
        "file_bytes":fs::metadata(path).unwrap().len(),
        "nodes":identities.len(), "eligible_turns":count, "archive_versions":cva.archive_version(),
        "unique_roles":roles, "version_linked_turn_payloads":formats,
        "open_runs":3, "open_median_us":median(&mut opens)/1000,
        "replay_runs":9, "node_index_replay_median_us":baseline_ns/1000,
        "node_and_activity_replay_median_us":indexed_ns/1000,
        "accounting_delta_ns_per_node":(indexed_ns as i128-baseline_ns as i128)/rows.len().max(1) as i128,
        "activity_used_bytes":used_bytes, "activity_capacity_bytes":capacity_bytes,
        "activity_struct_bytes":std::mem::size_of::<ArchiveActivityIndex>()
    });
    drop(cva);
    assert_eq!(
        digest(&inspected),
        before,
        "inspection must not mutate its copy"
    );
    assert_eq!(
        digest(path),
        before,
        "fixture inspection must not mutate bytes"
    );
    println!("activity-measurement {result}");
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn activity_fixture_ledger_and_measurement() {
    if let Ok(paths) = std::env::var("REL_ACTIVITY_FIXTURES") {
        let paths: Vec<_> = paths.split(';').filter(|p| !p.is_empty()).collect();
        assert!(
            !paths.is_empty(),
            "REL_ACTIVITY_FIXTURES must name a fixture"
        );
        for path in paths {
            profile(Path::new(path));
        }
        return;
    }
    let dir = std::env::temp_dir().join(format!("activity-measurement-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let scratch = Scratch(dir);
    let path = scratch.0.join("synthetic.rel");
    let mut cva = Cva::create(&path).unwrap();
    for i in 0..512 {
        let role = ["user", "assistant", "system", "tool"][i % 4];
        let turn = IncomingTurn {
            id: format!("turn-{i}"),
            conversation_id: format!("conversation-{}", i % 3),
            parent_id: None,
            role: role.into(),
            principal_id: None,
            timestamp_ns: -(i as i64),
            content: "shared content".into(),
            attachments: Vec::new(),
            project_attachments: Vec::new(),
        };
        if i % 2 == 0 {
            cva.append_node(
                turn.id,
                turn.conversation_id,
                None,
                turn.role,
                turn.timestamp_ns,
                &turn.content,
            )
            .unwrap();
        } else {
            cva.ingest_turn(turn).unwrap();
        }
    }
    cva.sync().unwrap();
    drop(cva);
    profile(&path);
}
