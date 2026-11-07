use super::{
    MAX_CHAIN_PAGES,
    receipt::{PageReceipt, PageSnapshot, page_chain_schema_sha256},
    report::parse_page,
};
use crate::{
    cursor::{CURSOR_FAMILY, CURSOR_ORDER},
    evidence_encoding::hex,
};
use reliquary_owner_session::MemoryId;
use serde_json::{Value, json};

pub fn validate_memory_page_chain(reports: &[Value]) -> Result<Value, String> {
    if reports.is_empty() {
        return Err("memory_page_chain_has_no_pages".into());
    }
    if reports.len() > MAX_CHAIN_PAGES {
        return Err("memory_page_chain_exceeded_page_bound".into());
    }

    let mut prior: Option<PageReceipt> = None;
    let mut snapshot: Option<PageSnapshot> = None;
    let mut row_count = 0_u64;
    let mut first_id = None;
    let mut last_data_id = None;
    for (expected_index, report) in reports.iter().enumerate() {
        let current = parse_page(report)?;
        if current.page_index != expected_index as u64 {
            return Err("memory_page_chain_page_index_mismatch".into());
        }
        if snapshot
            .as_ref()
            .is_some_and(|value| value != &current.snapshot)
        {
            return Err("memory_page_chain_snapshot_mismatch".into());
        }
        if let Some(previous) = &prior {
            if previous.terminal_empty {
                return Err("memory_page_chain_page_after_terminal".into());
            }
            let expected_after = previous
                .ids
                .last()
                .copied()
                .ok_or("memory_page_chain_previous_page_empty")?;
            if current.after_id != Some(expected_after) {
                return Err("memory_page_chain_range_gap_or_overlap".into());
            }
            if current.previous_page_digest != Some(previous.page_digest) {
                return Err("memory_page_chain_predecessor_mismatch".into());
            }
        } else if current.after_id.is_some() || current.previous_page_digest.is_some() {
            return Err("memory_page_chain_first_page_not_rooted".into());
        }
        if current.terminal_empty && expected_index + 1 != reports.len() {
            return Err("memory_page_chain_terminal_not_last".into());
        }
        if current.terminal_empty && !current.ids.is_empty() {
            return Err("memory_page_chain_terminal_contains_rows".into());
        }
        if !current.terminal_empty && current.ids.is_empty() {
            return Err("memory_page_chain_empty_page_not_terminal".into());
        }
        if first_id.is_none() {
            first_id = current.ids.first().copied();
        }
        if let Some(id) = current.ids.last().copied() {
            last_data_id = Some(id);
        }
        row_count = row_count
            .checked_add(current.ids.len() as u64)
            .ok_or("memory_page_chain_row_count_overflow")?;
        snapshot.get_or_insert_with(|| current.snapshot.clone());
        prior = Some(current);
    }

    let last = prior.ok_or("memory_page_chain_has_no_pages")?;
    if !last.terminal_empty {
        return Err("memory_page_chain_missing_terminal_empty_page".into());
    }
    let snapshot = snapshot.ok_or("memory_page_chain_has_no_snapshot")?;
    Ok(json!({
        "schema_id": "reliquary-d02-memory-page-chain-summary-v1",
        "status": "INCOMPLETE",
        "internal_chain_consistency": "CONSISTENT",
        "producer_authenticated": false,
        "source_enumeration_proven": false,
        "reader_source_sha256": "INCOMPLETE",
        "reader_schema_sha256": hex(&page_chain_schema_sha256()),
        "maximum_chain_pages": MAX_CHAIN_PAGES.to_string(),
        "family": CURSOR_FAMILY,
        "order": CURSOR_ORDER,
        "owner_kind": snapshot.kind,
        "owner_scope": snapshot.scope,
        "owner_uuid": hex(&snapshot.owner_id),
        "candidate_sha256": hex(&snapshot.file_sha256),
        "history_generation_sha256": hex(&snapshot.history_generation),
        "memory_version": snapshot.memory_version.to_string(),
        "page_count_including_terminal": reports.len().to_string(),
        "row_count": row_count.to_string(),
        "first_id": first_id.map(|id: MemoryId| hex(&id.0)),
        "last_id": last_data_id.map(|id| hex(&id.0)),
        "range": {
            "first_page_after_id_exclusive": Value::Null,
            "last_data_id_inclusive": last_data_id.map(|id| hex(&id.0)),
            "terminal_empty_page_observed": true,
            "scope": "one_current_memory_version_only"
        },
        "historical_cut_inventory": "INCOMPLETE",
        "other_family_coverage": "INCOMPLETE",
        "authenticity_and_full_parity": "INCOMPLETE",
        "final_page_digest": hex(&last.page_digest),
        "scope_note": "This validates only that the supplied synthetic page receipts form an internally consistent ordered chain and include an empty terminal query. The public SHA-256 chain is not producer authentication and cannot prove a hostile or faulty exporter did not omit source rows and recompute receipts."
    }))
}
