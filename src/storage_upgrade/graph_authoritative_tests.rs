#![cfg(feature = "history-oracle-fixtures")]

use super::shell::stage_graph_storage_upgrade;
use super::{StorageUpgradeRequest, plan_storage_upgrade};
use crate::graph_codec::decode_version;
use crate::graph_store::GraphStore;
use crate::{Container, ContainerError, Cva, FileKind, Phylactery};
use serde::Serialize;
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
                "graph-authoritative-upgrade-{}",
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

#[derive(Default, Serialize)]
struct GraphPayloadBytes {
    legacy_mutations: u64,
    legacy_mutation_count: u64,
    legacy_batches: u64,
    legacy_batch_count: u64,
    compact_mutations: u64,
    compact_mutation_count: u64,
    compact_batches: u64,
    compact_batch_count: u64,
    node_mapping_bytes: u64,
    node_mapping_count: u64,
    graph_version_bytes: u64,
    graph_version_count: u64,
    all_graph_payload_bytes: u64,
    all_graph_record_count: u64,
    all_graph_framed_bytes: u64,
    total_payload_bytes: u64,
    total_file_bytes: u64,
    container_framing_bytes: u64,
    published_legacy_mutation_bytes: u64,
    published_legacy_mutations: u64,
    published_legacy_batch_bytes: u64,
    published_legacy_batches: u64,
    published_compact_mutation_bytes: u64,
    published_compact_mutations: u64,
    published_compact_batch_bytes: u64,
    published_compact_batches: u64,
}

fn payload_bytes(path: &Path, v1: bool) -> GraphPayloadBytes {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut result = GraphPayloadBytes {
        total_file_bytes: fs::metadata(path).unwrap().len(),
        ..GraphPayloadBytes::default()
    };
    let mut published = Vec::new();
    container
        .visit_payloads::<ContainerError>(|_chunk, payload| {
            result.total_payload_bytes += payload.len() as u64;
            if payload.starts_with(b"CVAG") {
                result.all_graph_payload_bytes += payload.len() as u64;
                result.all_graph_record_count += 1;
                result.all_graph_framed_bytes += payload.len() as u64 + 8;
            }
            if payload.starts_with(b"CVAGMUT") {
                result.legacy_mutations += payload.len() as u64;
                result.legacy_mutation_count += 1;
            } else if payload.starts_with(b"CVAGBAT") {
                result.legacy_batches += payload.len() as u64;
                result.legacy_batch_count += 1;
            } else if payload.starts_with(b"CVAGCM") {
                result.compact_mutations += payload.len() as u64;
                result.compact_mutation_count += 1;
            } else if payload.starts_with(b"CVAGCB") {
                result.compact_batches += payload.len() as u64;
                result.compact_batch_count += 1;
            } else if payload.starts_with(b"CVAGNODE") || payload.starts_with(b"CVAGNOD2") {
                result.node_mapping_bytes += payload.len() as u64;
                result.node_mapping_count += 1;
            } else if payload.starts_with(b"CVAGVER1") {
                result.graph_version_bytes += payload.len() as u64;
                result.graph_version_count += 1;
                if let Some(version) = decode_version(payload).unwrap() {
                    published.push(version.mutation);
                }
            }
            Ok(())
        })
        .unwrap();
    for mutation in published {
        let payload = container.read(mutation).unwrap();
        if payload.starts_with(b"CVAGMUT") {
            result.published_legacy_mutation_bytes += payload.len() as u64;
            result.published_legacy_mutations += 1;
        } else if payload.starts_with(b"CVAGBAT") {
            result.published_legacy_batch_bytes += payload.len() as u64;
            result.published_legacy_batches += 1;
        } else if payload.starts_with(b"CVAGCM") {
            result.published_compact_mutation_bytes += payload.len() as u64;
            result.published_compact_mutations += 1;
        } else if payload.starts_with(b"CVAGCB") {
            result.published_compact_batch_bytes += payload.len() as u64;
            result.published_compact_batches += 1;
        }
    }
    result.container_framing_bytes = result
        .total_file_bytes
        .saturating_sub(result.total_payload_bytes);
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
    assert_eq!(after.len(), original_bytes.len(), "source length changed");
    assert_eq!(sha256(&after), original_sha256, "source hash changed");
    assert_eq!(
        metadata.modified().unwrap(),
        original_modified,
        "source mtime changed"
    );
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct StableMutation {
    source_kind: u8,
    source_id: [u8; 32],
    target_kind: u8,
    target_id: [u8; 32],
    kind_code: u16,
    active: bool,
    origin_code: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct AcceptedPublication {
    global_version: u64,
    graph_version: u64,
    mutations: Vec<StableMutation>,
}

fn accepted_publications(
    container: &mut Container,
    graph: &GraphStore,
) -> Vec<AcceptedPublication> {
    let mut result = Vec::new();
    for chunk in container.chunks().unwrap() {
        let payload = container.read(chunk).unwrap();
        let Some(version) = decode_version(&payload).unwrap() else {
            continue;
        };
        let carrier = container.read(version.mutation).unwrap();
        let mutations = graph
            .decode_stored_mutations(&carrier)
            .unwrap()
            .expect("accepted Graph marker must reference a mutation carrier")
            .into_iter()
            .map(|mutation| StableMutation {
                source_kind: mutation.source.kind.code(),
                source_id: mutation.source.id,
                target_kind: mutation.target.kind.code(),
                target_id: mutation.target.id,
                kind_code: mutation.kind.code(),
                active: mutation.active,
                origin_code: mutation.origin.code(),
            })
            .collect();
        result.push(AcceptedPublication {
            global_version: version.global_version,
            graph_version: version.graph_version,
            mutations,
        });
    }
    result
}

fn mapping_payloads(container: &mut Container) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            if payload.starts_with(b"CVAGNODE") || payload.starts_with(b"CVAGNOD2") {
                result.push(payload.to_vec());
            }
            Ok(())
        })
        .unwrap();
    result
}

