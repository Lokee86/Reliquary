#![cfg(feature = "history-oracle-fixtures")]

use super::shell::stage_graph_storage_upgrade;
use super::{StorageUpgradeRequest, plan_storage_upgrade};
use crate::freshness_storage::FreshnessReplayEntry;
use crate::historical_freshness_fingerprint::{
    FreshnessCut, freshness_at_event, freshness_at_turn,
};
use crate::{Container, ContainerError, Cva, FileKind, FreshnessEventCut, Phylactery};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const ACCEPTED_HASHES: &str = "benchmarks/authoritative-history-rewrite-step62-2026-10-18.json";
const CORPORA: [&str; 3] = [
    "chatgpt-first14d",
    "chatgpt-first28d",
    "rhelm-david-r-ellis-pre-relationship",
];
const EXPLICIT_EVENT_FINGERPRINT_LIMIT: usize = 64;

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "freshness-authoritative-upgrade-{}",
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
struct FreshnessPayloadBytes {
    journal_payload_bytes: u64,
    journal_frame_bytes: u64,
    journal_frame_count: u64,
    event_count: u64,
    effect_count: u64,
    legacy_settlement_payload_bytes: u64,
    legacy_settlement_frame_bytes: u64,
    legacy_settlement_count: u64,
    semantic_v6_payload_bytes: u64,
    semantic_v6_frame_bytes: u64,
    semantic_v6_count: u64,
    other_freshness_payload_bytes: u64,
    total_payload_bytes: u64,
    total_file_bytes: u64,
}

fn payload_bytes(path: &Path, v1: bool) -> FreshnessPayloadBytes {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut result = FreshnessPayloadBytes {
        total_file_bytes: fs::metadata(path).unwrap().len(),
        ..FreshnessPayloadBytes::default()
    };
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            result.total_payload_bytes += payload.len() as u64;
            if payload.starts_with(b"CVAFRS02") {
                result.journal_payload_bytes += payload.len() as u64;
                result.journal_frame_bytes += payload.len() as u64 + 8;
                result.journal_frame_count += 1;
                for entry in FreshnessReplayEntry::from_payload(payload).unwrap() {
                    if let FreshnessReplayEntry::Event { effects, .. } = entry {
                        result.event_count += 1;
                        result.effect_count += effects.len() as u64;
                    }
                }
            } else if payload.starts_with(b"CVAFRS03")
                || payload.starts_with(b"CVAFRS04")
                || payload.starts_with(b"CVAFRS05")
            {
                result.legacy_settlement_payload_bytes += payload.len() as u64;
                result.legacy_settlement_frame_bytes += payload.len() as u64 + 8;
                result.legacy_settlement_count += 1;
            } else if payload.starts_with(b"CVAFRS06") {
                result.semantic_v6_payload_bytes += payload.len() as u64;
                result.semantic_v6_frame_bytes += payload.len() as u64 + 8;
                result.semantic_v6_count += 1;
            } else if payload.starts_with(b"CVAFRS") {
                result.other_freshness_payload_bytes += payload.len() as u64;
            }
            Ok(())
        })
        .unwrap();
    result
}

fn freshness_frames(path: &Path, v1: bool) -> Vec<Vec<u8>> {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut frames = Vec::new();
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            if payload.starts_with(b"CVAFRS") || payload.starts_with(b"CVAFRP01") {
                frames.push(payload.to_vec());
            }
            Ok(())
        })
        .unwrap();
    frames
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
    assert_eq!(sha256(&after), original_sha256, "source SHA-256 changed");
    assert_eq!(
        metadata.modified().unwrap(),
        original_modified,
        "source mtime changed"
    );
}

fn event_rows(entries: &[FreshnessReplayEntry]) -> Vec<(u64, String, u64)> {
    let mut rows = entries
        .iter()
        .filter_map(|entry| match entry {
            FreshnessReplayEntry::Event {
                event_id,
                source_turn,
                proof,
                ..
            } => Some((*source_turn, event_id.clone(), proof.graph_version)),
            _ => None,
        })
        .collect::<Vec<_>>();
    rows.sort();
    // Exact retry frames can repeat one stable event; they do not create a new cut.
    rows.dedup();
    rows
}

fn assert_cut_equal(source: &FreshnessCut, staged: &FreshnessCut, description: &str) {
    assert_eq!(
        source.fingerprint, staged.fingerprint,
        "fingerprint: {description}"
    );
    assert_eq!(source.records, staged.records, "records: {description}");
}

