//! Explicit read-only S3 regeneration audit against the frozen pre-retirement REL corpus.
//! The ignored test copies each source only through the settlement-frame boundary, reopens
//! that exact prefix, and invokes the production Import freeze/scorer without changing either
//! frozen input. Its JSON output is a measurement artifact, not an acceptance receipt.

use super::freeze;
use crate::Container;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn uleb_size(mut value: u64) -> usize {
    let mut size = 1;
    while value >= 0x80 {
        value >>= 7;
        size += 1;
    }
    size
}

fn record_wire_size(record: crate::freshness::FreshnessRecord) -> usize {
    2 + 1
        + usize::from(record.admitted_at_turn.is_some()) * 8
        + 1
        + usize::from(record.accounted_turn.is_some()) * 8
}

fn record_value(record: crate::freshness::FreshnessRecord) -> Value {
    json!({
        "score": record.score,
        "admitted_at_turn": record.admitted_at_turn,
        "accounted_turn": record.accounted_turn,
    })
}

fn scores_basis(
    initial: &BTreeMap<crate::MemoryId, crate::freshness::FreshnessRecord>,
    scores: &[crate::freshness::HistoricalScoreEvent],
) -> Result<(Value, usize), String> {
    let mut canonical_scores = scores.to_vec();
    canonical_scores.sort_by_key(|row| (row.recipient, row.position, row.event_id));
    let ids: Vec<_> = initial.keys().copied().collect();
    let id_indexes: BTreeMap<_, _> = ids
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index as u64))
        .collect();
    let mut positions = BTreeMap::new();
    let mut event_ids = BTreeSet::new();
    let mut rows = BTreeMap::<crate::MemoryId, Vec<crate::freshness::HistoricalScoreEvent>>::new();
    for row in &canonical_scores {
        if !id_indexes.contains_key(&row.recipient) {
            return Err(format!(
                "score recipient {} missing from base",
                hex(&row.recipient.0)
            ));
        }
        if positions
            .insert(row.event_id, row.position)
            .is_some_and(|previous| previous != row.position)
        {
            return Err(format!(
                "event {} has conflicting positions",
                hex(&row.event_id)
            ));
        }
        event_ids.insert(row.event_id);
        rows.entry(row.recipient).or_default().push(row.clone());
    }
    let mut events: Vec<_> = positions.into_iter().collect();
    events.sort_by_key(|(id, position)| (*position, *id));
    let event_indexes: BTreeMap<_, _> = events
        .iter()
        .enumerate()
        .map(|(index, (id, _))| (*id, index as u64))
        .collect();
    for (recipient, recipient_rows) in &mut rows {
        recipient_rows.sort_by_key(|row| (row.position, row.event_id));
        let mut seen = BTreeSet::new();
        if recipient_rows.iter().any(|row| !seen.insert(row.event_id)) {
            return Err(format!(
                "duplicate (recipient,event) row for {}",
                hex(&recipient.0)
            ));
        }
    }

    // Exact model for a standalone causal-basis section (not a candidate full settlement
    // codec): record key spine; one exact base record per key; ordered event table; then
    // recipient groups of delta-coded event ordinals and signed amounts.
    let mut bytes = uleb_size(ids.len() as u64) + ids.len() * 32;
    bytes += initial
        .values()
        .map(|record| record_wire_size(*record))
        .sum::<usize>();
    bytes += uleb_size(events.len() as u64);
    for (id, position) in &events {
        bytes += 32 + uleb_size(*position);
        let _ = id;
    }
    bytes += uleb_size(rows.len() as u64);
    let mut previous_recipient = None;
    for (recipient, recipient_rows) in &rows {
        let index = id_indexes[recipient];
        let delta = previous_recipient.map_or(index + 1, |previous| index - previous);
        bytes += uleb_size(delta) + uleb_size(recipient_rows.len() as u64);
        let mut previous_event = None;
        for row in recipient_rows {
            let event_index = event_indexes[&row.event_id];
            let event_delta =
                previous_event.map_or(event_index + 1, |previous| event_index - previous);
            let zigzag_amount =
                ((i32::from(row.amount) << 1) ^ (i32::from(row.amount) >> 31)) as u64;
            bytes += uleb_size(event_delta) + uleb_size(zigzag_amount);
            previous_event = Some(event_index);
        }
        previous_recipient = Some(index);
    }

    let initial_records = initial
        .iter()
        .map(|(id, record)| json!({"memory_id": hex(&id.0), "value": record_value(*record)}))
        .collect::<Vec<_>>();
    let scorer_rows = canonical_scores
        .iter()
        .map(|row| {
            json!({
                "recipient": hex(&row.recipient.0),
                "position": row.position,
                "event_id": hex(&row.event_id),
                "amount": row.amount,
            })
        })
        .collect::<Vec<_>>();
    let basis = json!({
        "wire_model": "ULEB sorted base MemoryId spine + exact FreshnessRecord values + (position,event_id) table + recipient-grouped delta event ordinals and zigzag amounts; estimate excludes settlement metadata and semantic proofs",
        "basis_bytes": bytes,
        "record_keys": ids.len(),
        "initial_record_value_bytes": initial.values().map(|record| record_wire_size(*record)).sum::<usize>(),
        "unique_scorer_events": event_ids.len(),
        "scorer_rows": canonical_scores.len(),
        "recipient_groups": rows.len(),
        "max_rows_per_recipient": rows.values().map(Vec::len).max().unwrap_or(0),
        "amount_min": canonical_scores.iter().map(|row| row.amount).min(),
        "amount_max": canonical_scores.iter().map(|row| row.amount).max(),
        "position_min": canonical_scores.iter().map(|row| row.position).min(),
        "position_max": canonical_scores.iter().map(|row| row.position).max(),
        "initial_records": initial_records,
        "ordered_scorer_rows": scorer_rows,
    });
    Ok((basis, bytes))
}

