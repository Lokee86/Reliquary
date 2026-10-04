//! Opt-in, stage-level diagnostics for accepted-use and initial-Dream paths.
//! The default build compiles calls to no-ops. Feature builds buffer rows until
//! an explicit flush after the operation has been timed.
#[derive(Default)]
pub(crate) struct Counts {
    pub payload_bytes: Option<u64>,
    pub object_bytes: Option<u64>,
    pub event_count: Option<u64>,
    pub effect_count: Option<u64>,
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
        rows: std::collections::BTreeMap<&'static str, (u128, u64, Counts)>,
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
                        pending.push((
                            frame.operation,
                            frame
                                .rows
                                .into_iter()
                                .map(|(stage, (elapsed, calls, counts))| {
                                    row(stage, elapsed, calls, counts)
                                })
                                .collect(),
                        ));
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
                    rows: std::collections::BTreeMap::new(),
                })
            });
        }
        Scope { enabled }
    }

    fn row(stage: &'static str, elapsed_ns: u128, calls: u64, counts: Counts) -> String {
        format!(
            "{{\"stage\":\"{}\",\"elapsed_ns\":{},\"calls\":{},\"payload_bytes\":{},\"object_bytes\":{},\"event_count\":{},\"effect_count\":{},\"chunks_visited\":{},\"graph_mutations_decoded\":{},\"community_snapshots_encountered\":{},\"community_snapshots_matched\":{},\"version_filtered_records\":{}}}",
            stage,
            elapsed_ns,
            calls,
            option(counts.payload_bytes),
            option(counts.object_bytes),
            option(counts.event_count),
            option(counts.effect_count),
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
        FRAMES.with(|frames| {
            if let Some(frame) = frames.borrow_mut().last_mut() {
                if frame.rows.len() >= 64 && !frame.rows.contains_key(stage) {
                    return;
                }
                let entry = frame
                    .rows
                    .entry(stage)
                    .or_insert_with(|| (0, 0, Counts::default()));
                entry.0 = entry.0.saturating_add(elapsed_ns);
                entry.1 = entry.1.saturating_add(1);
                fn add(target: &mut Option<u64>, value: Option<u64>) {
                    if let Some(value) = value {
                        *target = Some(target.unwrap_or(0).saturating_add(value));
                    }
                }
                add(&mut entry.2.payload_bytes, counts.payload_bytes);
                add(&mut entry.2.object_bytes, counts.object_bytes);
                add(&mut entry.2.event_count, counts.event_count);
                add(&mut entry.2.effect_count, counts.effect_count);
                add(&mut entry.2.chunks_visited, counts.chunks_visited);
                add(
                    &mut entry.2.graph_mutations_decoded,
                    counts.graph_mutations_decoded,
                );
                add(
                    &mut entry.2.community_snapshots_encountered,
                    counts.community_snapshots_encountered,
                );
                add(
                    &mut entry.2.community_snapshots_matched,
                    counts.community_snapshots_matched,
                );
                add(
                    &mut entry.2.version_filtered_records,
                    counts.version_filtered_records,
                );
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