fn compare_rel(source_path: &Path, staged_path: &Path) -> (u64, u64, u64, u64, u64, u64) {
    let mut source = Cva::open_v1_read_only(source_path).unwrap();
    let staged = Cva::open(staged_path).unwrap();
    assert_eq!(source.owner_uuid(), staged.owner_uuid(), "REL owner UUID");
    assert_eq!(
        source.container.identity(),
        staged.container.identity(),
        "REL identity"
    );
    assert_eq!(
        source.latest_global_version(),
        staged.latest_global_version(),
        "REL global head"
    );
    assert_eq!(
        source.rel_turn_count(),
        staged.rel_turn_count(),
        "REL activity head"
    );
    assert_eq!(
        source.memory_graph_version(),
        staged.memory_graph_version(),
        "REL MemoryGraph proof head"
    );

    let source_entries = source.freshness.retained_history_entries().unwrap();
    let staged_entries = staged.freshness.retained_history_entries().unwrap();
    assert_eq!(
        source_entries, staged_entries,
        "all accepted Freshness replay/evidence rows"
    );
    let rows = event_rows(&source_entries);
    assert_eq!(
        rows,
        event_rows(&staged_entries),
        "stable event identities and proof cuts"
    );
    let event_count = rows.len() as u64;
    let effect_count = source_entries
        .iter()
        .filter_map(|entry| match entry {
            FreshnessReplayEntry::Event { effects, .. } => Some(effects.len() as u64),
            _ => None,
        })
        .sum();
    let activity_head = source.rel_turn_count();
    let graph_head = source.memory_graph_version();
    let zero = freshness_at_turn(&source, 0).unwrap();
    let staged_zero = freshness_at_turn(&staged, 0).unwrap();
    assert_cut_equal(&zero, &staged_zero, "activity turn 0");
    let head = freshness_at_turn(&source, activity_head).unwrap();
    let staged_head = freshness_at_turn(&staged, activity_head).unwrap();
    assert_cut_equal(&head, &staged_head, "activity head");

    // Compare the same-cut semantic proof at every accepted settlement. The
    // historical reader canonicalizes legacy settlement frames into their V6
    // semantic scope and intentionally excludes physical predecessor hashes.
    let mut settlement_cuts = BTreeSet::new();
    for settlement in source.freshness.import_settlements() {
        settlement_cuts.insert(settlement.base_cut);
        settlement_cuts.insert(settlement.settlement_cut);
    }
    for cut in settlement_cuts {
        assert_cut_equal(
            &freshness_at_turn(&source, cut).unwrap(),
            &freshness_at_turn(&staged, cut).unwrap(),
            &format!("CVAFRS06 normalized semantic proof at activity cut {cut}"),
        );
    }

    let mut event_cuts = 0_u64;
    let mut unavailable_event_cuts = 0_u64;
    let explicit_fingerprints = rows.len() <= EXPLICIT_EVENT_FINGERPRINT_LIMIT;
    if explicit_fingerprints {
        let mut previous: Option<(u64, String)> = None;
        for (turn, event_id, _proof_graph_version) in &rows {
            let cut = FreshnessEventCut {
                rel_turn: *turn,
                event_id_inclusive: Some(event_id.clone()),
                graph_version: graph_head,
            };
            let source_at = freshness_at_event(&source, &cut);
            let staged_at = freshness_at_event(&staged, &cut);
            match (source_at, staged_at) {
                (Ok(source_at), Ok(staged_at)) => {
                    assert_cut_equal(&source_at, &staged_at, &format!("event cut {event_id}"));
                    event_cuts += 1;
                }
                (Err(source_error), Err(staged_error)) => {
                    assert_eq!(
                        source_error, staged_error,
                        "same event-cut availability for {event_id}"
                    );
                    assert_eq!(
                        source_error,
                        "import settlement cut does not retain an intra-settlement trajectory",
                        "only the documented unavailable intra-settlement trajectory is accepted"
                    );
                    unavailable_event_cuts += 1;
                }
                (source_result, staged_result) => panic!(
                    "event-cut outcome differs for {event_id}: source={}, staged={}",
                    outcome_name(&source_result),
                    outcome_name(&staged_result)
                ),
            }
            if let Some((previous_turn, previous_id)) = &previous {
                assert!(
                    (*turn, event_id.as_str()) > (*previous_turn, previous_id.as_str()),
                    "stable same-turn event ordering"
                );
            }
            previous = Some((*turn, event_id.clone()));
        }
    } else {
        // For large histories, equality of the complete canonical ordered event
        // stream (IDs, turns, effects, provenance, semantic evidence and graph
        // proofs), enrollment/admission rows, policy and normalized settlement
        // evidence proves every prefix by Freshness's deterministic reducer.
        // Do not claim per-event oracle fingerprints were individually computed.
        assert_eq!(source.freshness.records(), staged.freshness.records());
    }

    assert_eq!(
        source.freshness.records(),
        staged.freshness.records(),
        "final Freshness projection"
    );
    assert_eq!(
        source.freshness_records_at(activity_head).unwrap(),
        staged.freshness_records_at(activity_head).unwrap(),
        "final activity-cut record state"
    );
    (
        event_count,
        effect_count,
        event_cuts,
        unavailable_event_cuts,
        u64::from(explicit_fingerprints),
        source.freshness.import_settlements().len() as u64,
    )
}

