use super::*;
use crate::{
    Cva, EpisodeBoundary, EpisodeConfig, EpisodeOrigin, GraphRelationKind, MemoryDraft, MemoryId,
};
use std::{fs, path::PathBuf};
fn scratch() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("graph-oracle-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn graph_rewrite_baseline_keeps_isolated_memory_population_at_zero() {
    let dir = scratch();
    let source = dir.join("source.rel");
    let output = dir.join("rewritten.rel");
    let mut cva = Cva::create_project(&source).unwrap();
    let ep = episode(&mut cva);
    let a = memory(&mut cva, &ep, "isolated-a");
    let b = memory(&mut cva, &ep, "isolated-b");
    cva.set_memory_relation(a, b, GraphRelationKind::Factual, true, 0)
        .unwrap();
    cva.set_memory_relation(a, b, GraphRelationKind::Factual, false, 1)
        .unwrap();
    cva.sync().unwrap();
    drop(cva);
    crate::rewrite::rewrite_file(&source, &output, &crate::RewriteRequest::default()).unwrap();
    let rewritten = Cva::open(&output).unwrap();
    let history = GraphHistory::read(&rewritten.container, &rewritten.graph).unwrap();
    assert_eq!(rewritten.graph_version(), 0);
    assert_eq!(history.baseline_memories.len(), 2);
    assert_matches_replay(&history, &rewritten.graph);
    let with_isolated = history.at(0, true).unwrap().fingerprint;
    let empty = GraphHistory {
        baseline_memories: vec![],
        ..history
    };
    assert_ne!(with_isolated, empty.at(0, true).unwrap().fingerprint);
    drop(rewritten);
    fs::remove_dir_all(dir).unwrap();
}
fn assert_matches_replay(history: &GraphHistory, store: &GraphStore) {
    let full = history.at(store.graph_version(), false).unwrap();
    assert_eq!(full.memory_graph_version, store.memory_graph_version());
    let active: BTreeMap<_, _> = full
        .states
        .iter()
        .filter(|(_, r)| {
            r.active
                && store.semantic_node_id(r.source).is_ok()
                && store.semantic_node_id(r.target).is_ok()
        })
        .map(|(k, r)| (*k, *r))
        .collect();
    let expected: BTreeMap<_, _> = store
        .active_semantic_relations()
        .into_iter()
        .map(|r| ((r.source, r.target, r.kind), r))
        .collect();
    assert_eq!(active, expected);
    let memory = history.at(store.memory_graph_version(), true).unwrap();
    let actual: BTreeMap<_, _> = memory
        .states
        .iter()
        .filter(|(_, r)| {
            r.active
                && store.semantic_node_id(r.source).is_ok()
                && store.semantic_node_id(r.target).is_ok()
        })
        .map(|(k, r)| (*k, *r))
        .collect();
    let expected: BTreeMap<_, _> = expected
        .into_iter()
        .filter(|(_, r)| matches!(r.kind, SemanticGraphRelationKind::Memory(_)))
        .collect();
    assert_eq!(actual, expected);
    let mut nodes: std::collections::BTreeSet<_> =
        history.baseline_memories.iter().copied().collect();
    for relation in memory.states.values() {
        nodes.insert(relation.source.as_memory().unwrap());
        nodes.insert(relation.target.as_memory().unwrap());
    }
    nodes.retain(|id| store.semantic_node_id(SemanticNodeRef::memory(*id)).is_ok());
    assert_eq!(nodes, store.node_memory_ids().into_iter().collect());
}