fn assert_path_hash(path: &Path, expected: &str) -> Vec<u8> {
    let bytes = fs::read(path).expect("read frozen source");
    assert_eq!(
        hex(&Sha256::digest(&bytes)),
        expected,
        "frozen source hash changed"
    );
    bytes
}

#[test]
#[ignore = "explicit bounded Phase 3 S3 native settlement regeneration; writes only repo-local temporary prefixes and JSON"]
fn explicit_phase3_s3_regenerates_frozen_settlements_from_pre_settlement_prefixes() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let baseline: Value = serde_json::from_slice(
        &fs::read(repo.join("benchmarks/dependency-convergence-phase3-baseline-2026-10-22.json"))
            .unwrap(),
    )
    .unwrap();
    let audit: Value = serde_json::from_slice(
        &fs::read(repo.join(
            "benchmarks/dependency-convergence-phase3-settlement-field-audit-2026-10-22.json",
        ))
        .unwrap(),
    )
    .unwrap();
    let audit_files: BTreeMap<_, _> = audit["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| (row["fixture"].as_str().unwrap().to_owned(), row))
        .collect();
    let report_dir = repo.join(".verification-tmp/phase3-s3-regeneration");
    fs::create_dir_all(&report_dir).unwrap();
    let mut source_hashes = serde_json::Map::new();
    for source in baseline["canonical_sources"].as_array().unwrap() {
        let fixture = source["fixture"].as_str().unwrap();
        let path = repo.join(".verification-tmp/phase3-before").join(fixture);
        let expected_hash = source["sha256"].as_str().unwrap();
        assert_path_hash(&path, expected_hash);
        source_hashes.insert(fixture.to_owned(), json!({"before": expected_hash}));
    }
    let mut outcomes = Vec::new();

    for source in baseline["canonical_sources"].as_array().unwrap() {
        let fixture = source["fixture"].as_str().unwrap();
        if !fixture.ends_with(".rel") {
            continue;
        }
        let path = repo.join(".verification-tmp/phase3-before").join(fixture);
        let expected_hash = source["sha256"].as_str().unwrap();
        let source_bytes = assert_path_hash(&path, expected_hash);
        let file_audit = audit_files[fixture];
        let settlement_row = &file_audit["settlements"][0];
        let frame_offset = settlement_row["frame_offset"].as_u64().unwrap() as usize;
        let payload_bytes = settlement_row["payload_bytes"].as_u64().unwrap() as usize;
        assert!(frame_offset >= 40 && frame_offset + 8 + payload_bytes <= source_bytes.len());
        let frame_length = u64::from_le_bytes(
            source_bytes[frame_offset..frame_offset + 8]
                .try_into()
                .unwrap(),
        );
        assert_eq!(
            frame_length as usize, payload_bytes,
            "settlement frame length changed"
        );
        let original_payload = &source_bytes[frame_offset + 8..frame_offset + 8 + payload_bytes];
        assert!(original_payload.starts_with(b"CVAFRS06"));
        assert_eq!(
            hex(&Sha256::digest(original_payload)),
            settlement_row["payload_sha256"].as_str().unwrap()
        );

        // The frozen source's whole-owner history acceptance is a separate
        // corpus oracle. Decode the exact authenticated frame here instead of
        // repeating an unrelated whole-file open before every prefix audit.
        let expected = crate::freshness_storage::HistoricalImportFreshnessSettlement::from_semantic(
            crate::freshness_storage::decode_phase3_frozen_settlement(original_payload).unwrap(),
        );
        let mut prefix_material = Sha256::new();
        prefix_material.update(std::process::id().to_le_bytes());
        prefix_material.update(fixture.as_bytes());
        let temp_name = format!("prefix-{}", &hex(&prefix_material.finalize())[..16]);
        let prefix_path = report_dir.join(temp_name);
        let mut prefix_file = File::create(&prefix_path).unwrap();
        prefix_file
            .write_all(&source_bytes[..frame_offset])
            .unwrap();
        prefix_file.sync_all().unwrap();
        drop(prefix_file);
        let prefix_size = fs::metadata(&prefix_path).unwrap().len();
        let prefix_hash = hex(&Sha256::digest(fs::read(&prefix_path).unwrap()));

        let result = (|| -> Result<Value, String> {
            let container = Container::open(&prefix_path).map_err(|error| error.to_string())?;
            let mut replay = crate::owner_lifecycle::ReliquaryReplay::read(container)
                .map_err(|error| error.to_string())?;
            let reconstruction = crate::history::reconstruct_reliquary(&mut replay)
                .map_err(|error| error.to_string())?;
            let (owner, _) = replay.into_owner(reconstruction);
            if !owner.freshness.import_settlements().is_empty() {
                return Err(
                    "pre-settlement prefix unexpectedly contains an accepted settlement".into(),
                );
            }
            // Resolve every original event leaf against the retained journal,
            // independently of the settlement being replaced. This proves
            // recipient/digest/turn references can survive without copied sets.
            owner.freshness.validate_semantic_import_evidence(
                expected
                    .semantic
                    .as_ref()
                    .ok_or("original lacks semantic scope")?,
            )?;
            let frozen = freeze(&owner).map_err(|error| error.to_string())?;
            let mut scores = frozen.native_scores().map_err(|error| error.to_string())?;
            scores.extend(frozen.retained.clone());
            let (basis, basis_bytes) = scores_basis(&frozen.initial, &scores)?;
            let prepared = frozen.prepare(&scores).map_err(|error| error.to_string())?;
            let mut expected_without_records = expected.clone();
            expected_without_records.records.clear();
            let mut actual_without_records = prepared.settlement.clone();
            actual_without_records.records.clear();
            let mut record_diffs = Vec::new();
            let all_ids: BTreeSet<_> = expected
                .records
                .keys()
                .chain(prepared.settlement.records.keys())
                .copied()
                .collect();
            for id in all_ids {
                let old = expected.records.get(&id).copied();
                let new = prepared.settlement.records.get(&id).copied();
                if old != new {
                    record_diffs.push(json!({
                        "memory_id": hex(&id.0),
                        "original": old.map(record_value),
                        "regenerated": new.map(record_value),
                    }));
                }
            }
            let mut metadata_differences = Vec::new();
            macro_rules! compare_field {
                ($($field:ident),* $(,)?) => { $(if expected.$field != prepared.settlement.$field { metadata_differences.push(stringify!($field)); })* };
            }
            compare_field!(
                owner,
                import_id,
                start_evidence_digest,
                corpus_digest,
                manifest_digest,
                input_digest,
                cohort,
                base_cut,
                settlement_cut,
                global_version,
                archive_version,
                graph_version,
                memory_graph_version,
                superseded
            );
            if expected.semantic != prepared.settlement.semantic {
                metadata_differences.push("semantic_scope");
            }
            let expected_nonrecord_payload = expected_without_records.encode();
            let actual_nonrecord_payload = actual_without_records.encode();
            let original_scores_raw_bytes = expected.records.len() * 32
                + expected
                    .records
                    .values()
                    .map(|record| record_wire_size(*record))
                    .sum::<usize>()
                + 4;
            Ok(json!({
                "fixture": fixture,
                "source_sha256": expected_hash,
                "settlement_frame_offset": frame_offset,
                "original_settlement_payload_bytes": payload_bytes,
                "original_settlement_sha256": hex(&Sha256::digest(original_payload)),
                "prefix_bytes": prefix_size,
                "prefix_sha256": prefix_hash,
                "prefix_had_accepted_settlement": false,
                "generated_source_bytes": prepared.source_bytes,
                "generated_source_sha256": hex(&prepared.source_digest),
                "settlement_nonrecord_fields_equal": expected_without_records == actual_without_records,
                "nonrecord_field_differences": metadata_differences,
                "original_event_leaves_validated_against_prefix_journal": true,
                "settlement_nonrecord_wire_equal": expected_nonrecord_payload == actual_nonrecord_payload,
                "original_record_count": expected.records.len(),
                "regenerated_record_count": prepared.settlement.records.len(),
                "residual_record_count": record_diffs.len(),
                "residual_records": record_diffs,
                "original_record_field_bytes": original_scores_raw_bytes,
                "causal_basis": basis,
                "causal_basis_bytes": basis_bytes,
            }))
        })();
        outcomes.push(match result {
            Ok(value) => value,
            Err(error) => json!({
                "fixture": fixture,
                "source_sha256": expected_hash,
                "settlement_frame_offset": frame_offset,
                "prefix_bytes": prefix_size,
                "prefix_sha256": prefix_hash,
                "status": "regeneration_error",
                "error": error,
            }),
        });
        let outcome = outcomes.last().unwrap();
        eprintln!(
            "S3 prefix {}: {}",
            fixture,
            outcome["status"].as_str().unwrap_or("measured")
        );
        fs::write(
            report_dir.join("phase3-s3-regeneration-progress.json"),
            serde_json::to_vec_pretty(&outcomes).unwrap(),
        )
        .unwrap();
        let _ = fs::remove_file(prefix_path);
    }

    let failed_regenerations = outcomes
        .iter()
        .filter(|row| row.get("status").is_some())
        .count();
    let report = json!({
        "schema": 1,
        "purpose": "ignored test-only native S3 regeneration audit; no source or frozen corpus mutation",
        "baseline_source_commit": baseline["source_commit"],
        "source_identity_hashes": source_hashes,
        "fixtures": outcomes,
    });
    for source in baseline["canonical_sources"].as_array().unwrap() {
        let fixture = source["fixture"].as_str().unwrap();
        let path = repo.join(".verification-tmp/phase3-before").join(fixture);
        let expected_hash = source["sha256"].as_str().unwrap();
        assert_path_hash(&path, expected_hash);
        source_hashes
            .entry(fixture.to_owned())
            .and_modify(|value| value["after"] = json!(expected_hash));
    }
    let mut report = report;
    report["source_identity_hashes"] = json!(source_hashes);
    let report_path = report_dir.join("phase3-s3-regeneration-audit.json");
    fs::write(&report_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("S3 regeneration audit: {}", report_path.display());
    for outcome in report["fixtures"].as_array().unwrap() {
        println!(
            "{}: status={}, records={}, residuals={}, basis={} B",
            outcome["fixture"].as_str().unwrap_or("unknown"),
            outcome["status"].as_str().unwrap_or("measured"),
            outcome["original_record_count"].as_u64().unwrap_or(0),
            outcome["residual_record_count"].as_u64().unwrap_or(0),
            outcome["causal_basis_bytes"].as_u64().unwrap_or(0),
        );
    }
    assert_eq!(report["fixtures"].as_array().unwrap().len(), 3);
    assert_eq!(
        failed_regenerations, 0,
        "regeneration failures are measurements unavailable, not successful parity"
    );
}
