//! Mandatory scenarios against retained data; no synthetic graph facts are inserted.
use super::*;
fn limitation(reason: &str) -> Value {
    json!({"status":"corpus_limitation","reason":reason})
}
pub(super) fn run(f: &Fixture) -> Value {
    let g = &f.gateway;
    let i = &f.instance;
    f.oracle.mode.store(0, Ordering::SeqCst);
    let before = f.snapshot();
    let mut controller = AgenticMemoryController::new();
    let prepared = g
        .memory_query(i, &mut controller, "retained evidence", "functional-query")
        .unwrap();
    assert_eq!(before, f.snapshot());
    assert!(controller.selected_ids().is_empty());
    assert!(prepared.delivered().len() <= 10);
    let envelope = g.capture_agentic_read(i).unwrap();
    let (raw, fresh) = g
        .with_rel(&envelope.owners[0].token, |runtime| {
            let vector = f.embedding.query.lock().unwrap().clone();
            let pool = runtime
                .cva
                .collect_memory_candidate_pool("retained evidence", &vector, f.embedding.profile.id)
                .unwrap();
            let mut raw = pool.candidates.clone();
            raw.sort_by(|a, b| {
                b.evidence
                    .semantic_score
                    .unwrap_or(-2.0)
                    .total_cmp(&a.evidence.semantic_score.unwrap_or(-2.0))
                    .then(a.memory_id.cmp(&b.memory_id))
            });
            let ids = pool.matched_memory_ids().collect::<Vec<_>>();
            let mut ranked = crate::agentic_memory_retrieval::grade_agentic_memories(
                &runtime.cva,
                &ids,
                envelope.owners[0].cut,
            )
            .unwrap();
            crate::agentic_memory_retrieval::order_agentic_memories(&mut ranked);
            Ok((
                raw.iter()
                    .take(10)
                    .map(|c| f.reference(c.memory_id))
                    .collect::<BTreeSet<_>>(),
                ranked
                    .iter()
                    .take(10)
                    .map(|c| c.memory.clone())
                    .collect::<BTreeSet<_>>(),
            ))
        })
        .unwrap();
    assert_eq!(prepared.delivered(), &fresh);
    for failure in [
        AgenticMemoryHandoffConfirmation::Failed,
        AgenticMemoryHandoffConfirmation::Cancelled,
    ] {
        assert!(
            g.complete_agentic_memory_delivery(i, &mut controller, &prepared, &failure)
                .is_err()
        );
        assert!(controller.selected_ids().is_empty());
        assert_eq!(before, f.snapshot());
    }
    f.complete(&mut controller, &prepared);
    let once = f.snapshot();
    assert_eq!(controller.selected_ids(), prepared.delivered());
    f.complete(&mut controller, &prepared);
    assert_eq!(once, f.snapshot());
    f.oracle.mode.store(1, Ordering::SeqCst);
    let repeated = g
        .memory_query(
            i,
            &mut controller,
            "retained evidence",
            "functional-already",
        )
        .unwrap();
    assert!(matches!(
        repeated.payload().outcome,
        AgenticMemoryOutcome::AlreadyProvided(_)
    ));
    assert!(repeated.delivered().is_empty());
    let mut reports = json!({"inventory":f.metadata,"query_top_ten": {"status":"passed","presented":fresh.len()},
        "freshness_ordering":if raw!=fresh {json!({"status":"passed","differs_from_raw_semantic_top_ten":true})}else{limitation("retained raw semantic and Freshness top-ten sets coincide")},
        "failed_handoff_and_cancel":{"status":"passed","receipt_bytes_unchanged":true},
        "success_and_identical_retry":{"status":"passed","retry_bytes_unchanged":true},
        "already_provided":{"status":"passed"}});
    let mut adjacency = BTreeMap::<MemoryId, BTreeSet<MemoryId>>::new();
    let envelope = g.capture_agentic_read(i).unwrap();
    let edges = g
        .with_rel(&envelope.owners[0].token, |runtime| {
            Ok(runtime.cva.graph_relations())
        })
        .unwrap();
    for edge in edges.into_iter().filter(|e| {
        e.active && f.memories.contains_key(&e.source) && f.memories.contains_key(&e.target)
    }) {
        adjacency
            .entry(edge.source)
            .or_default()
            .insert(edge.target);
        adjacency
            .entry(edge.target)
            .or_default()
            .insert(edge.source);
    }
    let anchor = adjacency
        .iter()
        .filter(|(_, neighbors)| neighbors.len() > 10)
        .max_by_key(|(id, neighbors)| (neighbors.len(), **id))
        .map(|(id, neighbors)| (*id, neighbors.len()));
    let mut refined = AgenticMemoryController::new();
    g.memory_query(
        i,
        &mut refined,
        "initial declined evidence",
        "functional-declined",
    )
    .unwrap();
    if let Some((id, _)) = anchor {
        for round in 0..2 {
            g.traverse_graph(
                i,
                &mut refined,
                &[f.reference(id)],
                &format!("functional-declined-graph-{round}"),
            )
            .unwrap();
        }
    }
    let declined = refined.searched_ids().clone();
    let embeds = f.embedding.calls.load(Ordering::SeqCst);
    f.oracle.mode.store(0, Ordering::SeqCst);
    let reconsidered = g
        .refine_query(
            i,
            &mut refined,
            "corrected complete retained evidence",
            "functional-refine",
        )
        .unwrap();
    assert_eq!(reconsidered.delivered(), &declined);
    assert_eq!(
        refined.current_query(),
        Some("corrected complete retained evidence")
    );
    assert_eq!(f.embedding.calls.load(Ordering::SeqCst), embeds);
    reports["refinement"] =
        json!({"status":"passed","reconsidered":declined.len(),"additional_query_embeddings":0});
    reports["repeated_one_hop"] = if let Some((id, degree)) = anchor {
        let mut graph = Fixture::controller();
        let anchor = f.reference(id);
        let passive = f.snapshot();
        let first = g
            .traverse_graph(
                i,
                &mut graph,
                std::slice::from_ref(&anchor),
                "functional-graph-1",
            )
            .unwrap();
        let second = g
            .traverse_graph(i, &mut graph, &[anchor], "functional-graph-2")
            .unwrap();
        assert_eq!(first.delivered().len(), 10);
        assert_eq!(second.delivered().len(), (degree - 10).min(10));
        assert!(first.delivered().is_disjoint(second.delivered()));
        assert_eq!(first.payload().anchors[0].unseen_neighbors, degree);
        assert_eq!(second.payload().anchors[0].unseen_neighbors, degree - 10);
        assert_eq!(passive, f.snapshot());
        json!({"status":"passed","one_hop_neighbors":degree,"first":first.delivered().len(),"second":second.delivered().len()})
    } else {
        limitation("no retained active anchor has more than ten one-hop neighbors")
    };
    // Use the production preparation policy to find two anchors whose actual next
    // presentation batches overlap; discovery never changes the retained graph.
    let graph = Fixture::controller();
    let envelope = g.capture_agentic_read(i).unwrap();
    let mut seen = BTreeMap::<MemoryRef, MemoryRef>::new();
    let mut overlap = None;
    for id in adjacency.keys() {
        let anchor = f.reference(*id);
        let batch = g
            .prepare_agentic_anchor(&envelope, &graph, &anchor)
            .unwrap();
        for body in batch.candidates {
            if let Some(other) = seen.get(&body.memory) {
                overlap = Some((other.clone(), anchor.clone()));
                break;
            }
            seen.insert(body.memory, anchor.clone());
        }
        if overlap.is_some() {
            break;
        }
    }
    reports["concurrent_shared_neighbor"] = if let Some((left, right)) = overlap {
        let mut graph = graph;
        let result = g
            .traverse_graph(i, &mut graph, &[left, right], "functional-overlap")
            .unwrap();
        assert_eq!(result.payload().anchors.len(), 2);
        let a = &result.payload().anchors[0].approved;
        let b = &result.payload().anchors[1].approved;
        assert!(!a.is_disjoint(b));
        let union = a.union(b).cloned().collect::<BTreeSet<_>>();
        assert_eq!(result.delivered(), &union);
        let AgenticMemoryOutcome::Memories(content) = &result.payload().outcome else {
            panic!("shared evidence expected")
        };
        assert_eq!(content.len(), union.len());
        json!({"status":"passed","independent_reports":2,"unique_content":content.len(),"shared":a.intersection(b).count()})
    } else {
        limitation("no retained anchor pair has overlapping next-ten presentation batches")
    };
    reports
}
