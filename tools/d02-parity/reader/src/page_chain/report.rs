use super::receipt::{PAGE_CHAIN_SCHEMA_ID, PageReceipt, PageSnapshot, page_chain_schema_sha256};
use crate::{
    cursor::{CURSOR_FAMILY, CURSOR_ORDER, decode_cursor},
    evidence_encoding::{hex, parse_hex},
    reader::{CONTRACT_COMMIT, PROBE_SCHEMA_ID},
};
use reliquary_owner_session::{
    MAX_MEMORY_ID_PAGE_COUNT, MAX_MEMORY_ID_PAGE_PAYLOAD_BYTES, MemoryId,
};
use serde_json::{Map, Value};
mod helpers;
mod shape;
use helpers::{
    object_at, optional_hex_digest, optional_hex_id, require_true, string_at, string_at_value,
};
use shape::validate_memory_page_shape;

pub(super) fn parse_page(report: &Value) -> Result<PageReceipt, String> {
    if string_at(report, &["schema_id"])? != PROBE_SCHEMA_ID
        || string_at(report, &["contract_commit"])? != CONTRACT_COMMIT
    {
        return Err("memory_page_chain_probe_schema_or_contract_unsupported".into());
    }
    require_true(report, "same_handle_observation")?;
    require_true(report, "candidate_unchanged")?;
    require_true(report, "path_matches_open_file_before")?;
    require_true(report, "path_matches_open_file_after")?;
    require_true(report, "physical_file_identity_unchanged")?;
    require_true(report, "requested_kind_matches_observed")?;
    if string_at(report, &["status"])? != "INCOMPLETE"
        || string_at(report, &["evidence_scope"])? != "reader_probe_not_parity_evidence"
        || string_at(report, &["reason"])? != ""
    {
        return Err("memory_page_chain_report_not_clean_incomplete_probe".into());
    }
    let candidate_before = string_at(report, &["candidate_sha256_before"])?;
    if candidate_before.len() != 64
        || candidate_before != string_at(report, &["candidate_sha256_after"])?
    {
        return Err("memory_page_chain_candidate_hash_changed".into());
    }
    let kind = string_at(report, &["owner", "kind"])?;
    let scope = string_at(report, &["owner", "scope"])?;
    if !matches!(kind, "REL" | "PHY")
        || !matches!(
            scope,
            "unspecified" | "organization" | "project" | "connection"
        )
        || (kind == "PHY" && scope != "unspecified")
    {
        return Err("memory_page_chain_owner_kind_or_scope_unsupported".into());
    }
    let snapshot = PageSnapshot {
        kind: kind.to_owned(),
        scope: scope.to_owned(),
        owner_id: parse_hex::<16>(string_at(report, &["owner", "owner_uuid"])?)
            .ok_or("memory_page_chain_owner_id_invalid")?,
        file_sha256: parse_hex::<32>(candidate_before)
            .ok_or("memory_page_chain_candidate_hash_invalid")?,
        history_generation: parse_hex::<32>(string_at(
            report,
            &["target_clocks", "history_generation_sha256"],
        )?)
        .ok_or("memory_page_chain_generation_invalid")?,
        memory_version: bounded_report_string(report, &["target_clocks", "memory_version"], 20)?
            .parse()
            .map_err(|_| "memory_page_chain_memory_version_invalid")?,
    };
    if string_at(report, &["observed_owner_kind"])? != snapshot.kind {
        return Err("memory_page_chain_owner_kind_mismatch".into());
    }
    let memory = object_at(report, &["memory_observation"])?;
    validate_memory_page_shape(memory)?;
    let expected_memory_version = snapshot.memory_version.to_string();
    if string_at_value(memory, "status")? != "INCOMPLETE"
        || string_at_value(memory, "family")? != CURSOR_FAMILY
        || string_at_value(memory, "memory_version")? != expected_memory_version.as_str()
    {
        return Err("memory_page_chain_memory_observation_mismatch".into());
    }
    let chain = memory
        .get("page_chain")
        .and_then(Value::as_object)
        .ok_or("memory_page_chain_receipt_missing")?;
    let expected_generation = hex(&snapshot.history_generation);
    if string_at_value(chain, "schema_id")? != PAGE_CHAIN_SCHEMA_ID
        || string_at_value(chain, "schema_sha256")? != hex(&page_chain_schema_sha256())
        || string_at_value(chain, "history_generation_sha256")? != expected_generation.as_str()
        || string_at_value(chain, "memory_version")? != expected_memory_version.as_str()
        || string_at_value(chain, "family")? != CURSOR_FAMILY
        || string_at_value(chain, "order")? != CURSOR_ORDER
    {
        return Err("memory_page_chain_receipt_binding_mismatch".into());
    }

    let page_index = string_at_value(chain, "page_index")?
        .parse::<u64>()
        .map_err(|_| "memory_page_chain_page_index_invalid")?;
    let after_id = optional_hex_id(chain, "after_id_exclusive")?;
    let previous_page_digest = optional_hex_digest(chain, "previous_page_digest")?;
    let page_digest = parse_hex::<32>(string_at_value(chain, "page_digest")?)
        .ok_or("memory_page_chain_page_digest_invalid")?;
    let ids = memory
        .get("ids")
        .and_then(Value::as_array)
        .ok_or("memory_page_chain_ids_missing")?
        .iter()
        .map(|value| {
            parse_hex::<32>(value.as_str().ok_or("memory_page_chain_id_invalid")?)
                .map(MemoryId)
                .ok_or("memory_page_chain_id_invalid")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let declared_count = string_at_value(chain, "returned_count")?
        .parse::<usize>()
        .map_err(|_| "memory_page_chain_count_invalid")?;
    let declared_bytes = string_at_value(chain, "raw_payload_bytes")?
        .parse::<usize>()
        .map_err(|_| "memory_page_chain_payload_bytes_invalid")?;
    let terminal_empty = chain
        .get("terminal_empty")
        .and_then(Value::as_bool)
        .ok_or("memory_page_chain_terminal_marker_invalid")?;
    let expected_count = declared_count.to_string();
    let expected_bytes = declared_bytes.to_string();
    if declared_count != ids.len()
        || ids.len() > MAX_MEMORY_ID_PAGE_COUNT
        || declared_bytes
            != ids
                .len()
                .checked_mul(32)
                .ok_or("memory_page_chain_size_overflow")?
        || declared_bytes > MAX_MEMORY_ID_PAGE_PAYLOAD_BYTES
        || string_at_value(memory, "returned_count")? != expected_count.as_str()
        || string_at_value(memory, "raw_payload_bytes")? != expected_bytes.as_str()
    {
        return Err("memory_page_chain_count_or_size_mismatch".into());
    }
    if ids.windows(2).any(|pair| pair[0] >= pair[1])
        || after_id.is_some_and(|after| ids.first().is_some_and(|first| *first <= after))
    {
        return Err("memory_page_chain_page_order_mismatch".into());
    }
    if optional_hex_id(chain, "last_id")? != ids.last().copied() || terminal_empty != ids.is_empty()
    {
        return Err("memory_page_chain_page_bounds_mismatch".into());
    }

    let receipt = PageReceipt::new(&snapshot, page_index, after_id, previous_page_digest, &ids);
    if receipt.page_digest != page_digest {
        return Err("memory_page_chain_digest_mismatch".into());
    }
    validate_next_cursor(memory, &receipt)?;
    Ok(receipt)
}

fn bounded_report_string<'a>(
    report: &'a Value,
    path: &[&str],
    maximum_bytes: usize,
) -> Result<&'a str, String> {
    match path.iter().try_fold(report, |value, part| value.get(*part)) {
        Some(Value::String(value)) if value.len() <= maximum_bytes => Ok(value),
        _ => Err("memory_page_chain_bounded_report_string_invalid".into()),
    }
}

fn validate_next_cursor(memory: &Map<String, Value>, receipt: &PageReceipt) -> Result<(), String> {
    match (receipt.terminal_empty, memory.get("next_cursor")) {
        (true, Some(Value::Null)) => Ok(()),
        (false, Some(Value::String(token))) => {
            let cursor = decode_cursor(token, &receipt.snapshot)?;
            let expected_index = receipt
                .page_index
                .checked_add(1)
                .ok_or("memory_page_chain_page_index_overflow")?;
            if cursor.page_index != expected_index
                || Some(cursor.after) != receipt.ids.last().copied()
                || cursor.previous_page_digest != receipt.page_digest
            {
                return Err("memory_page_chain_next_cursor_mismatch".into());
            }
            Ok(())
        }
        _ => Err("memory_page_chain_continuation_marker_mismatch".into()),
    }
}
