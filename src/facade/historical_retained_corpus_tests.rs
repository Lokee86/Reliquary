//! Explicit retained-corpus gate. Never opens the authoritative input for writing.
use super::*;
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf, time::Instant};
#[path = "historical_retained_diagnostics.rs"]
mod diagnostics;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
#[ignore = "requires a hash-pinned retained REL; run scripts/freshness_historical_corpus.py"]
fn historical_retained_corpus_native_acceptance() {
    let source = PathBuf::from(std::env::var_os("RELIQUARY_HISTORICAL_CORPUS").unwrap());
    let expected = std::env::var("RELIQUARY_HISTORICAL_CORPUS_SHA256").unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let scratch = root
        .join("target/historical-corpus")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&scratch).unwrap();
    let copy = scratch.join("input.rel");
    assert_eq!(hex(&file_digest(&source).unwrap()), expected);
    fs::copy(&source, &copy).unwrap();
    #[cfg(windows)]
    {
        let mut permissions = fs::metadata(&copy).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&copy, permissions).unwrap();
    }
    assert_eq!(hex(&file_digest(&copy).unwrap()), expected);
    let started = Instant::now();
    let opened = Cva::open(&copy);
    let open_ns = started.elapsed().as_nanos();
    let mut result = json!({"schema":1,"source_sha256":expected,"source_bytes":fs::metadata(&source).unwrap().len(),
        "copy":copy,"open_ns":open_ns,"status":"rejected","acceptance":null,
        "embedding_fill": "not requested; retained evidence only", "measurement_kind":"native retained-corpus gate"});
    if let Ok(mut cva) = opened {
        let ids = cva.memory_ids();
        let cut = cva.rel_turn_count();
        result["prerequisite_diagnosis"] = diagnostics::diagnose(&mut cva);
        let work = cva.insomnia_stats();
        result["insomnia_queue"] = json!({"total":work.total,"pending":work.pending,"processing":work.processing,
            "complete":work.complete,"failed":work.failed,"terminal":work.terminal});
        result["timeline_gate"] = match cva.historical_freshness_timeline() {
            Ok(_) => json!({"valid":true}),
            Err(crate::freshness::historical_planner::PlannerError::TimestampRegression {
                ancestor,
                descendant,
            }) => {
                let parent = cva
                    .archive
                    .nodes
                    .get(&ancestor.conversation_id, &ancestor.node_id)
                    .unwrap();
                let child = cva
                    .archive
                    .nodes
                    .get(&descendant.conversation_id, &descendant.node_id)
                    .unwrap();
                json!({"valid":false,"reason":"ancestor timestamp exceeds descendant timestamp",
                    "ancestor_timestamp_ns":parent.timestamp_ns,"descendant_timestamp_ns":child.timestamp_ns,
                    "ancestor_id":ancestor.node_id,"descendant_id":descendant.node_id})
            }
            Err(error) => json!({"valid":false,"error":error.to_string()}),
        };
        let mut population = BTreeMap::<String, usize>::new();
        let mut structured = 0;
        let mut pending = 0;
        for &id in &ids {
            let memory = cva.memory(id).unwrap();
            structured += usize::from(
                memory.content_source_conversation_id.is_some()
                    && memory.content_source_node_id.is_some(),
            );
            pending += usize::from(cva.freshness_has_pending_publication(id));
            let state = cva
                .freshness_record(id)
                .map_or("NotEnrolled".to_owned(), |record| {
                    format!("{:?}", record.state_at(cut).unwrap())
                });
            *population.entry(state).or_default() += 1;
        }
        let passes = cva.historical_completed_initial_dream_passes().unwrap();
        let generations = cva.vector_generations.generations().to_vec();
        let mut usable_fragments = 0;
        let mut archive_rows = 0;
        let mut target_rows = 0;
        if let Some(generation) = generations.iter().max_by_key(|g| g.vector_version) {
            archive_rows = cva
                .archive_vectors(generation.archive_vector_id)
                .unwrap()
                .fragment_ids
                .len();
            for id in &ids {
                let body = cva.memory_body_id(*id).unwrap();
                target_rows += usize::from(
                    cva.memory_vector_location(generation.compatibility_profile_id, body)
                        .is_some(),
                );
            }
            for fragment in cva.fragments() {
                if let crate::cva_semantic_source::SemanticSourceReadiness::Ready(source) = cva
                    .prepare_semantic_fragment(fragment.id, generation.id)
                    .unwrap()
                {
                    usable_fragments += usize::from(!source.vectors.is_empty());
                }
            }
        }
        result["inventory"] = json!({"memories":ids.len(),"activity_turns":cut,"episodes":cva.episodes().len(),
            "structured_content_provenance_memories":structured,"pending_publications":pending,
            "retained_first_cycle_passes":passes.len(),"vector_generations":generations.len(),
            "latest_generation_archive_rows":archive_rows,"latest_generation_target_bindings":target_rows,
            "usable_fragment_source_evidence":usable_fragments,"graph_relations":cva.graph_relations().len(),
            "before_distribution":population,"already_checkpointed":cva.freshness.import_checkpoint().is_some()});
        // Diagnostics read the same owner adapters; only the public operation accepts.
        let diagnostic = cva.freeze_historical_import();
        match &diagnostic {
            Ok(frozen) => {
                if let (Some(batch), Some(inputs)) = (&frozen.events, &frozen.propagation) {
                    let events: Vec<_> = batch
                        .ordered
                        .iter()
                        .map(|(_, event)| event.clone())
                        .collect();
                    let outcomes =
                        crate::freshness::historical_propagation::propagate_historical_events(
                            &events,
                            &inputs.adjacency,
                            &inputs.memberships,
                            &inputs.authorized,
                            &inputs.availability,
                            1,
                            &crate::freshness::FreshnessPolicy::default_v1(),
                        )
                        .unwrap();
                    let mut coverage = BTreeMap::<String, usize>::new();
                    for value in batch.semantic.coverage.values() {
                        *coverage.entry(value.clone()).or_default() += 1;
                    }
                    let mut bonus = BTreeMap::<i16, usize>::new();
                    for event in &batch.semantic.events {
                        for effect in &event.effects {
                            *bonus.entry(effect.bonus).or_default() += 1;
                        }
                    }
                    result["diagnostics"] = json!({"semantic_events":batch.semantic.events.len(),"semantic_coverage":coverage,
                        "local_match_bonus_histogram":bonus,"dream_events":batch.dream.events.len(),
                        "propagation_events":outcomes.len(),"recipient_contributions":outcomes.iter().map(|o| o.candidates.len()).sum::<usize>(),
                        "examined_edges":outcomes.iter().map(|o|o.stats.examined_edges).sum::<u64>(),
                        "candidate_expansions":outcomes.iter().map(|o|o.stats.candidate_expansions).sum::<u64>(),
                        "maximum_frontier_width":outcomes.iter().map(|o|o.stats.max_frontier_width).max().unwrap_or(0)});
                }
            }
            Err(error) => result["diagnostic_rejection"] = json!(error.to_string()),
        }
        let before_hash = hex(&file_digest(&copy).unwrap());
        let before_records = cva.freshness_records_at(cut).unwrap();
        let before_bytes = fs::metadata(&copy).unwrap().len();
        let started = Instant::now();
        let accepted = cva.reconstruct_historical_freshness(Default::default());
        result["operation_ns"] = json!(started.elapsed().as_nanos());
        match accepted {
            Ok(report) => {
                result["status"] = json!(if report.committed {
                    "accepted"
                } else {
                    "already-accepted"
                });
                result["acceptance"] = json!({"manifest_digest":hex(&report.manifest_digest),"import_id":hex(&report.import_id),
                    "reconstructed_memories":report.reconstructed_memories,"activity_cut":report.activity_cut,
                    "checkpoint_append_bytes":fs::metadata(&copy).unwrap().len() - before_bytes});
                let records = cva.freshness_records_at(cut).unwrap();
                drop(cva);
                let started = Instant::now();
                let mut reopened = Cva::open(&copy).unwrap();
                result["reopen_ns"] = json!(started.elapsed().as_nanos());
                assert_eq!(reopened.freshness_records_at(cut).unwrap(), records);
                let hash = hex(&file_digest(&copy).unwrap());
                let started = Instant::now();
                assert!(
                    !reopened
                        .reconstruct_historical_freshness(Default::default())
                        .unwrap()
                        .committed
                );
                result["retry_ns"] = json!(started.elapsed().as_nanos());
                assert_eq!(hex(&file_digest(&copy).unwrap()), hash);
                let mut distribution = BTreeMap::<String, usize>::new();
                for record in records.values() {
                    *distribution
                        .entry(format!("{:?}", record.state_at(cut).unwrap()))
                        .or_default() += 1;
                }
                result["final_distribution"] = json!(distribution);
            }
            Err(error) => {
                result["error"] = json!(error.to_string());
                assert_eq!(cva.freshness_records_at(cut).unwrap(), before_records);
                assert_eq!(hex(&file_digest(&copy).unwrap()), before_hash);
                result["rejection_preserves_bytes_and_projection"] = json!(true);
            }
        }
    } else if let Err(error) = opened {
        result["error"] = json!(format!("open: {error}"));
    }
    assert_eq!(hex(&file_digest(&source).unwrap()), expected);
    result["source_unchanged"] = json!(true);
    let output = PathBuf::from(std::env::var_os("RELIQUARY_HISTORICAL_CORPUS_REPORT").unwrap());
    assert!(
        output
            .canonicalize()
            .unwrap_or(output.clone())
            .starts_with(&root)
    );
    fs::write(output, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    // A rejected legacy corpus is an audit result, never a passing C10 acceptance.
    println!("retained corpus native gate: {}", result["status"]);
}
