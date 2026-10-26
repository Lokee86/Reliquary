use crate::graph_codec::decode_node;
use crate::graph_compact_codec::publication;
use crate::{
    Cva, EntityDraft, GraphRelationChange, GraphRelationKind, GraphRelationOrigin, MemoryDraft,
    MemoryId, Phylactery, SemanticGraphRelationChange, SemanticGraphRelationKind,
};
use reliquary_container::Container;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("graph-compact-writer-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn memory_draft(name: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "unknown".into(),
        temporal_status: "unknown".into(),
        title: name.into(),
        content: format!("body {name}"),
        scope: "private".into(),
        lifecycle_state: "active".into(),
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
        mutation_id: format!("create-{name}"),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn entity_draft(name: &str) -> EntityDraft {
    EntityDraft {
        canonical_name: name.into(),
        aliases: vec![],
        kind: "person".into(),
        summary: format!("Summary for {name}"),
        mutation_id: format!("entity-{name}"),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn payloads_from(container: &Container) -> Vec<(reliquary_container::ObjectRef, Vec<u8>)> {
    let mut values = Vec::new();
    let source = container.read_source().unwrap();
    let mut cursor = source.physical_cursor().unwrap();
    while let Some(frame) = cursor.next_frame().unwrap() {
        values.push((frame.object, frame.payload));
    }
    values
}

fn payloads(path: &Path) -> Vec<(reliquary_container::ObjectRef, Vec<u8>)> {
    let container = Container::open_read_only(path).unwrap();
    payloads_from(&container)
}

fn assert_compact_publications_from(
    container: &Container,
    expected_single: usize,
    expected_batch: usize,
) {
    let values = payloads_from(container);
    let mut mapping_positions = BTreeMap::new();
    let mut compact_single_count = 0;
    let mut compact_batch_count = 0;
    for (index, (_, bytes)) in values.iter().enumerate() {
        if let Some(node) = decode_node(bytes).unwrap() {
            mapping_positions.insert(node.node_id.0, index);
        }
        if bytes.starts_with(b"CVAGMUT") || bytes.starts_with(b"CVAGBAT") {
            panic!(
                "legacy Graph carrier found in native writer fixture: {:?}",
                &bytes[..8]
            );
        }
        assert!(!bytes.starts_with(b"CVAGCM01") && !bytes.starts_with(b"CVAGCB01"));
    }
    let mut publication_count = 0;
    for (index, (_, bytes)) in values.iter().enumerate() {
        let Some(envelope) = reliquary_container::publication_envelope::decode(bytes).unwrap()
        else {
            continue;
        };
        let Some(publication) =
            publication::decode(envelope.body, envelope.global_version).unwrap()
        else {
            continue;
        };
        publication_count += 1;
        if publication.is_single() {
            compact_single_count += 1;
        } else {
            compact_batch_count += 1;
        }
        for mutation in publication.mutations() {
            for ordinal in [mutation.source, mutation.target] {
                let mapped_at = mapping_positions.get(&ordinal).unwrap_or_else(|| {
                    panic!("ordinal {ordinal} has no mapping before Graph envelope")
                });
                assert!(
                    *mapped_at < index,
                    "every ordinal mapping precedes publication"
                );
            }
        }
    }
    assert_eq!(compact_single_count, expected_single);
    assert_eq!(compact_batch_count, expected_batch);
    assert_eq!(publication_count, expected_single + expected_batch);
}

fn assert_compact_publications(path: &Path, expected_single: usize, expected_batch: usize) {
    let container = Container::open_read_only(path).unwrap();
    assert_compact_publications_from(&container, expected_single, expected_batch);
}

fn publish_graph_changes_rel(owner: &mut Cva, ids: [MemoryId; 3]) {
    let initial_graph_version = owner.graph_version();
    owner
        .set_memory_relation(
            ids[0],
            ids[1],
            GraphRelationKind::Factual,
            true,
            initial_graph_version,
        )
        .unwrap();
    let batch = [
        GraphRelationChange {
            source: ids[1],
            target: ids[2],
            kind: GraphRelationKind::Causal,
            active: true,
        },
        GraphRelationChange {
            source: ids[2],
            target: ids[0],
            kind: GraphRelationKind::Topical,
            active: true,
        },
    ];
    owner
        .set_memory_relations(&batch, owner.graph_version())
        .unwrap();
    let first = owner
        .publish_entity(None, 0, entity_draft("first"))
        .unwrap()
        .0;
    let second = owner
        .publish_entity(None, 0, entity_draft("second"))
        .unwrap()
        .0;
    owner
        .set_entity_association(ids[0], first.id, true, owner.graph_version())
        .unwrap();
    let changes = [
        SemanticGraphRelationChange::entity_association(ids[1], first.id, true),
        SemanticGraphRelationChange::entity_association(ids[2], second.id, true),
    ];
    let graph_version = owner.graph_version();
    owner
        .graph
        .set_semantic_relations_with_origin(
            &mut owner.container,
            &owner.memories,
            &owner.entities,
            &changes,
            GraphRelationOrigin::Perception,
            graph_version,
        )
        .unwrap();
}

fn publish_graph_changes_phy(owner: &mut Phylactery, ids: [MemoryId; 3]) {
    let initial_graph_version = owner.graph_version();
    owner
        .set_memory_relation(
            ids[0],
            ids[1],
            GraphRelationKind::Factual,
            true,
            initial_graph_version,
        )
        .unwrap();
    let batch = [
        GraphRelationChange {
            source: ids[1],
            target: ids[2],
            kind: GraphRelationKind::Causal,
            active: true,
        },
        GraphRelationChange {
            source: ids[2],
            target: ids[0],
            kind: GraphRelationKind::Topical,
            active: true,
        },
    ];
    owner
        .set_memory_relations(&batch, owner.graph_version())
        .unwrap();
    let first = owner
        .publish_entity(None, 0, entity_draft("first"))
        .unwrap()
        .0;
    let second = owner
        .publish_entity(None, 0, entity_draft("second"))
        .unwrap()
        .0;
    owner
        .set_entity_association(ids[0], first.id, true, owner.graph_version())
        .unwrap();
    let changes = [
        SemanticGraphRelationChange::entity_association(ids[1], first.id, true),
        SemanticGraphRelationChange::entity_association(ids[2], second.id, true),
    ];
    let graph_version = owner.graph_version();
    owner
        .graph
        .set_semantic_relations_with_origin(
            &mut owner.container,
            &owner.memories,
            &owner.entities,
            &changes,
            GraphRelationOrigin::Perception,
            graph_version,
        )
        .unwrap();
}

fn verify_owner_reopen_rel(path: &Path, owner: Cva) {
    let expected_relations = owner.semantic_graph_relations();
    let expected_graph_version = owner.graph_version();
    let expected_memory_graph_version = owner.memory_graph_version();
    let expected_latest = owner.latest_global_version();
    let expected_times = expected_relations
        .iter()
        .map(|relation| {
            (
                relation.global_version,
                owner.transaction_time_ns(relation.global_version),
            )
        })
        .collect::<Vec<_>>();
    owner.sync().unwrap();
    drop(owner);
    assert_compact_publications(path, 2, 2);
    let reopened = Cva::open(path).unwrap();
    assert_eq!(reopened.semantic_graph_relations(), expected_relations);
    assert_eq!(reopened.graph_version(), expected_graph_version);
    assert_eq!(
        reopened.memory_graph_version(),
        expected_memory_graph_version
    );
    assert_eq!(reopened.latest_global_version(), expected_latest);
    for (global, time) in expected_times {
        assert_eq!(reopened.transaction_time_ns(global), time);
    }
}

#[test]
fn rel_memory_and_typed_entity_single_and_batch_writers_reopen_and_noop_exactly() {
    let scratch = Scratch::new();
    let path = scratch.file("writer.rel");
    let mut owner = Cva::create(&path).unwrap();
    let ids = ["a", "b", "c"].map(|name| {
        owner
            .publish_memory(None, 0, memory_draft(name))
            .unwrap()
            .0
            .id
    });
    publish_graph_changes_rel(&mut owner, ids);
    assert_compact_publications_from(&owner.container, 2, 2);
    owner.sync().unwrap();
    let bytes = fs::read(&path).unwrap();
    let global = owner.latest_global_version();
    let graph_version = owner.graph_version();
    let graph = owner.semantic_graph_relations();
    assert!(
        owner
            .set_memory_relation(
                ids[0],
                ids[1],
                GraphRelationKind::Factual,
                true,
                graph_version,
            )
            .unwrap()
            .is_none()
    );
    let noop_batch = [
        GraphRelationChange {
            source: ids[1],
            target: ids[2],
            kind: GraphRelationKind::Causal,
            active: true,
        },
        GraphRelationChange {
            source: ids[2],
            target: ids[0],
            kind: GraphRelationKind::Topical,
            active: true,
        },
    ];
    assert!(
        owner
            .set_memory_relations(&noop_batch, graph_version)
            .unwrap()
            .is_empty()
    );
    assert!(
        owner
            .set_entity_association(
                ids[0],
                owner
                    .entities()
                    .into_iter()
                    .find(|entity| entity.canonical_name == "first")
                    .unwrap()
                    .id,
                true,
                graph_version
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(owner.latest_global_version(), global);
    assert_eq!(owner.graph_version(), graph_version);
    assert_eq!(owner.semantic_graph_relations(), graph);
    verify_owner_reopen_rel(&path, owner);
}

#[test]
fn phy_memory_and_typed_entity_single_and_batch_writers_reopen_and_noop_exactly() {
    let scratch = Scratch::new();
    let path = scratch.file("writer.phy");
    let mut owner = Phylactery::create(&path).unwrap();
    let ids = ["a", "b", "c"].map(|name| {
        owner
            .publish_memory(None, 0, memory_draft(name))
            .unwrap()
            .0
            .id
    });
    publish_graph_changes_phy(&mut owner, ids);
    assert_compact_publications_from(&owner.container, 2, 2);
    owner.sync().unwrap();
    let bytes = fs::read(&path).unwrap();
    let global = owner.latest_global_version();
    let graph_version = owner.graph_version();
    let graph = owner.semantic_graph_relations();
    assert!(
        owner
            .set_memory_relation(
                ids[0],
                ids[1],
                GraphRelationKind::Factual,
                true,
                graph_version,
            )
            .unwrap()
            .is_none()
    );
    let noop_batch = [
        GraphRelationChange {
            source: ids[1],
            target: ids[2],
            kind: GraphRelationKind::Causal,
            active: true,
        },
        GraphRelationChange {
            source: ids[2],
            target: ids[0],
            kind: GraphRelationKind::Topical,
            active: true,
        },
    ];
    assert!(
        owner
            .set_memory_relations(&noop_batch, graph_version)
            .unwrap()
            .is_empty()
    );
    assert!(
        owner
            .set_entity_association(
                ids[0],
                owner
                    .entities()
                    .into_iter()
                    .find(|entity| entity.canonical_name == "first")
                    .unwrap()
                    .id,
                true,
                graph_version
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(owner.latest_global_version(), global);
    assert_eq!(owner.graph_version(), graph_version);
    assert_eq!(owner.semantic_graph_relations(), graph);
    owner.sync().unwrap();
    drop(owner);
    assert_compact_publications(&path, 2, 2);
    let reopened = Phylactery::open(&path).unwrap();
    assert_eq!(reopened.semantic_graph_relations(), graph);
    assert_eq!(reopened.graph_version(), graph_version);
    assert_eq!(reopened.latest_global_version(), global);
}

#[test]
fn fresh_history_rewrite_emits_compact_graph_and_preserves_graph_zero_isolated_baseline() {
    let scratch = Scratch::new();
    let source = scratch.file("rewrite-source.rel");
    let output = scratch.file("rewrite-output.rel");
    let mut owner = Cva::create_project(&source).unwrap();
    let a = owner
        .publish_memory(None, 0, memory_draft("rewrite-a"))
        .unwrap()
        .0;
    let b = owner
        .publish_memory(None, 0, memory_draft("rewrite-b"))
        .unwrap()
        .0;
    owner
        .set_memory_relation(a.id, b.id, GraphRelationKind::Factual, true, 0)
        .unwrap();
    owner.sync().unwrap();
    drop(owner);

    crate::rewrite::rewrite_file(&source, &output, &crate::RewriteRequest::default()).unwrap();
    let rewritten = Cva::open(&output).unwrap();
    assert_eq!(rewritten.graph_version(), 1);
    assert_eq!(rewritten.semantic_graph_relations().len(), 1);
    assert_compact_publications_from(&rewritten.container, 1, 0);
    let history = crate::historical_graph_fingerprint::GraphHistory::read(
        &rewritten.container,
        &rewritten.graph,
    )
    .unwrap();
    assert_eq!(
        rewritten
            .graph
            .rewrite_baseline_memories()
            .unwrap()
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([a.id, b.id])
    );
    let zero = history.at(0, true).unwrap();
    assert!(zero.states.is_empty());
    assert_eq!(zero.memory_graph_version, 0);
    assert_eq!(history.at(1, true).unwrap().memory_graph_version, 1);
}

#[test]
fn every_incomplete_native_graph_transaction_envelope_recovers_before_retry() {
    let scratch = Scratch::new();
    let complete_path = scratch.file("complete.rel");
    let mut owner = Cva::create_project(&complete_path).unwrap();
    let ids = ["partial-a", "partial-b"].map(|name| {
        owner
            .publish_memory(None, 0, memory_draft(name))
            .unwrap()
            .0
            .id
    });
    let prior_global = owner.latest_global_version();
    owner
        .set_memory_relation(
            ids[0],
            ids[1],
            GraphRelationKind::Factual,
            true,
            owner.graph_version(),
        )
        .unwrap();
    owner.sync().unwrap();
    drop(owner);

    let bytes = fs::read(&complete_path).unwrap();
    let mut container = Container::open(&complete_path).unwrap();
    let mut envelope_start = None;
    let mut envelope_len = None;
    let source = container.read_source().unwrap();
    let mut cursor = source.physical_cursor().unwrap();
    while let Some(frame) = cursor.next_frame().unwrap() {
        if reliquary_container::publication_envelope::decode(&frame.payload)
            .unwrap()
            .is_some_and(|envelope| {
                publication::decode(envelope.body, envelope.global_version)
                    .unwrap()
                    .is_some()
            })
        {
            let reference = frame.object.legacy_bytes();
            envelope_start = Some(u64::from_le_bytes(reference[..8].try_into().unwrap()) as usize);
            envelope_len = Some(frame.payload.len());
        }
    }
    let start = envelope_start.unwrap();
    let frame_len = 8 + envelope_len.unwrap();
    assert_eq!(start + frame_len, bytes.len());

    for prefix_len in 0..frame_len {
        let path = scratch.file(&format!("partial-{prefix_len}.rel"));
        fs::write(&path, &bytes[..start + prefix_len]).unwrap();
        if prefix_len > 0 {
            crate::container_owner_tests::assert_normal_open_requires_repair(
                &path,
                |path| {
                    Cva::open(path)
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                },
                start as u64,
            );
            crate::container_owner_tests::repair_incomplete_physical_tail(&path, start as u64);
        }
        let mut reopened = Cva::open(&path).unwrap();
        assert_eq!(reopened.graph_version(), 0, "prefix {prefix_len}");
        assert_eq!(reopened.latest_global_version(), prior_global);
        reopened
            .set_memory_relation(
                ids[0],
                ids[1],
                GraphRelationKind::Factual,
                true,
                reopened.graph_version(),
            )
            .unwrap();
        reopened.sync().unwrap();
        drop(reopened);
        let recovered = Cva::open(&path).unwrap();
        assert_eq!(recovered.graph_version(), 1, "prefix {prefix_len}");
        assert_eq!(recovered.latest_global_version(), prior_global + 1);
        assert_eq!(recovered.semantic_graph_relations().len(), 1);
    }
}