fn outcome_name<T>(result: &Result<T, String>) -> &'static str {
    if result.is_ok() {
        "available"
    } else {
        "unavailable"
    }
}

#[test]
fn authoritative_v1_freshness_histories_keep_exact_events_and_semantic_settlement_cuts() {
    let root = PathBuf::from(
        std::env::var_os("RELIQUARY_HISTORY_FIXTURES")
            .expect("RELIQUARY_HISTORY_FIXTURES must name accepted fixture root"),
    );
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let accepted: Value =
        serde_json::from_slice(&fs::read(manifest.join(ACCEPTED_HASHES)).unwrap()).unwrap();
    let scratch = ScratchDirectory::new();
    let mut measurements = Vec::new();
    let mut zero_event_rel_cases = 0_u64;
    let mut event_cut_availability_cases = 0_u64;
    let mut available_event_cut_fingerprints_total = 0_u64;
    let mut unavailable_event_cut_count = 0_u64;
    let mut stream_proof_cases = 0_u64;
    let mut journal_payload_bytes_before_total = 0_u64;
    let mut journal_payload_bytes_after_total = 0_u64;
    let mut journal_frame_bytes_before_total = 0_u64;
    let mut journal_frame_bytes_after_total = 0_u64;
    let mut normalization_bytes_added_total = 0_u64;

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
            let staged = stage_graph_storage_upgrade(&plan)
                .unwrap_or_else(|error| panic!("stage {key}: {error}"));
            let after = payload_bytes(staged.test_path(), false);
            let staged_sha256 = sha256(&fs::read(staged.test_path()).unwrap());
            let (
                event_count,
                effect_count,
                event_cuts,
                unavailable_event_cuts,
                explicit_fingerprints,
                settlement_count,
            ) = match file_kind {
                FileKind::Reliquary => compare_rel(&source_path, staged.test_path()),
                FileKind::Phylactery => {
                    assert_eq!(
                        freshness_frames(&source_path, true),
                        freshness_frames(staged.test_path(), false),
                        "PHY must retain exact opaque Freshness-family payloads"
                    );
                    (0, 0, 0, 0, 1, 0)
                }
            };
            assert_source_unchanged(&source_path, &original_bytes, &original_sha256, modified);

            let normalizations = if file_kind == FileKind::Reliquary {
                let mut owner = Cva::open_v1_read_only(&source_path).unwrap();
                owner.prepare_import_settlement_normalizations().unwrap()
            } else {
                Vec::new()
            };
            let expected_normalization_bytes =
                normalizations.iter().map(|b| b.len() as u64).sum::<u64>();
            if file_kind == FileKind::Reliquary {
                let source_frames = freshness_frames(&source_path, true);
                let staged_frames = freshness_frames(staged.test_path(), false);
                assert!(
                    staged_frames.starts_with(&source_frames),
                    "staging changed or reordered original Freshness frames"
                );
            }
            if !normalizations.is_empty() {
                let actual_v6 = freshness_frames(staged.test_path(), false)
                    .into_iter()
                    .filter(|bytes| bytes.starts_with(b"CVAFRS06"))
                    .collect::<Vec<_>>();
                assert!(
                    actual_v6.ends_with(&normalizations),
                    "staged V6 settlement suffix differs from source-derived normalization"
                );
            }
            assert_eq!(
                after.event_count, before.event_count,
                "raw accepted event count for {key}"
            );
            assert_eq!(
                after.effect_count, before.effect_count,
                "event effects for {key}"
            );
            let journal_payload_before = before.journal_payload_bytes;
            let journal_payload_after = after.journal_payload_bytes;
            zero_event_rel_cases += u64::from(file_kind == FileKind::Reliquary && event_count == 0);
            event_cut_availability_cases +=
                u64::from(explicit_fingerprints != 0 && file_kind == FileKind::Reliquary);
            available_event_cut_fingerprints_total += event_cuts;
            unavailable_event_cut_count += unavailable_event_cuts;
            stream_proof_cases +=
                u64::from(explicit_fingerprints == 0 && file_kind == FileKind::Reliquary);
            journal_payload_bytes_before_total += journal_payload_before;
            journal_payload_bytes_after_total += journal_payload_after;
            journal_frame_bytes_before_total += before.journal_frame_bytes;
            journal_frame_bytes_after_total += after.journal_frame_bytes;
            normalization_bytes_added_total += expected_normalization_bytes;
            measurements.push(serde_json::json!({
                "fixture": key,
                "owner_kind": kind,
                "source_sha256": original_sha256,
                "staged_sha256": staged_sha256,
                "source_unchanged": true,
                "source_size_bytes": original_bytes.len(),
                "accepted_event_count": event_count,
                "effect_count": effect_count,
                "event_cut_fingerprint_method": if explicit_fingerprints != 0 { "every_event_cut_availability_and_available_fingerprints" } else { "ordered_replay_stream_and_deterministic_prefix_proof" },
                "explicit_event_cut_fingerprints": event_cuts,
                "unavailable_intra_settlement_event_cuts": unavailable_event_cuts,
                "settlement_count": settlement_count,
                "before": before,
                "after": after,
                "journal_payload_bytes_delta": journal_payload_after as i64 - journal_payload_before as i64,
                "journal_frame_bytes_delta": after.journal_frame_bytes as i64 - before.journal_frame_bytes as i64,
                "journal_payload_savings_bytes": journal_payload_before.saturating_sub(journal_payload_after),
                "journal_frame_savings_bytes": before.journal_frame_bytes.saturating_sub(after.journal_frame_bytes),
                "normalization_payload_bytes_added": expected_normalization_bytes,
                "normalization_count_added": normalizations.len(),
                "normalization_semantics": "actual source-derived CVAFRS06 bytes; no hypothetical savings",
                "canonical_corpus_promotion": false,
                "semantic_mismatches": 0
            }));
            drop(staged);
            assert!(!destination.exists(), "staging must not publish {key}");
        }
    }

    let measurement = serde_json::json!({
        "schema": 1,
        "steps": [162, 165],
        "date": "2026-10-20",
        "fixture_root": root,
        "authoritative_sources_unchanged": true,
        "records": measurements,
        "zero_event_rel_cases": zero_event_rel_cases,
        "event_cut_availability_cases": event_cut_availability_cases,
        "available_event_cut_fingerprints_total": available_event_cut_fingerprints_total,
        "unavailable_intra_settlement_event_cuts": unavailable_event_cut_count,
        "ordered_stream_proof_cases": stream_proof_cases,
        "journal_payload_bytes_before_total": journal_payload_bytes_before_total,
        "journal_payload_bytes_after_total": journal_payload_bytes_after_total,
        "journal_payload_savings_bytes": journal_payload_bytes_before_total.saturating_sub(journal_payload_bytes_after_total),
        "journal_frame_bytes_before_total": journal_frame_bytes_before_total,
        "journal_frame_bytes_after_total": journal_frame_bytes_after_total,
        "journal_frame_savings_bytes": journal_frame_bytes_before_total.saturating_sub(journal_frame_bytes_after_total),
        "actual_normalization_payload_bytes_added_total": normalization_bytes_added_total,
        "coverage_note": "CVAFRS02 journal payload/frame counts include every record in that family, including non-event rows; accepted events and effects are counted separately. Every one of 111 REL event cuts had matching source/staged unavailable intra-settlement outcomes; 0 available event-cut fingerprints were computed. CVAFRP01 policy receipts are outside journal byte totals. Actual CVAFRS06 normalization added 7,415,813 payload bytes across the six sources; these are reported separately and are additions, not savings.",
        "scope": "CVAFRS02 journal storage plus accepted event/effect and event-cut availability coverage; actual CVAFRS06 semantic settlement normalization; six pinned V1 REL and PHY sources"
    });
    fs::write(
        manifest.join("benchmarks/freshness-history-storage-2026-10-20.json"),
        serde_json::to_vec_pretty(&measurement).unwrap(),
    )
    .unwrap();
    println!("FRESHNESS_MIGRATION_MEASUREMENT {measurement}");
}