fn compare_rel(source_path: &Path, staged_path: &Path) {
    let mut source = Cva::open_v1_read_only(source_path).unwrap();
    let mut staged = Cva::open(staged_path).unwrap();
    assert_eq!(source.container.identity(), staged.container.identity());
    assert_eq!(
        source.latest_global_version(),
        staged.latest_global_version()
    );
    let source_owner = source.owner_uuid();
    let source_head = source.graph_version();
    let source_memory_head = source.memory_graph_version();
    let staged_owner = staged.owner_uuid();
    let staged_head = staged.graph_version();
    let staged_memory_head = staged.memory_graph_version();
    compare_graphs(
        &mut source.container,
        &source.graph,
        source_owner,
        source_head,
        source_memory_head,
        &mut staged.container,
        &staged.graph,
        staged_owner,
        staged_head,
        staged_memory_head,
    );
}

fn compare_phy(source_path: &Path, staged_path: &Path) {
    let mut source = Phylactery::open_v1_read_only(source_path).unwrap();
    let mut staged = Phylactery::open(staged_path).unwrap();
    assert_eq!(source.container.identity(), staged.container.identity());
    assert_eq!(
        source.latest_global_version(),
        staged.latest_global_version()
    );
    let source_owner = source.owner_uuid();
    let source_head = source.graph_version();
    let source_memory_head = source.memory_graph_version();
    let staged_owner = staged.owner_uuid();
    let staged_head = staged.graph_version();
    let staged_memory_head = staged.memory_graph_version();
    compare_graphs(
        &mut source.container,
        &source.graph,
        source_owner,
        source_head,
        source_memory_head,
        &mut staged.container,
        &staged.graph,
        staged_owner,
        staged_head,
        staged_memory_head,
    );
}

#[allow(clippy::too_many_arguments)]
fn compare_graphs(
    source_container: &mut Container,
    source_graph: &GraphStore,
    source_owner: Option<[u8; 16]>,
    source_head: u64,
    source_memory_head: u64,
    staged_container: &mut Container,
    staged_graph: &GraphStore,
    staged_owner: Option<[u8; 16]>,
    staged_head: u64,
    staged_memory_head: u64,
) {
    assert_eq!(source_owner, staged_owner, "owner UUID");
    assert_eq!(source_head, staged_head, "Graph head");
    assert_eq!(source_memory_head, staged_memory_head, "MemoryGraph head");
    assert_eq!(
        source_graph.rewrite_baseline_memories(),
        staged_graph.rewrite_baseline_memories(),
        "Graph zero-cut isolated Memory baseline"
    );
    assert_eq!(
        source_graph.transaction_global_versions(),
        staged_graph.transaction_global_versions(),
        "Graph transaction coordinate sequence"
    );
    let source_mappings = mapping_payloads(source_container);
    let staged_mappings = mapping_payloads(staged_container);
    assert_eq!(
        source_mappings, staged_mappings,
        "stable node mapping bytes"
    );

    let source_publications = accepted_publications(source_container, source_graph);
    let staged_publications = accepted_publications(staged_container, staged_graph);
    assert_eq!(
        source_publications, staged_publications,
        "ordered accepted Graph publications"
    );
    for (index, publication) in source_publications.iter().enumerate() {
        assert_eq!(publication.graph_version, index as u64 + 1);
        assert!(!publication.mutations.is_empty());
        // Exact ordered mutations at every marker establish identical prefixes:
        // the owner reducer deterministically replaces each (source,target,kind)
        // state with the last mutation in that publication.
    }
    assert_eq!(
        source_graph.active_semantic_relations(),
        staged_graph.active_semantic_relations(),
        "current stable Graph relations"
    );
    assert_eq!(
        source_graph.stats(),
        staged_graph.stats(),
        "Graph projections"
    );
    for ticket in 0..=source_container.latest_version() {
        assert_eq!(
            source_container.transaction_time_ns(ticket),
            staged_container.transaction_time_ns(ticket),
            "transaction time for global ticket {ticket}"
        );
    }

    let source_state = reduce(&source_publications);
    let staged_state = reduce(&staged_publications);
    assert_eq!(
        source_state, staged_state,
        "deterministic final state reduction"
    );
}

