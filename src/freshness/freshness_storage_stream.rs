//! Exact, bounded-memory scan of the Freshness journal prefix.
//!
//! `prefix_end` is the physical byte end of the last receipt admitted to the
//! projection. Payloads beyond it are intentionally not decoded: this lets
//! open and append validation reason only about the already-accepted prefix.

use super::{JournalEntry, MAGIC, decode};
use crate::{Container, CvaError};
use std::path::Path;

pub(crate) fn scan_entries(
    path: &Path,
    prefix_end: u64,
    mut visitor: impl FnMut([u8; 16], JournalEntry) -> Result<(), String>,
) -> Result<(), String> {
    #[cfg(feature = "freshness-performance-trace")]
    let started = std::time::Instant::now();
    #[cfg(feature = "freshness-performance-trace")]
    let (mut bytes, mut chunks) = (0u64, 0u64);
    let mut container =
        Container::open_read_only_for_scan(path).map_err(|error| error.to_string())?;
    let mut callback_error: Option<String> = None;

    container
        .visit_payloads_through::<CvaError>(prefix_end, |_, payload| {
            #[cfg(feature = "freshness-performance-trace")]
            {
                bytes += payload.len() as u64;
                chunks += 1;
            }
            if callback_error.is_some() {
                return Ok(());
            }

            if payload.starts_with(super::import_checkpoint_storage::IMPORT_MAGIC) {
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
                    if let Err(error) = visitor(owner, entry) {
                        callback_error = Some(error);
                    }
                }
                Err(error) => callback_error = Some(error),
            }
            Ok(())
        })
        .map_err(|error| error.to_string())?;

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
