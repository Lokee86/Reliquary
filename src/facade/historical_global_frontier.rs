//! Mechanical physical frontier for clock-neutral frames at a global cut.
//!
//! This does not fingerprint, persist, or assign clocks to any frame. The
//! caller must hold the source owner stable while selecting the accepted prefix.
//! Its offsets are a V1 Container scan boundary only, never semantic data.
use crate::{Container, CvaError};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn frontier_end(source: &Container, global_cut: u64) -> Result<u64, String> {
    let source_head = source.latest_version();
    if global_cut > source_head {
        return Err(format!(
            "global cut {global_cut} exceeds source head {source_head}"
        ));
    }

    // Bound the scan to the accepted source byte end observed at entry. This
    // byte offset is only a replay boundary; it is not fingerprint input.
    let source_end = std::fs::metadata(source.path())
        .map_err(|error| error.to_string())?
        .len();
    let mut scan =
        Container::open_read_only_for_scan(source.path()).map_err(|error| error.to_string())?;
    let mut next_version = 1_u64;
    let mut transaction_times = BTreeMap::new();
    let mut standalone_reservations = BTreeSet::new();
    let mut frontier_end = scan.header_len;
    let mut position = scan.header_len;
    scan.visit_payloads_through::<CvaError>(source_end, |_object, payload| {
        crate::container_version::observe_version_payload_state(
            &mut next_version,
            &mut transaction_times,
            &mut standalone_reservations,
            payload,
        )?;
        // V1 frames are contiguous `[u64 length][payload]` chunks; accumulate
        // their accepted byte positions without exposing ObjectRef internals.
        position = position
            .checked_add(8)
            .and_then(|value| value.checked_add(u64::try_from(payload.len()).ok()?))
            .ok_or(crate::ContainerError::ChunkTooLarge)?;
        if next_version.saturating_sub(1) <= global_cut {
            frontier_end = position;
        }
        Ok(())
    })
    .map_err(|error| error.to_string())?;

    let scanned_head = next_version.saturating_sub(1);
    if scanned_head != source_head {
        return Err(format!(
            "source global head changed or differs during frontier scan: expected {source_head}, scanned {scanned_head}"
        ));
    }
    Ok(frontier_end)
}

#[path = "historical_global_frontier_tests.rs"]
mod tests;
