use d02_current_reader::{CandidateKind, probe_candidate, probe_candidate_page};
use reliquary_memory_core::{MemoryDraft, MemorySourceRef};
use reliquary_owner_session::{OwnerScope, PhyWriter, RelWriter};
use serde_json::{Value, json};
use std::{fs, path::Path};
use tempfile::tempdir;

fn assert_probe_is_incomplete(report: &serde_json::Value) {
    assert_eq!(report["schema_id"], "reliquary-d02-current-reader-probe-v2");
    assert_eq!(report["evidence_scope"], "reader_probe_not_parity_evidence");
    assert_eq!(report["status"], "INCOMPLETE");
    assert_eq!(report["semantic_rows"], json!([]));
    assert_eq!(report["semantic_row_count"], "0");
    assert_eq!(report["semantic_export_state"], "INCOMPLETE");
    assert_eq!(report["historical_proof_used_as_export_gate"], false);
    assert!(report["native_historical_proof"].is_null());
    assert_eq!(
        report["total_current_owner_replay_bounded"], false,
        "the current-only owner replay itself is not bounded by the page size"
    );
    let families = report["family_coverage"].as_array().unwrap();
    assert_eq!(families.len(), 5);
    assert!(
        families
            .iter()
            .all(|family| family["status"] == "INCOMPLETE")
    );
    assert_eq!(
        report["cut_coverage"]["historical_cut_inventory"],
        "INCOMPLETE"
    );
}

fn assert_final_hash_unchanged(report: &Value) {
    assert_eq!(report["candidate_unchanged"], true);
    assert_eq!(
        report["candidate_sha256_before"],
        report["candidate_sha256_after"]
    );
    assert_eq!(report["path_matches_open_file_before"], true);
    assert_eq!(report["path_matches_open_file_after"], true);
}

