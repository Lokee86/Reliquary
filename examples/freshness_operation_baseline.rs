//! Deterministic full-operation Freshness trace on a disposable, real REL.
//! Run with: RELIQUARY_FRESHNESS_TRACE=1 cargo run --release --features freshness-performance-trace --example freshness_operation_baseline
//! The scenario publishes 64 pre-existing Memories plus Dream sources, then drives
//! Dream-origin relation publication, accepted settlement and +50 events for 1/2/4/8
//! roots. Accepted-use receipts represent one accepted context each with 1/4/16/64
//! distinct Memories; repeated use IDs remain unique and append-ordered. This is a
//! deterministic workload fixture, not production telemetry or end-to-end host latency.
use reliquary_memory::{AcceptedMemoryUseReceipt, Cva, MemoryDraft, MemoryId};
use std::{error::Error, fs, path::PathBuf, time::Instant};
use uuid::Uuid;

const USE_REPETITIONS: usize = 3;

struct DisposableRel(PathBuf);
impl Drop for DisposableRel {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn sample(stage: &str, elapsed_ns: u128, events: usize, roots: usize) {
    let rate = if elapsed_ns == 0 {
        0.0
    } else {
        events as f64 * 1_000_000_000.0 / elapsed_ns as f64
    };
    let (graph_nodes, active_memory_edges, published_memories) =
        if std::env::var_os("RELIQUARY_FRESHNESS_HISTORY_DEPTH").is_some() {
            (0, 0, 64)
        } else {
            (12, 15, 68)
        };
    println!(
        "{{\"kind\":\"freshness_operation_sample\",\"scenario\":\"deterministic_synthetic_rel\",\"stage\":\"{stage}\",\"elapsed_ns\":{elapsed_ns},\"event_count\":{events},\"events_per_second\":{rate},\"root_count\":{roots},\"community_snapshot\":\"absent; all origins unknown\",\"graph_nodes\":{graph_nodes},\"active_memory_edges\":{active_memory_edges},\"published_memories\":{published_memories},\"runtime_mutex_wait_ns\":null,\"real_host_latency_ns\":null}}"
    );
}

fn draft(mutation_id: &str, title: &str) -> MemoryDraft {
    MemoryDraft {
        category: "knowledge".into(),
        memory_type: "fact".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: title.into(),
        content: format!("Deterministic freshness trace fixture: {title}."),
        scope: "user".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: mutation_id.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn history_sample(
    mut cva: Cva,
    roots: Vec<MemoryId>,
    path: &std::path::Path,
    depth: usize,
    batch: usize,
) -> Result<(), Box<dyn Error>> {
    if ![1, 4, 16, 64].contains(&batch) {
        return Err("history batch must be 1/4/16/64".into());
    }
    let owner_uuid = cva.owner_uuid().ok_or("owner missing")?;
    for index in 0..depth {
        cva.record_accepted_memory_use(
            AcceptedMemoryUseReceipt {
                owner_uuid,
                accepted_turn: cva.rel_turn_count(),
                use_id: format!("history-use-{index:08}"),
                memories: vec![roots[index % roots.len()]],
            },
            1,
        )?;
    }
    // Clear bounded seed traces before timing the selected stationary history cut.
    cva.freshness_trace_flush();
    let receipt = AcceptedMemoryUseReceipt {
        owner_uuid,
        accepted_turn: cva.rel_turn_count(),
        use_id: format!("history-use-{depth:08}"),
        memories: roots[..batch].to_vec(),
    };
    let bytes_before = fs::metadata(path)?.len();
    let started = Instant::now();
    let applied = cva.record_accepted_memory_use(receipt.clone(), 4)?;
    sample(
        "history_accepted_use",
        started.elapsed().as_nanos(),
        applied,
        batch,
    );
    cva.freshness_trace_flush();
    let bytes_after = fs::metadata(path)?.len();
    let started = Instant::now();
    if cva.record_accepted_memory_use(receipt, 4)? != 0 {
        return Err("exact retry reinforced twice".into());
    }
    sample(
        "history_exact_retry",
        started.elapsed().as_nanos(),
        0,
        batch,
    );
    cva.freshness_trace_flush();
    let records = roots
        .iter()
        .map(|id| cva.freshness_record(*id))
        .collect::<Vec<_>>();
    let historical = roots
        .iter()
        .map(|id| cva.freshness_record_at(*id, 0))
        .collect::<Result<Vec<_>, _>>()?;
    drop(cva);
    let reopened = Cva::open(path)?;
    let after = roots
        .iter()
        .map(|id| reopened.freshness_record(*id))
        .collect::<Vec<_>>();
    if records != after {
        return Err("history records changed on reopen".into());
    }
    println!(
        "{}",
        serde_json::json!({
            "kind": "freshness_history_identity", "history_depth": depth, "batch": batch,
            "payload_bytes": bytes_after - bytes_before - 8, "events": applied,
            "records": format!("{records:?}"), "historical_records": format!("{historical:?}")
        })
    );
    Ok(())
}

fn run() -> Result<(), Box<dyn Error>> {
    let path: PathBuf =
        std::env::temp_dir().join(format!("freshness-baseline-{}.rel", Uuid::new_v4()));
    let _disposable = DisposableRel(path.clone());
    let started = Instant::now();
    let mut cva = Cva::create_project(&path)?;
    sample("cold_rel_create", started.elapsed().as_nanos(), 0, 0);

    let mut roots = Vec::<MemoryId>::with_capacity(64);
    for index in 0..64 {
        let mutation_id = format!("trace-root-{index:03}-v1");
        let title = format!("preexisting root {index:03}");
        let (root, _) = cva.publish_memory(None, 0, draft(&mutation_id, &title))?;
        roots.push(root.id);
    }
    let owner_uuid = cva.owner_uuid().ok_or("new REL has no owner UUID")?;
    if let Ok(depth) = std::env::var("RELIQUARY_FRESHNESS_HISTORY_DEPTH") {
        let depth = depth.parse::<usize>()?;
        let batch = std::env::var("RELIQUARY_FRESHNESS_HISTORY_BATCH")?.parse::<usize>()?;
        return history_sample(cva, roots, &path, depth, batch);
    }
    let mut dream_sources = Vec::new();
    for root_count in [1_usize, 2, 4, 8] {
        let mutation_id = format!("trace-dream-source-{root_count:02}-v1");
        let title = format!("Dream source with {root_count} roots");
        let (source, _) = cva.publish_memory(None, 0, draft(&mutation_id, &title))?;
        dream_sources.push((source.id, root_count));
    }

    for (source, root_count) in &dream_sources {
        let started = Instant::now();
        if !cva.freshness_trace_run_initial_dream_fixture(*source, &roots[..*root_count], 1)? {
            return Err(
                format!("initial Dream fixture with {root_count} roots did not commit").into(),
            );
        }
        sample(
            &format!("initial_dream_{root_count}_root_prepare_evaluate_commit"),
            started.elapsed().as_nanos(),
            1,
            *root_count,
        );
        cva.freshness_trace_flush();
    }

    let first_receipt = AcceptedMemoryUseReceipt {
        owner_uuid,
        accepted_turn: cva.rel_turn_count(),
        use_id: "freshness-baseline-accepted-use-000000".into(),
        memories: vec![roots[0]],
    };
    let started = Instant::now();
    let applied = cva.record_accepted_memory_use(first_receipt.clone(), 1)?;
    let cold_elapsed = started.elapsed().as_nanos();
    sample(
        "accepted_use_cold_single_memory_receipt",
        cold_elapsed,
        applied,
        1,
    );
    cva.freshness_trace_flush();

    let started = Instant::now();
    cva.record_accepted_memory_use(first_receipt.clone(), 1)?;
    sample(
        "accepted_use_warm_exact_retry",
        started.elapsed().as_nanos(),
        0,
        1,
    );
    cva.freshness_trace_flush();

    let mut receipt_sequence = 1_u64;
    for batch_size in [1_usize, 4, 16, 64] {
        for workers in [1_usize, 2, 4, 8] {
            let started = Instant::now();
            let mut applied_events = 0;
            for _ in 0..USE_REPETITIONS {
                let receipt = AcceptedMemoryUseReceipt {
                    owner_uuid,
                    accepted_turn: cva.rel_turn_count(),
                    use_id: format!("freshness-baseline-accepted-use-{receipt_sequence:06}"),
                    memories: roots[..batch_size].to_vec(),
                };
                receipt_sequence += 1;
                applied_events += cva.record_accepted_memory_use(receipt, workers)?;
            }
            let elapsed = started.elapsed().as_nanos();
            sample(
                &format!("accepted_use_receipts_{batch_size}_memories_workers_{workers}"),
                elapsed,
                applied_events,
                batch_size,
            );
            cva.freshness_trace_flush();
        }
    }

    cva.sync()?;
    let records_before = roots
        .iter()
        .chain(dream_sources.iter().map(|(id, _)| id))
        .map(|id| (*id, cva.freshness_record(*id)))
        .collect::<Vec<_>>();
    drop(cva);
    let started = Instant::now();
    let mut reopened = Cva::open(&path)?;
    sample("reopen", started.elapsed().as_nanos(), 0, 0);
    let started = Instant::now();
    reopened.record_accepted_memory_use(first_receipt, 1)?;
    sample(
        "accepted_use_reopen_exact_retry",
        started.elapsed().as_nanos(),
        0,
        1,
    );
    reopened.freshness_trace_flush();
    for (source, root_count) in &dream_sources {
        let started = Instant::now();
        if reopened.freshness_trace_run_initial_dream_fixture(*source, &roots[..*root_count], 1)? {
            return Err("settled initial-Dream fixture unexpectedly ran twice".into());
        }
        sample(
            &format!("initial_dream_{root_count}_root_reopen_retry"),
            started.elapsed().as_nanos(),
            0,
            *root_count,
        );
        reopened.freshness_trace_flush();
    }
    let records_after = roots
        .iter()
        .chain(dream_sources.iter().map(|(id, _)| id))
        .map(|id| (*id, reopened.freshness_record(*id)))
        .collect::<Vec<_>>();
    if records_after != records_before {
        return Err("Freshness records changed across exact retries or reopen".into());
    }
    drop(reopened);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("freshness operation baseline failed: {error}");
        std::process::exit(1);
    }
}
