use d02_current_reader::{CandidateKind, probe_candidate_page, validate_memory_page_chain};
use reliquary_memory_core::{MemoryDraft, MemorySourceRef};
use reliquary_owner_session::{OwnerScope, RelWriter};
use serde_json::{Value, json};
use tempfile::{TempDir, tempdir};

#[test]
fn validates_129_id_chain_and_keeps_acceptance_incomplete() {
    let (_directory, pages) = synthetic_page_chain(129);
    assert_eq!(pages.len(), 3);
    let summary = validate_memory_page_chain(&pages).unwrap();

    assert_eq!(summary["internal_chain_consistency"], "CONSISTENT");
    assert_eq!(summary["status"], "INCOMPLETE");
    assert_eq!(summary["producer_authenticated"], false);
    assert_eq!(summary["source_enumeration_proven"], false);
    assert_eq!(summary["page_count_including_terminal"], "3");
    assert_eq!(summary["row_count"], "129");
    assert_eq!(summary["range"]["terminal_empty_page_observed"], true);
    assert_eq!(summary["historical_cut_inventory"], "INCOMPLETE");
    assert_eq!(summary["other_family_coverage"], "INCOMPLETE");
    assert_eq!(
        pages[0]["memory_observation"]["page_chain"]["page_index"],
        "0"
    );
    assert_eq!(
        pages[1]["memory_observation"]["page_chain"]["page_index"],
        "1"
    );
    assert_eq!(
        pages[2]["memory_observation"]["page_chain"]["page_index"],
        "2"
    );
    assert_eq!(
        pages[2]["memory_observation"]["page_chain"]["terminal_empty"],
        true
    );
}

#[test]
fn rejects_missing_reordered_duplicate_tampered_and_unterminated_chains() {
    let (_directory, pages) = synthetic_page_chain(129);
    let first = &pages[0];
    let second = &pages[1];
    let terminal = &pages[2];

    assert!(validate_memory_page_chain(&[first.clone(), terminal.clone()]).is_err());
    assert!(
        validate_memory_page_chain(&[second.clone(), first.clone(), terminal.clone()]).is_err()
    );
    assert!(
        validate_memory_page_chain(&[
            first.clone(),
            first.clone(),
            second.clone(),
            terminal.clone()
        ])
        .is_err()
    );
    assert!(validate_memory_page_chain(&[first.clone(), second.clone()]).is_err());

    let mut changed_rows = first.clone();
    changed_rows["memory_observation"]["ids"][0] = json!("ff".repeat(32));
    assert!(validate_memory_page_chain(&[changed_rows, second.clone(), terminal.clone()]).is_err());

    let mut broken_predecessor = second.clone();
    broken_predecessor["memory_observation"]["page_chain"]["previous_page_digest"] =
        json!("00".repeat(32));
    assert!(
        validate_memory_page_chain(&[first.clone(), broken_predecessor, terminal.clone()]).is_err()
    );

    let mut changed_context = second.clone();
    changed_context["target_clocks"]["memory_version"] = json!("999");
    assert!(
        validate_memory_page_chain(&[first.clone(), changed_context, terminal.clone()]).is_err()
    );

    let (_empty_directory, empty_pages) = synthetic_page_chain(0);
    assert_eq!(empty_pages.len(), 1);
    assert!(validate_memory_page_chain(&[empty_pages[0].clone(), first.clone()]).is_err());
}

#[test]
fn rejects_unsupported_probe_versions_and_owner_enums() {
    let (_directory, pages) = synthetic_page_chain(0);
    let page = &pages[0];

    let mut unsupported_schema = page.clone();
    unsupported_schema["schema_id"] = json!("reliquary-d02-current-reader-probe-v2");
    assert_eq!(
        validate_memory_page_chain(&[unsupported_schema]).unwrap_err(),
        "memory_page_chain_probe_schema_or_contract_unsupported"
    );

    let mut unsupported_contract = page.clone();
    unsupported_contract["contract_commit"] = json!("0f824a09");
    assert_eq!(
        validate_memory_page_chain(&[unsupported_contract]).unwrap_err(),
        "memory_page_chain_probe_schema_or_contract_unsupported"
    );

    let mut unsupported_kind = page.clone();
    unsupported_kind["owner"]["kind"] = json!("RELATIVE");
    assert_eq!(
        validate_memory_page_chain(&[unsupported_kind]).unwrap_err(),
        "memory_page_chain_owner_kind_or_scope_unsupported"
    );

    let mut unsupported_scope = page.clone();
    unsupported_scope["owner"]["scope"] = json!("not-a-scope");
    assert_eq!(
        validate_memory_page_chain(&[unsupported_scope]).unwrap_err(),
        "memory_page_chain_owner_kind_or_scope_unsupported"
    );

    let mut invalid_phy_scope = page.clone();
    invalid_phy_scope["owner"]["kind"] = json!("PHY");
    invalid_phy_scope["owner"]["scope"] = json!("project");
    assert_eq!(
        validate_memory_page_chain(&[invalid_phy_scope]).unwrap_err(),
        "memory_page_chain_owner_kind_or_scope_unsupported"
    );
}