#[test]
fn rel_current_only_probe_observes_target_clock_and_one_empty_memory_page() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("synthetic-rel.owner");
    let writer = RelWriter::create_current_path(&path, OwnerScope::Project, [0x11; 16]).unwrap();
    drop(writer);
    let before = fs::read(&path).unwrap();

    let report = probe_candidate(CandidateKind::Rel, &path);

    assert_probe_is_incomplete(&report);
    assert_eq!(report["candidate_kind"], "REL");
    assert_eq!(report["observed_owner_kind"], "REL");
    assert_eq!(
        report["owner"]["owner_uuid"],
        "11111111111111111111111111111111"
    );
    assert_eq!(report["owner"]["scope"], "project");
    assert_eq!(report["same_handle_observation"], true);
    assert_eq!(report["target_clocks"]["memory_version"], "0");
    assert_eq!(report["memory_observation"]["returned_count"], "0");
    assert_eq!(report["memory_observation"]["raw_payload_bytes"], "0");
    assert_eq!(report["memory_observation"]["next_cursor"], Value::Null);
    assert_final_hash_unchanged(&report);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn phy_current_only_probe_is_read_only_and_marks_other_families_incomplete() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("synthetic-phy.owner");
    let writer = PhyWriter::create_path(&path, [0x22; 16]).unwrap();
    drop(writer);
    let before = fs::read(&path).unwrap();

    let report = probe_candidate(CandidateKind::Phy, &path);

    assert_probe_is_incomplete(&report);
    assert_eq!(report["candidate_kind"], "PHY");
    assert_eq!(report["observed_owner_kind"], "PHY");
    assert_eq!(
        report["owner"]["owner_uuid"],
        "22222222222222222222222222222222"
    );
    assert_eq!(report["owner"]["scope"], "unspecified");
    assert_eq!(report["same_handle_observation"], true);
    assert_eq!(report["target_clocks"]["memory_version"], "0");
    assert_final_hash_unchanged(&report);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn wrong_requested_kind_stays_incomplete_without_page_or_mutation() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("synthetic-rel.owner");
    let writer = RelWriter::create_current_path(&path, OwnerScope::Project, [0x33; 16]).unwrap();
    drop(writer);
    let before = fs::read(&path).unwrap();

    let report = probe_candidate(CandidateKind::Phy, Path::new(&path));

    assert_probe_is_incomplete(&report);
    assert_eq!(report["candidate_kind"], "PHY");
    assert_eq!(report["observed_owner_kind"], "REL");
    assert_eq!(report["requested_kind_matches_observed"], false);
    assert_eq!(report["memory_observation"], Value::Null);
    assert_eq!(report["reason"], "requested_candidate_kind_mismatch");
    assert_final_hash_unchanged(&report);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn memory_pages_are_count_and_byte_bounded_and_cursor_bound() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("synthetic-many-memories.rel");
    let owner = [0x44; 16];
    let mut writer = RelWriter::create_current_path(&path, OwnerScope::Project, owner).unwrap();
    for index in 0..129 {
        writer
            .publish_direct_memory("d02-test", synthetic_memory(owner, index))
            .unwrap();
    }
    drop(writer);
    let before = fs::read(&path).unwrap();

    let first = probe_candidate_page(CandidateKind::Rel, &path, None);
    assert_probe_is_incomplete(&first);
    assert_eq!(first["memory_observation"]["returned_count"], "128");
    assert_eq!(first["memory_observation"]["raw_payload_bytes"], "4096");
    assert_eq!(first["memory_observation"]["maximum_items"], "128");
    assert_eq!(
        first["memory_observation"]["maximum_raw_payload_bytes"],
        "4096"
    );
    assert!(
        first["memory_observation"]["serialized_page_bytes"]
            .as_str()
            .unwrap()
            .parse::<usize>()
            .unwrap()
            <= 10_240
    );
    let first_ids = first["memory_observation"]["ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(first_ids.len(), 128);
    assert!(first_ids.windows(2).all(|pair| pair[0] < pair[1]));
    let cursor = first["memory_observation"]["next_cursor"]
        .as_str()
        .expect("first full page provides a continuation cursor");
    assert!(cursor.len() <= 512);
    assert_final_hash_unchanged(&first);

    let second = probe_candidate_page(CandidateKind::Rel, &path, Some(cursor));
    assert_probe_is_incomplete(&second);
    assert_eq!(second["memory_observation"]["returned_count"], "1");
    assert_eq!(second["memory_observation"]["raw_payload_bytes"], "32");
    assert_eq!(second["memory_observation"]["next_cursor"], Value::Null);
    let second_id = second["memory_observation"]["ids"][0].as_str().unwrap();
    assert!(first_ids.last().unwrap().as_str() < second_id);
    assert_final_hash_unchanged(&second);
    assert_eq!(fs::read(&path).unwrap(), before);

    let other = directory.path().join("different-owner.rel");
    let other_writer =
        RelWriter::create_current_path(&other, OwnerScope::Project, [0x45; 16]).unwrap();
    drop(other_writer);
    let rejected = probe_candidate_page(CandidateKind::Rel, &other, Some(cursor));
    assert_probe_is_incomplete(&rejected);
    assert_eq!(rejected["memory_observation"], Value::Null);
    assert_eq!(rejected["reason"], "memory_cursor_binding_mismatch");
    assert_final_hash_unchanged(&rejected);
}

fn synthetic_memory(owner: [u8; 16], index: usize) -> MemoryDraft {
    let index = index as u64;
    let mut episode = [0_u8; 32];
    episode[..8].copy_from_slice(&index.to_le_bytes());
    MemoryDraft {
        title: format!("synthetic title {index}"),
        content: format!("synthetic current body {index}"),
        category: "fact".into(),
        memory_type: "semantic".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
        lifecycle_state: "current".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: Some(MemorySourceRef {
            owner_id: hex(&owner),
            principal_id: Some("d02-test".into()),
            source_episode_id: episode,
            source_node_id: format!("source-{index}"),
            content_source_conversation_id: Some("synthetic-conversation".into()),
            content_source_node_id: Some(format!("turn-{index}")),
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
        }),
        source_time_ns: Some(index as i64 + 1),
        temporal_inference: None,
        mutation_id: format!("synthetic-mutation-{index}"),
        created_at_ns: index as i64 + 1,
        updated_at_ns: index as i64 + 1,
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}
