//! Physical frontier selection over one immutable accepted owner snapshot.
use crate::owner_lifecycle::HistorySource;

pub(crate) fn frontier_end(source: &HistorySource<'_>, global_cut: u64) -> Result<u64, String> {
    let source_head = source.latest_version();
    if global_cut > source_head {
        return Err(format!(
            "global cut {global_cut} exceeds source head {source_head}"
        ));
    }
    let mut frontier = source.header_len();
    let mut scanned_head = 0;
    let mut cursor = source.cursor().map_err(|error| error.to_string())?;
    while let Some(frame) = cursor.next_frame().map_err(|error| error.to_string())? {
        // Container alone validates physical clocks and grouped ranges. A
        // grouped publication cannot be split by a global ordering point.
        scanned_head = frame.latest_global;
        if scanned_head <= global_cut {
            frontier = frame.end;
        }
    }
    if scanned_head != source_head {
        return Err(format!(
            "source global head changed or differs during frontier scan: expected {source_head}, scanned {scanned_head}"
        ));
    }
    source.validate().map_err(|error| error.to_string())?;
    Ok(frontier)
}

#[cfg(test)]
#[path = "../facade/historical_global_frontier_tests.rs"]
mod tests;
