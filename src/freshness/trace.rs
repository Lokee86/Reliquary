//! Opt-in, stage-level diagnostics for accepted-use and initial-Dream paths.
//! The default build compiles calls to no-ops. Feature builds buffer rows until
//! an explicit flush after the operation has been timed.
#[derive(Default)]
pub(crate) struct Counts {
    pub payload_bytes: Option<u64>,
    pub object_bytes: Option<u64>,
    pub chunks_visited: Option<u64>,
    pub graph_mutations_decoded: Option<u64>,
    pub community_snapshots_encountered: Option<u64>,
    pub community_snapshots_matched: Option<u64>,
    pub version_filtered_records: Option<u64>,
}

#[cfg(feature = "freshness-performance-trace")]
mod enabled {
    use super::Counts;
    use std::{cell::RefCell, time::Instant};

    struct Frame {
        operation: &'static str,
        rows: Vec<String>,
    }
    thread_local! {
        static FRAMES: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
        static PENDING: RefCell<Vec<(&'static str, Vec<String>)>> = const { RefCell::new(Vec::new()) };
    }

    pub(crate) struct Scope {
        enabled: bool,
    }
    impl Drop for Scope {
        fn drop(&mut self) {
            if !self.enabled {
                return;
            }
            FRAMES.with(|frames| {
                let Some(frame) = frames.borrow_mut().pop() else {
                    return;
                };
                if frame.rows.is_empty() {
                    return;
                }
                PENDING.with(|pending| {
                    let mut pending = pending.borrow_mut();
                    if pending.len() < 64 {
                        pending.push((frame.operation, frame.rows));
                    }
                });
            });
        }
    }

    pub(crate) fn scope(operation: &'static str) -> Scope {
        let enabled = std::env::var_os("RELIQUARY_FRESHNESS_TRACE").is_some();
        if enabled {
            FRAMES.with(|frames| {
                frames.borrow_mut().push(Frame {
                    operation,
                    rows: Vec::new(),
                })
            });
        }
        Scope { enabled }
    }

    fn row(stage: &'static str, elapsed_ns: u128, counts: Counts) -> String {
        format!(
            "{{\"stage\":\"{}\",\"elapsed_ns\":{},\"payload_bytes\":{},\"object_bytes\":{},\"chunks_visited\":{},\"graph_mutations_decoded\":{},\"community_snapshots_encountered\":{},\"community_snapshots_matched\":{},\"version_filtered_records\":{}}}",
            stage,
            elapsed_ns,
            option(counts.payload_bytes),
            option(counts.object_bytes),
            option(counts.chunks_visited),
            option(counts.graph_mutations_decoded),
            option(counts.community_snapshots_encountered),
            option(counts.community_snapshots_matched),
            option(counts.version_filtered_records)
        )
    }
    fn option(value: Option<u64>) -> String {
        value.map_or_else(|| "null".into(), |n| n.to_string())
    }

    pub(crate) fn measure<T>(stage: &'static str, operation: impl FnOnce() -> T) -> T {
        if std::env::var_os("RELIQUARY_FRESHNESS_TRACE").is_none() {
            return operation();
        }
        let started = Instant::now();
        let result = operation();
        record(stage, started.elapsed().as_nanos(), Counts::default());
        result
    }

    pub(crate) fn record(stage: &'static str, elapsed_ns: u128, counts: Counts) {
        let row = row(stage, elapsed_ns, counts);
        FRAMES.with(|frames| {
            if let Some(frame) = frames.borrow_mut().last_mut() {
                if frame.rows.len() < 256 {
                    frame.rows.push(row);
                }
            }
        });
    }

    pub(crate) fn flush() {
        if std::env::var_os("RELIQUARY_FRESHNESS_TRACE").is_none() {
            return;
        }
        PENDING.with(|pending| {
            let rows = std::mem::take(&mut *pending.borrow_mut());
            for (operation, samples) in rows {
                eprintln!(
                    "{{\"kind\":\"freshness_trace\",\"operation\":\"{}\",\"duration_semantics\":\"inclusive stage elapsed time; do not sum parent and child stages\",\"samples\":[{}]}}",
                    operation, samples.join(",")
                );
            }
        });
    }
}

#[cfg(feature = "freshness-performance-trace")]
pub(crate) use enabled::{flush, measure, record, scope};

#[cfg(not(feature = "freshness-performance-trace"))]
#[inline]
pub(crate) fn measure<T>(_: &'static str, operation: impl FnOnce() -> T) -> T {
    operation()
}
#[cfg(not(feature = "freshness-performance-trace"))]
#[inline]
pub(crate) fn scope(_: &'static str) {}
#[cfg(not(feature = "freshness-performance-trace"))]
#[inline]
pub(crate) fn flush() {}
#[cfg(not(feature = "freshness-performance-trace"))]
#[inline]
pub(crate) fn record(_: &'static str, _: u128, _: Counts) {}