fn reduce(
    publications: &[AcceptedPublication],
) -> BTreeMap<(u8, [u8; 32], u8, [u8; 32], u16), (bool, u8)> {
    let mut state = BTreeMap::new();
    for publication in publications {
        for mutation in &publication.mutations {
            state.insert(
                (
                    mutation.source_kind,
                    mutation.source_id,
                    mutation.target_kind,
                    mutation.target_id,
                    mutation.kind_code,
                ),
                (mutation.active, mutation.origin_code),
            );
        }
    }
    state
}

#[test]
fn authoritative_v1_graph_histories_stage_without_semantic_or_source_changes() {
    let root = PathBuf::from(
        std::env::var_os("RELIQUARY_HISTORY_FIXTURES")
            .expect("RELIQUARY_HISTORY_FIXTURES must name accepted fixture root"),
    );
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let accepted: Value =
        serde_json::from_slice(&fs::read(manifest.join(ACCEPTED_HASHES)).unwrap()).unwrap();
    let scratch = ScratchDirectory::new();
    let mut measurements = Vec::new();
    let mut zero_graph_cases = 0_u64;
    let mut raw_graph_bytes_before_total = 0_u64;
    let mut raw_graph_bytes_after_total = 0_u64;

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
                "accepted source hash for {key}"
            );
            let modified = fs::metadata(&source_path).unwrap().modified().unwrap();
            let before = payload_bytes(&source_path, true);
            let destination = scratch.destination(&format!("{corpus}-{kind}"));
            let plan = plan_storage_upgrade(StorageUpgradeRequest {
                source: source_path.clone(),
                destination: destination.clone(),
            })
            .unwrap_or_else(|error| panic!("plan {key}: {error}"));
            let staged = stage_graph_storage_upgrade(&plan)
                .unwrap_or_else(|error| panic!("stage {key}: {error}"));
            let after = payload_bytes(staged.test_path(), false);
            let staged_sha256 = sha256(&fs::read(staged.test_path()).unwrap());

            match file_kind {
                FileKind::Reliquary => compare_rel(&source_path, staged.test_path()),
                FileKind::Phylactery => compare_phy(&source_path, staged.test_path()),
            }
            assert_source_unchanged(&source_path, &original_bytes, &original_sha256, modified);
            assert_eq!(before.graph_version_count, after.graph_version_count);
            assert_eq!(before.node_mapping_bytes, after.node_mapping_bytes);
            assert_eq!(before.node_mapping_count, after.node_mapping_count);
            assert_eq!(after.published_legacy_mutations, 0);
            assert_eq!(after.published_legacy_batches, 0);
            let count_before = before.published_legacy_mutations + before.published_legacy_batches;
            let count_after = after.published_compact_mutations + after.published_compact_batches;
            assert_eq!(
                count_before, count_after,
                "published transaction carrier count"
            );
            let raw_before =
                before.published_legacy_mutation_bytes + before.published_legacy_batch_bytes;
            let raw_after =
                after.published_compact_mutation_bytes + after.published_compact_batch_bytes;
            zero_graph_cases += u64::from(count_before == 0);
            raw_graph_bytes_before_total += raw_before;
            raw_graph_bytes_after_total += raw_after;
            measurements.push(serde_json::json!({
                "fixture": key,
                "owner_kind": kind,
                "source_sha256": original_sha256,
                "staged_sha256": staged_sha256,
                "source_unchanged": true,
                "source_size_bytes": original_bytes.len(),
                "graph_publication_count": count_before,
                "before": before,
                "after": after,
                "raw_graph_carrier_bytes_before": raw_before,
                "raw_graph_carrier_bytes_after": raw_after,
                "raw_graph_carrier_bytes_delta": raw_after as i64 - raw_before as i64,
                "mapping_bytes_unchanged": true,
                "published_legacy_mutations_after": 0,
                "published_legacy_batches_after": 0,
                "canonical_corpus_promotion": false,
                "semantic_mismatches": 0
            }));

            drop(staged);
            assert!(!destination.exists(), "staging must not publish {key}");
        }
    }

    let measurement = serde_json::json!({
        "schema": 1,
        "step": 154,
        "date": "2026-10-20",
        "fixture_root": root,
        "authoritative_sources_unchanged": true,
        "records": measurements,
        "zero_graph_cases": zero_graph_cases,
        "raw_graph_carrier_bytes_before_total": raw_graph_bytes_before_total,
        "raw_graph_carrier_bytes_after_total": raw_graph_bytes_after_total,
        "raw_graph_carrier_bytes_delta_total": raw_graph_bytes_after_total as i64 - raw_graph_bytes_before_total as i64,
        "scope": "raw accepted Graph mutation and batch carriers; exact node mapping bytes; no compression; six frozen REL and PHY sources"
    });
    println!("GRAPH_MIGRATION_MEASUREMENT {measurement}");
    let artifact_dir = manifest.join(".test-tmp");
    fs::create_dir_all(&artifact_dir).unwrap();
    fs::write(
        artifact_dir.join("graph-upgrade-size.json"),
        serde_json::to_vec_pretty(&measurement).unwrap(),
    )
    .unwrap();
}
