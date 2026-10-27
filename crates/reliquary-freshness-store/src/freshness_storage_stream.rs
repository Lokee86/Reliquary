//! Exact, bounded-memory scan of the Freshness journal prefix.
//!
//! `prefix_end` is the physical byte end of the last receipt admitted to the
//! projection. Payloads beyond it are intentionally not decoded: this lets
//! open and append validation reason only about the already-accepted prefix.

use super::{JournalEntry, MAGIC, decode};
use reliquary_container::PhysicalReadSource;

pub(crate) fn scan_entries(
    source: &PhysicalReadSource,
    prefix_end: u64,
    mut visitor: impl FnMut([u8; 16], JournalEntry) -> Result<(), String>,
) -> Result<(), String> {
    scan_entries_impl(source, None, prefix_end, |owner, _, entry| {
        visitor(owner, entry)
    })
}

/// Scan Freshness events whose physical frames fall after `prefix_start` and
/// through `prefix_end`. Earlier payload bytes are traversed to reach the
/// interval, but are not decoded as Freshness entries or retained by this scan.
pub(crate) fn scan_entries_in_interval(
    source: &PhysicalReadSource,
    prefix_start: u64,
    prefix_end: u64,
    mut visitor: impl FnMut([u8; 16], u64, JournalEntry) -> Result<(), String>,
) -> Result<(), String> {
    if prefix_start > prefix_end || prefix_end == u64::MAX {
        return Err("invalid physical Freshness scan interval".into());
    }
    scan_entries_impl(
        source,
        Some(prefix_start),
        prefix_end,
        |owner, frame_end, entry| visitor(owner, frame_end, entry),
    )
}

fn scan_entries_impl(
    source: &PhysicalReadSource,
    prefix_start: Option<u64>,
    prefix_end: u64,
    mut visitor: impl FnMut([u8; 16], u64, JournalEntry) -> Result<(), String>,
) -> Result<(), String> {
    // A rebuilding store starts with no accepted frames, before its first
    // scanned payload advances the durable journal boundary.
    if prefix_end == 0 && prefix_start.is_none() {
        return Ok(());
    }
    let mut start_boundary_seen = prefix_start.is_none_or(|start| start == source.header_len());
    let mut cursor = source
        .physical_cursor_through(prefix_end)
        .map_err(|error| error.to_string())?;
    while let Some(frame) = cursor.next_frame().map_err(|error| error.to_string())? {
        let payload = frame.payload.as_slice();
        if prefix_start.is_some_and(|start| start == frame.offset || start == frame.end) {
            start_boundary_seen = true;
        }
        if prefix_start.is_some_and(|start| frame.end <= start) {
            continue;
        }
        if payload.starts_with(super::semantic_settlement::SEMANTIC_SETTLEMENT_MAGIC) {
            continue;
        }
        if payload.starts_with(b"CVAFRS") && payload.get(..8) != Some(MAGIC.as_slice()) {
            return Err("unsupported legacy CVAFRS01 Freshness receipt".into());
        }
        if payload.get(..8) == Some(MAGIC.as_slice()) {
            let (owner, entry) = decode(payload)?;
            visitor(owner, frame.end, entry)?;
        }
    }

    if !start_boundary_seen {
        return Err("Freshness interval start is not a physical frame boundary".into());
    }

    Ok(())
}
