use super::*;
use crate::freshness_storage::FreshnessReplayEntry;
use crate::{
    Cva, FreshnessEventCut, FreshnessEventProof, GraphRelationKind, GraphRelationOrigin,
    MemoryDraft, MemoryId,
};
use std::{collections::BTreeSet, fs, path::PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new(name: &str) -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("freshness-history-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft(name: &str, lifecycle: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: name.into(),
        content: format!("Freshness history fixture: {name}"),
        scope: "project".into(),
        lifecycle_state: lifecycle.into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(0),
        mutation_id: name.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn publish(rel: &mut Cva, name: &str, lifecycle: &str) -> MemoryId {
    let (memory, created) = rel.publish_memory(None, 0, draft(name, lifecycle)).unwrap();
    assert!(created);
    memory.id
}

fn append_turns(rel: &mut Cva, start: usize, end: usize) {
    for index in start..end {
        rel.append_node(
            format!("freshness-oracle-{index}"),
            "freshness-oracle".into(),
            None,
            if index % 2 == 0 { "user" } else { "assistant" }.into(),
            index as i64,
            "Accepted owner activity for historical projection.",
        )
        .unwrap();
    }
}

fn add_accepted_initial_event(
    rel: &mut Cva,
    source: MemoryId,
    target: MemoryId,
    amount: i16,
) -> (String, FreshnessEventProof) {
    let owner = rel.owner_uuid().unwrap();
    let pass_id = crate::dream_freshness::initial_pass_id(owner, source);
    let turn = rel.rel_turn_count();
    rel.begin_initial_dream_pass(
        pass_id.clone(),
        source,
        1,
        turn,
        BTreeSet::from([target]),
        rel.graph_version(),
    )
    .unwrap();
    rel.set_memory_relation_with_origin(
        source,
        target,
        GraphRelationKind::Topical,
        true,
        GraphRelationOrigin::Dream,
        rel.graph_version(),
    )
    .unwrap();
    rel.record_initial_dream_roots(&pass_id, BTreeSet::from([target]))
        .unwrap();
    rel.accept_initial_dream_settlement(&pass_id, turn).unwrap();
    let proof = FreshnessEventProof {
        graph_version: rel.memory_graph_version(),
        community_generation: None,
        community_graph_version: None,
    };
    rel.settle_initial_dream_pass(
        &pass_id,
        false,
        proof,
        std::collections::BTreeMap::from([(target, amount)]),
    )
    .unwrap();

    let actual_event_id = rel
        .freshness_replay_entries()
        .unwrap()
        .into_iter()
        .find_map(|entry| match entry {
            FreshnessReplayEntry::Event {
                event_id,
                source_turn,
                proof: event_proof,
                effects,
                ..
            } if source_turn == turn
                && event_proof == proof
                && effects.get(&reliquary_freshness_store::FreshnessMemoryId(target.0))
                    == Some(&amount) =>
            {
                Some(event_id)
            }
            _ => None,
        })
        .expect("accepted event must appear in the owner replay projection");
    assert_eq!(actual_event_id, pass_id);
    (actual_event_id, proof)
}

fn assert_per_record_exact_cut(rel: &Cva, cut: &FreshnessEventCut, projected: &FreshnessCut) {
    let ids = rel
        .freshness_records_at(cut.rel_turn)
        .unwrap()
        .into_keys()
        .collect::<Vec<_>>();
    // Each single-record public read replays the complete owner. Compare the
    // full map separately and sample this API boundary on large corpora.
    let ids = if ids.len() > 8 {
        vec![
            ids[0],
            ids[1],
            ids[ids.len() / 2],
            ids[ids.len() - 2],
            ids[ids.len() - 1],
        ]
    } else {
        ids
    };
    for id in ids {
        assert_eq!(
            rel.freshness_record_at_cut(id, cut).unwrap(),
            projected.records.get(&id).copied(),
            "per-record exact-cut result differs for {id:?}"
        );
    }
}

#[test]
fn exact_freshness_cuts_order_same_turn_events_and_filter_graph_proofs() {
    let fixture = Fixture::new("same-turn");
    let path = fixture.path("owner.rel");
    let mut rel = Cva::create(&path).unwrap();
    // Give the standard publication a valid first-cycle pass with no roots.
    // Its accepted admission at turn 0 provides a decaying target while
    // retaining authentic publication provenance for later Dream receipts.
    let target = publish(&mut rel, "initial-target", "knowledge");
    let target_pass = crate::dream_freshness::initial_pass_id(rel.owner_uuid().unwrap(), target);
    rel.begin_initial_dream_pass(
        target_pass.clone(),
        target,
        1,
        0,
        BTreeSet::new(),
        rel.graph_version(),
    )
    .unwrap();
    rel.record_initial_dream_roots(&target_pass, BTreeSet::new())
        .unwrap();
    rel.accept_initial_dream_settlement(&target_pass, 0)
        .unwrap();
    let target_proof = FreshnessEventProof {
        graph_version: rel.memory_graph_version(),
        community_generation: None,
        community_graph_version: None,
    };
    rel.settle_initial_dream_pass(
        &target_pass,
        true,
        target_proof,
        std::collections::BTreeMap::new(),
    )
    .unwrap();
    append_turns(&mut rel, 0, 1500);
    assert_eq!(
        rel.freshness_record(target)
            .unwrap()
            .score_at(1500)
            .unwrap(),
        -50
    );
    let source_a = publish(&mut rel, "initial-source-a", "extracted");
    let source_b = publish(&mut rel, "initial-source-b", "extracted");
    assert_eq!(rel.rel_turn_count(), 1500);

    // Event identity is an accepted ledger value. Process the two valid Dream
    // events in its canonical same-turn string order.
    let mut sources = vec![source_a, source_b]
        .into_iter()
        .map(|source| {
            let id = crate::dream_freshness::initial_pass_id(rel.owner_uuid().unwrap(), source);
            (id, source)
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    let (first_id, first_proof) = add_accepted_initial_event(&mut rel, sources[0].1, target, 50);
    let first_graph = first_proof.graph_version;
    let (second_id, second_proof) = add_accepted_initial_event(&mut rel, sources[1].1, target, 50);
    let second_graph = second_proof.graph_version;
    assert!(first_id < second_id);
    assert!(first_graph < second_graph);

    let event_only_first = FreshnessEventCut {
        rel_turn: 1500,
        event_id_inclusive: Some(first_id.clone()),
        graph_version: second_graph,
    };
    let graph_only_first = FreshnessEventCut {
        rel_turn: 1500,
        event_id_inclusive: None,
        graph_version: first_graph,
    };
    let both_events = FreshnessEventCut {
        rel_turn: 1500,
        event_id_inclusive: None,
        graph_version: second_graph,
    };

    let first_by_id = freshness_at_event(&rel, &event_only_first).unwrap();
    let first_by_graph = freshness_at_event(&rel, &graph_only_first).unwrap();
    let both = freshness_at_event(&rel, &both_events).unwrap();
    // Equal materialized records at different query points still bind distinct
    // stable event/graph cut metadata in the fingerprint contract.
    assert_ne!(first_by_id.fingerprint, first_by_graph.fingerprint);
    assert_eq!(first_by_id.records, first_by_graph.records);
    assert_eq!(first_by_id.records[&target].score_at(1500).unwrap(), 0);
    assert_eq!(both.records[&target].score_at(1500).unwrap(), 50);
    assert_per_record_exact_cut(&rel, &event_only_first, &first_by_id);
    assert_per_record_exact_cut(&rel, &graph_only_first, &first_by_graph);
    assert_per_record_exact_cut(&rel, &both_events, &both);

    // A later accepted event advances activity and graph clocks, while the old
    // exact event cut remains stable.
    append_turns(&mut rel, 1500, 1510);
    let later_source = publish(&mut rel, "later-source", "extracted");
    let (_, later_proof) = add_accepted_initial_event(&mut rel, later_source, target, 50);
    assert!(later_proof.graph_version > second_graph);
    let old_again = freshness_at_event(&rel, &both_events).unwrap();
    assert_eq!(old_again.fingerprint, both.fingerprint);
    assert_eq!(old_again.records, both.records);

    let current_head_turn = rel.rel_turn_count();
    let current_graph = rel.memory_graph_version();
    assert!(
        freshness_at_event(
            &rel,
            &FreshnessEventCut {
                rel_turn: current_head_turn + 1,
                event_id_inclusive: None,
                graph_version: current_graph,
            }
        )
        .is_err()
    );
    assert!(
        freshness_at_event(
            &rel,
            &FreshnessEventCut {
                rel_turn: current_head_turn,
                event_id_inclusive: Some("  ".into()),
                graph_version: current_graph,
            }
        )
        .is_err()
    );
    assert!(
        freshness_at_event(
            &rel,
            &FreshnessEventCut {
                rel_turn: current_head_turn,
                event_id_inclusive: None,
                graph_version: current_graph + 1,
            }
        )
        .is_err()
    );

    rel.sync().unwrap();
    drop(rel);
    let reopened = Cva::open(&path).unwrap();
    let reopened_cut = freshness_at_event(&reopened, &both_events).unwrap();
    assert_eq!(reopened_cut.fingerprint, both.fingerprint);
    assert_eq!(reopened_cut.records, both.records);
    assert_per_record_exact_cut(&reopened, &both_events, &reopened_cut);
    assert_eq!(
        freshness_at_turn(&reopened, 1500).unwrap().records,
        reopened.freshness_records_at(1500).unwrap()
    );
}

#[test]
#[cfg(feature = "history-oracle-fixtures")]
fn authoritative_rel_freshness_cuts_match_public_records_deterministically() {
    let root = PathBuf::from(std::env::var_os("RELIQUARY_HISTORY_FIXTURES").expect("fixture root"));
    let fixture = Fixture::new("authoritative");
    for (index, corpus) in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ]
    .iter()
    .enumerate()
    {
        let path = fixture.path(&format!("{index}.rel"));
        let mut source = fs::File::open(root.join(corpus).join("project.prj.rel")).unwrap();
        let mut destination = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        std::io::copy(&mut source, &mut destination).unwrap();
        drop(destination);

        let rel = Cva::open(&path).unwrap();
        let head = rel.rel_turn_count();
        let memory_graph_head = rel.memory_graph_version();
        let mut turns = vec![0, 1.min(head), head / 2, head];
        turns.sort_unstable();
        turns.dedup();
        for turn in turns {
            let first = freshness_at_turn(&rel, turn).unwrap();
            let second = freshness_at_turn(&rel, turn).unwrap();
            let public_records = rel.freshness_records_at(turn).unwrap();
            assert_eq!(first.fingerprint, second.fingerprint);
            assert_eq!(first.records, second.records);
            assert_eq!(first.records, public_records);
        }

        let head_cut = FreshnessEventCut {
            rel_turn: head,
            event_id_inclusive: None,
            graph_version: memory_graph_head,
        };
        let first = freshness_at_event(&rel, &head_cut).unwrap();
        let second = freshness_at_event(&rel, &head_cut).unwrap();
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(first.records, second.records);
        assert_eq!(first.records, rel.freshness_records_at(head).unwrap());
        assert_per_record_exact_cut(&rel, &head_cut, &first);

        let (event_rows, proof_rows) = rel
            .freshness
            .retained_history_entries()
            .unwrap()
            .into_iter()
            .fold((0_usize, 0_usize), |(events, proofs), entry| match entry {
                FreshnessReplayEntry::Event { proof, .. } => (
                    events + 1,
                    proofs + usize::from(proof.graph_version <= memory_graph_head),
                ),
                _ => (events, proofs),
            });
        eprintln!(
            "Freshness fixture {corpus}/rel: activity head {head}, memory graph head {memory_graph_head}, enumerated retained event rows {event_rows}, graph-eligible proof rows {proof_rows}"
        );
    }
}
