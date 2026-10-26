//! Mechanical physical frontier for clock-neutral frames at a global cut.
//!
//! This does not fingerprint, persist, or assign clocks to any frame. The
//! caller must hold the source owner stable while selecting the accepted prefix.
//! Its offsets are a physical Container scan boundary only, never semantic data.
use reliquary_container::Container;

pub(crate) fn frontier_end(source: &Container, global_cut: u64) -> Result<u64, String> {
    let source_head = source.latest_version();
    if global_cut > source_head {
        return Err(format!(
            "global cut {global_cut} exceeds source head {source_head}"
        ));
    }

    let source_end = source
        .physical_length()
        .map_err(|error| error.to_string())?;
    let mut cursor = source
        .physical_cursor_through(source_end)
        .map_err(|error| error.to_string())?;
    let mut frontier_end = source.header_len();
    let mut scanned_head = 0;
    while let Some(frame) = cursor.next_frame().map_err(|error| error.to_string())? {
        scanned_head = frame.latest_global;
        if scanned_head <= global_cut {
            frontier_end = frame.end;
        }
    }
    if scanned_head != source_head {
        return Err(format!(
            "source global head changed or differs during frontier scan: expected {source_head}, scanned {scanned_head}"
        ));
    }
    Ok(frontier_end)
}

#[path = "historical_global_frontier_tests.rs"]
mod tests;
