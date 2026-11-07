use crate::{
    cursor::{CURSOR_FAMILY, CursorPosition, decode_cursor, encode_cursor},
    page_chain::{PageReceipt, PageSnapshot},
};
use reliquary_owner_session::{
    MAX_MEMORY_ID_CURSOR_BYTES, MAX_MEMORY_ID_PAGE_COUNT, MAX_MEMORY_ID_PAGE_PAYLOAD_BYTES,
    ReadOnlyCurrentOwnerObserver,
};
use serde_json::{Value, json};

const MAX_MEMORY_PAGE_JSON_BYTES: usize = 10_240;

pub(crate) fn observe_memory_id_page(
    observer: &mut ReadOnlyCurrentOwnerObserver,
    cursor: Option<&str>,
) -> Result<Value, String> {
    let snapshot = PageSnapshot::from_observation(observer.observation());
    let position = cursor
        .map(|token| decode_cursor(token, &snapshot))
        .transpose()?;
    let (page_index, after_id, previous_page_digest) = match position {
        Some(CursorPosition {
            page_index,
            after,
            previous_page_digest,
        }) => (page_index, Some(after), Some(previous_page_digest)),
        None => (0, None, None),
    };
    let page = observer.memory_ids_page(after_id, MAX_MEMORY_ID_PAGE_COUNT)?;
    if page.payload_bytes != page.ids.len() * 32
        || page.payload_bytes > MAX_MEMORY_ID_PAGE_PAYLOAD_BYTES
    {
        return Err("memory_page_payload_size_mismatch".into());
    }
    let receipt = PageReceipt::new(
        &snapshot,
        page_index,
        after_id,
        previous_page_digest,
        &page.ids,
    );
    let next_cursor = match page.ids.last().copied() {
        Some(last_id) => Some(encode_cursor(
            &snapshot,
            page_index
                .checked_add(1)
                .ok_or("memory_cursor_page_index_overflow")?,
            last_id,
            receipt.page_digest,
        )?),
        None => None,
    };
    let ids = page
        .ids
        .iter()
        .map(|id| crate::evidence_encoding::hex(&id.0))
        .collect::<Vec<_>>();
    let mut value = json!({
        "family": CURSOR_FAMILY,
        "status": "INCOMPLETE",
        "coverage": "one_current_memory_identity_page_only",
        "memory_version": page.memory_version.to_string(),
        "ids": ids,
        "returned_count": page.ids.len().to_string(),
        "raw_payload_bytes": page.payload_bytes.to_string(),
        "maximum_items": MAX_MEMORY_ID_PAGE_COUNT.to_string(),
        "maximum_raw_payload_bytes": MAX_MEMORY_ID_PAGE_PAYLOAD_BYTES.to_string(),
        "cursor_maximum_bytes": MAX_MEMORY_ID_CURSOR_BYTES.to_string(),
        "next_cursor": next_cursor,
        "page_chain": receipt.to_json(),
        "revision_history": "INCOMPLETE",
        "bodies": "INCOMPLETE",
        "lifecycle_and_provenance": "INCOMPLETE"
    });
    value["serialized_page_bytes"] = json!("0");
    for _ in 0..3 {
        let encoded = serde_json::to_vec(&value).map_err(|_| "memory_page_json_encode_failed")?;
        value["serialized_page_bytes"] = json!(encoded.len().to_string());
    }
    let encoded = serde_json::to_vec(&value).map_err(|_| "memory_page_json_encode_failed")?;
    let reported_len = encoded.len().to_string();
    if encoded.len() > MAX_MEMORY_PAGE_JSON_BYTES
        || value["serialized_page_bytes"].as_str() != Some(reported_len.as_str())
    {
        return Err("memory_page_json_exceeded_bound".into());
    }
    if page_index == 0 && previous_page_digest.is_some() {
        return Err("memory_page_chain_first_page_has_predecessor".into());
    }
    if page_index > 0 && previous_page_digest.is_none() {
        return Err("memory_page_chain_continuation_missing_predecessor".into());
    }
    Ok(value)
}