#[test]
fn graph_typed_publications_leave_gaps_in_memory_clock_and_lifecycle_is_owner_local() {
    let dir = scratch();
    let path = dir.join("typed.rel");
    let mut cva = Cva::create(&path).unwrap();
    let ep = episode(&mut cva);
    let a = memory(&mut cva, &ep, "a");
    let b = memory(&mut cva, &ep, "b");
    let entity = cva
        .publish_entity(
            None,
            0,
            crate::EntityDraft {
                canonical_name: "Graph entity".into(),
                aliases: vec![],
                kind: "person".into(),
                summary: "Exact summary".into(),
                mutation_id: "entity".into(),
                created_at_ns: 30,
                updated_at_ns: 30,
            },
        )
        .unwrap()
        .0;
    cva.set_entity_association(a, entity.id, true, 0).unwrap();
    let first = GraphHistory::read(&cva.container, &cva.graph).unwrap();
    assert_eq!(first.at(1, false).unwrap().memory_graph_version, 0);
    assert!(first.at(1, true).is_err());
    assert_matches_replay(&first, &cva.graph);
    cva.set_memory_relation(a, b, GraphRelationKind::Causal, true, 1)
        .unwrap();
    let principal = cva
        .publish_entity(
            None,
            0,
            crate::EntityDraft {
                canonical_name: "phy-00000000-0000-0000-0000-0000000000a1".into(),
                aliases: vec![],
                kind: crate::entity_principal::PRINCIPAL_ENTITY_KIND.into(),
                summary: "Exact principal".into(),
                mutation_id: "principal".into(),
                created_at_ns: 30,
                updated_at_ns: 30,
            },
        )
        .unwrap()
        .0;
    cva.set_principal_association(a, principal.id, true, 2)
        .unwrap();
    let history = GraphHistory::read(&cva.container, &cva.graph).unwrap();
    assert_eq!(history.at(3, false).unwrap().memory_graph_version, 2);
    assert!(history.at(3, true).is_err());
    assert_matches_replay(&history, &cva.graph);
    let memory_before = history.at(2, true).unwrap().fingerprint;
    cva.tombstone_memory(a, 1).unwrap();
    let after = GraphHistory::read(&cva.container, &cva.graph).unwrap();
    // Terminal visibility changes the current topology without inventing a Graph publication.
    assert_eq!(memory_before, after.at(2, true).unwrap().fingerprint);
    assert_matches_replay(&after, &cva.graph);
    cva.sync().unwrap();
    drop(cva);
    let reopened = Cva::open(&path).unwrap();
    let retained = GraphHistory::read(&reopened.container, &reopened.graph).unwrap();
    for cut in 0..=3 {
        assert_eq!(
            history.at(cut, false).unwrap().fingerprint,
            retained.at(cut, false).unwrap().fingerprint
        );
    }
    assert_matches_replay(&retained, &reopened.graph);
    drop(reopened);
    fs::remove_dir_all(dir).unwrap();
}
fn episode(cva: &mut Cva) -> crate::Episode {
    cva.append_node(
        "u".into(),
        "c".into(),
        None,
        "user".into(),
        10,
        "connect memories",
    )
    .unwrap();
    cva.append_node(
        "a".into(),
        "c".into(),
        Some("u".into()),
        "assistant".into(),
        20,
        "ok",
    )
    .unwrap();
    cva.materialize_path_episodes(
        "c",
        "a",
        EpisodeConfig::default(),
        EpisodeOrigin::Live,
        Some((EpisodeBoundary::Inactivity, 30)),
    )
    .unwrap()
    .created
    .into_iter()
    .next()
    .unwrap()
}

fn memory(cva: &mut Cva, episode: &crate::Episode, id: &str) -> MemoryId {
    cva.publish_memory(
        None,
        0,
        MemoryDraft {
            category: "fact".into(),
            memory_type: "project".into(),
            authority_kind: "unknown".into(),
            temporal_status: "unknown".into(),
            title: id.into(),
            content: format!("memory {id}"),
            scope: "private".into(),
            lifecycle_state: "active".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: Some("u".into()),
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: Some(episode.id),
            source_time_ns: None,
            mutation_id: id.into(),
            created_at_ns: 30,
            updated_at_ns: 30,
        },
    )
    .unwrap()
    .0
    .id
}