#[test]
fn rejects_oversized_page_shapes_before_serialization() {
    let (_directory, pages) = synthetic_page_chain(0);
    let page = &pages[0];

    let mut oversized_ids = page.clone();
    oversized_ids["memory_observation"]["ids"] = json!(vec!["00".repeat(32); 129]);
    assert_eq!(
        validate_memory_page_chain(&[oversized_ids]).unwrap_err(),
        "memory_page_chain_ids_exceed_or_miss_page_bound"
    );

    let mut oversized_id_string = page.clone();
    oversized_id_string["memory_observation"]["ids"] = json!(["x".repeat(50_000)]);
    assert_eq!(
        validate_memory_page_chain(&[oversized_id_string]).unwrap_err(),
        "memory_page_chain_ids_exceed_or_miss_page_bound"
    );

    let mut nested_extra = page.clone();
    nested_extra["memory_observation"]["unrecognized"] = json!({"nested": ["x".repeat(50_000)]});
    assert_eq!(
        validate_memory_page_chain(&[nested_extra]).unwrap_err(),
        "memory_page_chain_memory_observation_schema_mismatch"
    );
}

#[test]
fn empty_population_has_an_empty_terminal_but_is_not_a_parity_pass() {
    let (_directory, pages) = synthetic_page_chain(0);
    let summary = validate_memory_page_chain(&pages).unwrap();

    assert_eq!(summary["status"], "INCOMPLETE");
    assert_eq!(summary["internal_chain_consistency"], "CONSISTENT");
    assert_eq!(summary["row_count"], "0");
    assert_eq!(summary["page_count_including_terminal"], "1");
    assert_eq!(summary["producer_authenticated"], false);
    assert_eq!(summary["source_enumeration_proven"], false);
}

fn synthetic_page_chain(count: usize) -> (TempDir, Vec<Value>) {
    let directory = tempdir().unwrap();
    let path = directory.path().join("synthetic-chain.rel");
    let owner = [0x6D; 16];
    let mut writer = RelWriter::create_current_path(&path, OwnerScope::Project, owner).unwrap();
    for index in 0..count {
        writer
            .publish_direct_memory("d02-chain-test", synthetic_memory(owner, index))
            .unwrap();
    }
    drop(writer);

    let mut pages = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let report = probe_candidate_page(CandidateKind::Rel, &path, cursor.as_deref());
        cursor = report["memory_observation"]["next_cursor"]
            .as_str()
            .map(ToOwned::to_owned);
        pages.push(report);
        if cursor.is_none() {
            break;
        }
        assert!(
            pages.len() <= 4,
            "synthetic population unexpectedly spans many pages"
        );
    }
    (directory, pages)
}

fn synthetic_memory(owner: [u8; 16], index: usize) -> MemoryDraft {
    let index = index as u64;
    let mut episode = [0_u8; 32];
    episode[..8].copy_from_slice(&index.to_le_bytes());
    MemoryDraft {
        title: format!("chain title {index}"),
        content: format!("synthetic chain body {index}"),
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
            principal_id: Some("d02-chain-test".into()),
            source_episode_id: episode,
            source_node_id: format!("source-{index}"),
            content_source_conversation_id: Some("synthetic-chain".into()),
            content_source_node_id: Some(format!("turn-{index}")),
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
        }),
        source_time_ns: Some(index as i64 + 1),
        temporal_inference: None,
        mutation_id: format!("chain-mutation-{index}"),
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
