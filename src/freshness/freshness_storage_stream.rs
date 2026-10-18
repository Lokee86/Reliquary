//! Exact, bounded-memory scan of the Freshness journal prefix.
//!
//! `prefix_end` is the physical byte end of the last receipt admitted to the
//! projection. Payloads beyond it are intentionally not decoded: this lets
//! open and append validation reason only about the already-accepted prefix.

use super::{JournalEntry, MAGIC, decode};
use crate::{Container, CvaError};
use std::path::Path;

const CONTAINER_CHUNK_HEADER_LEN: u64 = 8;

pub(crate) fn scan_entries(
    path: &Path,
    prefix_end: u64,
    mut visitor: impl FnMut([u8; 16], JournalEntry) -> Result<(), String>,
) -> Result<(), String> {
    scan_entries_impl(path, None, prefix_end, |owner, _, entry| {
        visitor(owner, entry)
    })
}

/// Scan Freshness events whose physical frames fall after `prefix_start` and
/// through `prefix_end`. Earlier payload bytes are traversed to reach the
/// interval, but are not decoded as Freshness entries or retained by this scan.
pub(crate) fn scan_entries_in_interval(
    path: &Path,
    prefix_start: u64,
    prefix_end: u64,
    mut visitor: impl FnMut([u8; 16], u64, JournalEntry) -> Result<(), String>,
) -> Result<(), String> {
    if prefix_start > prefix_end || prefix_end == u64::MAX {
        return Err("invalid physical Freshness scan interval".into());
    }
    scan_entries_impl(
        path,
        Some(prefix_start),
        prefix_end,
        |owner, frame_end, entry| visitor(owner, frame_end, entry),
    )
}

fn scan_entries_impl(
    path: &Path,
    prefix_start: Option<u64>,
    prefix_end: u64,
    mut visitor: impl FnMut([u8; 16], u64, JournalEntry) -> Result<(), String>,
) -> Result<(), String> {
    #[cfg(feature = "freshness-performance-trace")]
    let started = std::time::Instant::now();
    #[cfg(feature = "freshness-performance-trace")]
    let (mut bytes, mut chunks) = (0u64, 0u64);
    let mut container =
        Container::open_read_only_for_scan(path).map_err(|error| error.to_string())?;
    let mut callback_error: Option<String> = None;
    let mut start_boundary_seen = prefix_start.is_none_or(|start| start == container.header_len);

    container
        .visit_payloads_through::<CvaError>(prefix_end, |object, payload| {
            #[cfg(feature = "freshness-performance-trace")]
            {
                bytes += payload.len() as u64;
                chunks += 1;
            }
            if callback_error.is_some() {
                return Ok(());
            }

            let bytes = object.legacy_bytes();
            let frame_start = u64::from_le_bytes(bytes[..8].try_into().unwrap());
            let frame_len = u64::from_le_bytes(bytes[8..].try_into().unwrap());
            let Some(frame_end) = frame_start
                .checked_add(CONTAINER_CHUNK_HEADER_LEN)
                .and_then(|header_end| header_end.checked_add(frame_len))
            else {
                callback_error = Some("Freshness physical frame extent overflow".into());
                return Ok(());
            };
            if prefix_start.is_some_and(|start| start == frame_start || start == frame_end) {
                start_boundary_seen = true;
            }
            if prefix_start.is_some_and(|start| frame_end <= start) {
                return Ok(());
            }

            if payload.starts_with(super::import_settlement_storage::SETTLEMENT_MAGIC)
                || payload.starts_with(super::import_settlement_storage::LEGACY_CHECKPOINT_MAGIC)
            {
                return Ok(());
            }
            if payload.starts_with(b"CVAFRS") && payload.get(..8) != Some(MAGIC.as_slice()) {
                callback_error = Some("unsupported legacy CVAFRS01 Freshness receipt".into());
                return Ok(());
            }
            if payload.get(..8) != Some(MAGIC.as_slice()) {
                return Ok(());
            }

            match decode(payload) {
                Ok((owner, entry)) => {
                    if let Err(error) = visitor(owner, frame_end, entry) {
                        callback_error = Some(error);
                    }
                }
                Err(error) => callback_error = Some(error),
            }
            Ok(())
        })
        .map_err(|error| error.to_string())?;

    if !start_boundary_seen {
        return Err("Freshness interval start is not a physical frame boundary".into());
    }

    #[cfg(feature = "freshness-performance-trace")]
    crate::freshness_trace::record(
        "freshness.history.scan",
        started.elapsed().as_nanos(),
        crate::freshness_trace::Counts {
            payload_bytes: Some(bytes),
            chunks_visited: Some(chunks),
            ..Default::default()
        },
    );
    callback_error.map_or(Ok(()), Err)
}
