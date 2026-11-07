//! Per-row commitments for the isolated F03 read-only exporter.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{cell::RefCell, collections::BTreeMap};

const MAX_ROWS: usize = 262_144;

pub(crate) struct RowCommitment {
    pub(crate) domain: String,
    pub(crate) family: String,
    pub(crate) key_sha256: [u8; 32],
    pub(crate) value_sha256: [u8; 32],
    pub(crate) canonical_bytes: usize,
    pub(crate) coordinate: BTreeMap<&'static str, String>,
    pub(crate) availability: &'static str,
    pub(super) anchor: super::membership::Anchor,
}

thread_local! {
    static ROWS: RefCell<Option<Vec<RowCommitment>>> = const { RefCell::new(None) };
}

struct CaptureGuard;
impl Drop for CaptureGuard {
    fn drop(&mut self) {
        ROWS.with(|rows| {
            rows.borrow_mut().take();
        });
    }
}

/// The historical proof API remains unchanged. Collection only occurs for the
/// calling export scope and is discarded if a native proof fails.
pub(crate) fn collect<T>(
    observe: impl FnOnce() -> Result<T, String>,
) -> Result<(T, Vec<RowCommitment>), String> {
    ROWS.with(|rows| {
        let mut state = rows.borrow_mut();
        if state.is_some() {
            return Err("nested historical row evidence collection".to_string());
        }
        *state = Some(Vec::new());
        Ok(())
    })?;
    let _guard = CaptureGuard;
    let result = observe()?;
    let rows = ROWS.with(|rows| rows.borrow_mut().take().unwrap_or_default());
    Ok((result, rows))
}

pub(crate) fn observe(
    domain: &str,
    family: &str,
    key: &[u8],
    value: &Value,
    canonical_value: &[u8],
) -> Result<(), String> {
    ROWS.with(|rows| {
        let mut state = rows.borrow_mut();
        let Some(rows) = state.as_mut() else {
            return Ok(());
        };
        if rows.len() >= MAX_ROWS {
            return Err("native record commitment limit exceeded".into());
        }
        let availability = match value.get("pruned") {
            Some(Value::Bool(true) | Value::Object(_)) => "pruned",
            _ if family == "terminal"
                || value.get("availability").and_then(Value::as_str) == Some("tombstoned") =>
            {
                "tombstoned"
            }
            _ if value.get("availability").and_then(Value::as_str) == Some("inert") => "inert",
            _ => "retained",
        };
        let coordinate = [
            "global_version",
            "archive_version",
            "first_applying_archive_version",
            "first_applying_global_version",
            "memory_version",
            "entity_version",
            "relationship_version",
            "graph_version",
            "ego_version",
            "vector_version",
            "source_turn",
            "rel_turn",
            "transaction_time_ns",
        ]
        .into_iter()
        .filter_map(|name| {
            let number = value.get(name)?.as_number()?;
            (number.is_u64() || number.is_i64()).then(|| (name, number.to_string()))
        })
        .collect();
        rows.push(RowCommitment {
            domain: domain.into(),
            family: family.into(),
            key_sha256: Sha256::digest(key).into(),
            value_sha256: Sha256::digest(canonical_value).into(),
            canonical_bytes: canonical_value.len(),
            coordinate,
            availability,
            anchor: super::membership::anchor(domain, family, key, value),
        });
        Ok(())
    })
}
