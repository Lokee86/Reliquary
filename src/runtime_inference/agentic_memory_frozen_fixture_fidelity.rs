//! Literal evidence recall stress comparison, not a live-model answer benchmark.
use super::*;
fn terms(body: &Memory) -> BTreeSet<String> {
    format!("{} {}", body.title, body.content)
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|word| (6..=32).contains(&word.len()))
        .map(str::to_owned)
        .collect()
}
pub(super) fn run(f: &Fixture) -> Value {
    let mut frequencies = BTreeMap::<String, usize>::new();
    for body in f.memories.values() {
        for word in terms(body) {
            *frequencies.entry(word).or_default() += 1;
        }
    }
    let ids = f.vectors.keys().copied().collect::<Vec<_>>();
    let mut tasks = Vec::new();
    let mut used = BTreeSet::new();
    for index in 0..12.min(ids.len()) {
        let target = ids[index * ids.len() / 12.min(ids.len())];
        let mut words = terms(&f.memories[&target]).into_iter().collect::<Vec<_>>();
        words.sort_by_key(|word| (frequencies[word], word.clone()));
        if let Some(word) = words.into_iter().find(|word| used.insert(word.clone())) {
            tasks.push((target, word));
        }
    }
    assert!(
        !tasks.is_empty(),
        "retained fact probes require alphabetic terms"
    );
    f.oracle.mode.store(2, Ordering::SeqCst);
    let mut results = Vec::new();
    let mut recalled = 0;
    let mut expected = 0;
    for (index, (target, query)) in tasks.into_iter().enumerate() {
        *f.embedding.query.lock().unwrap() = f.vectors[&target].clone();
        let relevant = f
            .memories
            .iter()
            .filter(|(_, body)| {
                format!("{} {}", body.title, body.content)
                    .to_lowercase()
                    .contains(&query)
            })
            .map(|(id, _)| f.reference(*id))
            .collect::<BTreeSet<_>>();
        let decision_start = f.oracle.calls.lock().unwrap().len();
        let embed_start = f.embedding.calls.load(Ordering::SeqCst);
        let mut controller = AgenticMemoryController::new();
        let started = std::time::Instant::now();
        let prepared = f
            .gateway
            .memory_query(
                &f.instance,
                &mut controller,
                &query,
                &format!("fidelity-{index}-query"),
            )
            .unwrap();
        f.complete(&mut controller, &prepared);
        // Literal refinement preserves the complete information need and reconsiders
        // every declined body; only returned, approved references may become anchors.
        let refined = f
            .gateway
            .refine_query(
                &f.instance,
                &mut controller,
                &query,
                &format!("fidelity-{index}-refine"),
            )
            .unwrap();
        f.complete(&mut controller, &refined);
        let anchors = controller
            .selected_ids()
            .iter()
            .take(2)
            .cloned()
            .collect::<Vec<_>>();
        let graph_calls = usize::from(!anchors.is_empty());
        if !anchors.is_empty() {
            let graph = f
                .gateway
                .traverse_graph(
                    &f.instance,
                    &mut controller,
                    &anchors,
                    &format!("fidelity-{index}-graph"),
                )
                .unwrap();
            f.complete(&mut controller, &graph);
        }
        let elapsed = started.elapsed().as_micros();
        let hits = relevant.intersection(controller.selected_ids()).count();
        recalled += hits;
        expected += relevant.len();
        let calls = f.oracle.calls.lock().unwrap();
        let calls = &calls[decision_start..];
        let candidate_presentations = calls
            .iter()
            .filter_map(|call| call["candidates"].as_array())
            .map(Vec::len)
            .sum::<usize>();
        results.push(json!({"probe":index,"target":crate::agentic_memory_retrieval_manifest::memory_ref_value(&f.reference(target)),
            "query":query,"broad_evidence":relevant.len(),"broad_candidate_memories":f.memories.len(),
            "agentic_selected":controller.selected_ids().len(),"evidence_hits":hits,
            "selected_evidence":controller.selected_ids().iter().map(crate::agentic_memory_retrieval_manifest::memory_ref_value).collect::<Vec<_>>(),
            "missed_evidence":relevant.difference(controller.selected_ids()).map(crate::agentic_memory_retrieval_manifest::memory_ref_value).collect::<Vec<_>>(),
            "evidence_recall":hits as f64/relevant.len() as f64,"broad_evidence_recall":1.0,
            "query_retrievals":1,"refinements":1,"graph_traversals":graph_calls,"max_graph_anchors":2,
            "query_embeddings":f.embedding.calls.load(Ordering::SeqCst)-embed_start,
            "decision_calls":calls.len(),"candidate_presentations":candidate_presentations,
            "raw_vector_candidates":f.vectors.len(),"elapsed_us":elapsed}));
    }
    json!({"inventory":f.metadata,"tasks":results,"evidence_hits":recalled,"broad_evidence":expected,
        "micro_evidence_recall":recalled as f64/expected as f64,"broad_evidence_recall":1.0,"answer_recall":null,
        "method":"12 deterministic retained-fact probes sampled across owner-qualified Memory ID order; rare literal terms; full retained corpus evidence oracle; target document vectors replay an idealized embedding; sequential turn receipt effects are retained",
        "limitations":["No live selection/rewrite/embedding model or generated-answer grading; no general inference required",
        "Broader context is an evaluation-only full-corpus oracle, not a deployable context-window mode",
        "Rare literal terms stress candidate exposure; results are not representative user-task accuracy",
        "Diagnostic elapsed time includes local receipts and is not quiet-host or service latency",
        "Graph budget is two already-approved anchors per probe; no hidden declined candidate is exposed to the requester"]})
}