#[test]
fn graph_cuts_preserve_retractions_inert_payloads_and_reopen() {
    let dir = scratch();
    let path = dir.join("owner.rel");
    let mut cva = Cva::create(&path).unwrap();
    let ep = episode(&mut cva);
    let a = memory(&mut cva, &ep, "a");
    let b = memory(&mut cva, &ep, "b");
    let published = cva
        .set_memory_relation(a, b, GraphRelationKind::Factual, true, 0)
        .unwrap()
        .unwrap();
    let history = GraphHistory::read(&cva.container, &cva.graph).unwrap();
    let first = history.at(1, false).unwrap().fingerprint;
    let mut expected =
        OwnerProjection::new(cva.container.owner_uuid().unwrap(), "graph", 1, None).unwrap();
    expected
        .insert(
            "publication",
            1_u64.to_be_bytes(),
            json!({
                "global_version": published.global_version,
                "time_ns": cva.container.transaction_time_ns(published.global_version),
                "changes": [{
                    "source_kind": SemanticNodeRef::memory(a).kind.code(),
                    "source": a.0,
                    "target_kind": SemanticNodeRef::memory(b).kind.code(),
                    "target": b.0,
                    "kind": SemanticGraphRelationKind::Memory(GraphRelationKind::Factual).code(),
                    "active": true,
                    "origin": published.origin.code(),
                    "global_version": published.global_version,
                    "graph_version": published.graph_version,
                }]
            }),
        )
        .unwrap();
    expected
        .insert("clock", b"memory_graph".to_vec(), json!(1))
        .unwrap();
    assert_eq!(first, expected.fingerprint().unwrap());
    cva.set_memory_relation(a, b, GraphRelationKind::Factual, false, 1)
        .unwrap();
    // Unpublished backing must not alter semantic history.
    cva.container
        .append(&crate::graph_codec::encode_mutation(
            crate::graph_codec::GraphMutationPayload {
                source: SemanticNodeRef::memory(b),
                target: SemanticNodeRef::memory(a),
                kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Causal),
                active: true,
                origin: crate::GraphRelationOrigin::User,
            },
        ))
        .unwrap();
    let history = GraphHistory::read(&cva.container, &cva.graph).unwrap();
    assert_eq!(first, history.at(1, false).unwrap().fingerprint);
    assert!(history.at(0, false).unwrap().states.is_empty());
    assert!(
        history
            .at(1, false)
            .unwrap()
            .states
            .values()
            .all(|r| r.active)
    );
    assert!(
        history
            .at(2, false)
            .unwrap()
            .states
            .values()
            .all(|r| !r.active)
    );
    assert!(history.at(3, false).is_err());
    assert_matches_replay(&history, &cva.graph);
    cva.sync().unwrap();
    drop(cva);
    let reopened = Cva::open(&path).unwrap();
    let after = GraphHistory::read(&reopened.container, &reopened.graph).unwrap();
    for cut in 0..=2 {
        for memory_only in [false, true] {
            assert_eq!(
                history.at(cut, memory_only).unwrap().fingerprint,
                after.at(cut, memory_only).unwrap().fingerprint
            );
        }
    }
    assert_matches_replay(&after, &reopened.graph);
    drop(reopened);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
#[cfg(feature = "history-oracle-fixtures")]
fn authoritative_graph_cuts_match_replay() {
    let root = PathBuf::from(std::env::var_os("RELIQUARY_HISTORY_FIXTURES").expect("fixture root"));
    let dir = scratch();
    for (index, corpus) in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ]
    .iter()
    .enumerate()
    {
        for (kind, name) in [("rel", "project.prj.rel"), ("phy", "user.phy")] {
            let path = dir.join(format!("{index}.{kind}"));
            let mut source = fs::File::open(root.join(corpus).join(name)).unwrap();
            let mut destination = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            std::io::copy(&mut source, &mut destination).unwrap();
            drop(destination);
            let verify = |container: &Container, store: &GraphStore| {
                let head = store.graph_version();
                let history = GraphHistory::read(container, store).unwrap();
                let rescanned = GraphHistory::read(container, store).unwrap();
                for cut in [0, head.min(1), head / 2, head] {
                    assert_eq!(
                        history.at(cut, false).unwrap().fingerprint,
                        rescanned.at(cut, false).unwrap().fingerprint
                    );
                }
                assert_matches_replay(&history, store);
                eprintln!(
                    "Graph fixture {corpus}/{kind}: full {head}, memory {}",
                    store.memory_graph_version()
                );
            };
            if kind == "rel" {
                let cva = crate::Cva::open(&path).unwrap();
                verify(&cva.container, &cva.graph);
            } else {
                let phy = crate::Phylactery::open(&path).unwrap();
                verify(&phy.container, &phy.graph);
            }
        }
    }
    fs::remove_dir_all(dir).unwrap();
}
