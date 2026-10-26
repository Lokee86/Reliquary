use crate::graph_codec::{GraphVersionPayload, encode_version};
use crate::graph_compact_codec::publication::{
    self as graph_publication_codec, GraphPublication, GraphTransaction,
};
use crate::graph_compact_codec::{encode_batch, encode_mutation};
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::graph_owner::GraphOwner;
use crate::historical_graph_fingerprint::GraphHistory;
use crate::{
    Cva, GraphRelationKind, GraphRelationOrigin, MemoryDraft, Phylactery, SemanticGraphRelationKind,
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
            .join(format!(
                "graph-compact-maintenance-{}",
                uuid::Uuid::new_v4()
            ));
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

fn append_compact_history(
    container: &mut Container,
    graph_version: u64,
    unmarked: &mut Vec<u8>,
    native: bool,
) -> BTreeMap<u64, Vec<u8>> {
    *unmarked = graph_publication_codec::encode(&GraphPublication {
        graph_version: graph_version + 1000,
        transaction: GraphTransaction::Single(GraphOrdinalMutation {
            source: u32::MAX,
            target: u32::MAX - 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
            active: true,
            origin: GraphRelationOrigin::Dream,
        }),
    })
    .unwrap();
    container.append(unmarked).unwrap();

    let mut carriers = BTreeMap::new();
    let single = encode_mutation(GraphOrdinalMutation {
        source: 0,
        target: 1,
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
        active: true,
        origin: GraphRelationOrigin::User,
    });
    append_graph_publication(
        container,
        &single,
        graph_version + 1,
        710_001,
        &mut carriers,
        native,
    );
    let batch = encode_batch(&[
        GraphOrdinalMutation {
            source: 1,
            target: 0,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Causal),
            active: true,
            origin: GraphRelationOrigin::User,
        },
        GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Topical),
            active: true,
            origin: GraphRelationOrigin::User,
        },
    ])
    .unwrap();
    append_graph_publication(
        container,
        &batch,
        graph_version + 2,
        710_002,
        &mut carriers,
        native,
    );
    carriers
}

fn append_graph_publication(
    container: &mut Container,
    carrier: &[u8],
    graph_version: u64,
    time: i64,
    carriers: &mut BTreeMap<u64, Vec<u8>>,
    native: bool,
) {
    {
        let mutations = crate::graph_compact_codec::decode(carrier)
            .unwrap()
            .unwrap();
        let transaction = if mutations.len() == 1 {
            GraphTransaction::Single(mutations[0])
        } else {
            GraphTransaction::Batch(mutations)
        };
        let body = graph_publication_codec::encode(&GraphPublication {
            graph_version,
            transaction,
        })
        .unwrap();
        container.set_next_transaction_time_override(Some(time));
        container.append_publication_envelope(&body).unwrap();
        carriers.insert(graph_version, body);
        return;
    }
}

fn graph_refs(container: &mut Container) -> BTreeMap<u64, reliquary_container::ObjectRef> {
    container
        .chunks()
        .unwrap()
        .into_iter()
        .filter_map(|object| {
            let payload = container.read(object).unwrap();
            graph_publication_codec::version_at(object, &payload)
                .unwrap()
                .map(|version| (version.graph_version, version.mutation))
        })
        .collect()
}

fn payloads(container: &mut Container) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    let source = container.read_source().unwrap();
    let mut cursor = source.physical_cursor().unwrap();
    while let Some(frame) = cursor.next_frame().unwrap() {
        result.push(frame.payload);
    }
    result
}

fn assert_graph_history_equal(
    before_container: &Container,
    before_graph: &GraphOwner,
    after_container: &Container,
    after_graph: &GraphOwner,
) {
    let before = GraphHistory::read(before_container, before_graph).unwrap();
    let after = GraphHistory::read(after_container, after_graph).unwrap();
    for cut in 0..=before_graph.graph_version() {
        let before_cut = before.at(cut, false).unwrap();
        let after_cut = after.at(cut, false).unwrap();
        assert_eq!(
            before_cut.fingerprint, after_cut.fingerprint,
            "Graph cut {cut}"
        );
        assert_eq!(
            before_cut.states, after_cut.states,
            "Graph states at cut {cut}"
        );
        assert_eq!(
            before_cut.memory_graph_version, after_cut.memory_graph_version,
            "MemoryGraph head at cut {cut}"
        );
        for relation in before_cut.states.values() {
            assert_eq!(
                before_container.transaction_time_ns(relation.global_version),
                after_container.transaction_time_ns(relation.global_version),
                "transaction time at Graph cut {cut}"
            );
        }
    }
    assert_eq!(
        before.memory_graph_positions(),
        after.memory_graph_positions()
    );
    for cut in before.memory_graph_positions() {
        assert_eq!(
            before.at(cut, true).unwrap().fingerprint,
            after.at(cut, true).unwrap().fingerprint,
            "MemoryGraph cut {cut}"
        );
    }
}

fn accepted_graph_cuts(
    container: &mut Container,
    graph: &GraphOwner,
) -> Vec<(
    BTreeMap<
        (
            crate::SemanticNodeRef,
            crate::SemanticNodeRef,
            SemanticGraphRelationKind,
        ),
        crate::SemanticGraphRelation,
    >,
    u64,
)> {
    let mut states = BTreeMap::new();
    let mut memory_graph_version = 0;
    let mut cuts = vec![(states.clone(), memory_graph_version)];
    for object in container.chunks().unwrap() {
        let payload = container.read(object).unwrap();
        let Some(version) = graph_publication_codec::version_at(object, &payload).unwrap() else {
            continue;
        };
        let bytes = container.read(version.mutation).unwrap();
        let mutations = graph.decode_stored_mutations(&bytes).unwrap().unwrap();
        for mutation in mutations {
            let relation = crate::SemanticGraphRelation {
                source: mutation.source.into(),
                target: mutation.target.into(),
                kind: mutation.kind,
                active: mutation.active,
                origin: mutation.origin,
                global_version: version.global_version,
                graph_version: version.graph_version,
            };
            if matches!(relation.kind, SemanticGraphRelationKind::Memory(_)) {
                memory_graph_version = version.graph_version;
            }
            states.insert((relation.source, relation.target, relation.kind), relation);
        }
        cuts.push((states.clone(), memory_graph_version));
    }
    cuts
}

fn append_rel_history(path: &Path, native: bool) -> (Vec<u8>, BTreeMap<u64, Vec<u8>>) {
    let mut rel = Cva::create_project(path).unwrap();
    let ids = [
        crate::cva_reconcile_graph_test_support::publish_memory(&mut rel, "reclaim-a"),
        crate::cva_reconcile_graph_test_support::publish_memory(&mut rel, "reclaim-b"),
    ];
    rel.set_memory_relation_with_origin(
        ids[0],
        ids[1],
        GraphRelationKind::Causal,
        true,
        GraphRelationOrigin::User,
        rel.graph_version(),
    )
    .unwrap();
    let mut orphan = Vec::new();
    let graph_version = rel.graph_version();
    let carriers = append_compact_history(&mut rel.container, graph_version, &mut orphan, native);
    rel.sync().unwrap();
    drop(rel);
    (orphan, carriers)
}

fn append_phy_history(path: &Path, native: bool) -> (Vec<u8>, BTreeMap<u64, Vec<u8>>) {
    let mut phy = Phylactery::create(path).unwrap();
    let (first, _) = phy.publish_memory(None, 0, memory_draft("phy-a")).unwrap();
    let (second, _) = phy.publish_memory(None, 0, memory_draft("phy-b")).unwrap();
    phy.set_memory_relation_with_origin(
        first.id,
        second.id,
        GraphRelationKind::Causal,
        true,
        GraphRelationOrigin::User,
        phy.graph_version(),
    )
    .unwrap();
    let mut orphan = Vec::new();
    let graph_version = phy.graph_version();
    let carriers = append_compact_history(&mut phy.container, graph_version, &mut orphan, native);
    phy.sync().unwrap();
    drop(phy);
    (orphan, carriers)
}

fn assert_reclaimed_graph(
    source_path: &Path,
    output_path: &Path,
    expected_orphan: &[u8],
    expected_compact: &BTreeMap<u64, Vec<u8>>,
    rel: bool,
) {
    let source_refs = {
        let mut source_container = Container::open(source_path).unwrap();
        graph_refs(&mut source_container)
    };
    let source_bytes = fs::read(source_path).unwrap();
    let source_owner_uuid;
    let source_nodes;
    if rel {
        let source = Cva::open(source_path).unwrap();
        source_owner_uuid = source.owner_uuid();
        source_nodes = source.graph.semantic_nodes();
        let before = GraphHistory::read(&source.container, &source.graph).unwrap();
        let before_cuts: Vec<_> = (0..=source.graph.graph_version())
            .map(|cut| before.at(cut, false).unwrap().fingerprint)
            .collect();
        let before_memory_cuts: Vec<_> = before
            .memory_graph_positions()
            .into_iter()
            .map(|cut| before.at(cut, true).unwrap().fingerprint)
            .collect();
        drop(source);

        let mut owner = Cva::open(source_path).unwrap();
        let report = owner.reclaim_storage(output_path).unwrap();
        assert_eq!(report.unpublished_backing_chunks, 1);
        assert_eq!(fs::read(source_path).unwrap(), source_bytes);
        let mut output = Cva::open(output_path).unwrap();
        assert_eq!(output.owner_uuid(), source_owner_uuid);
        assert_eq!(output.graph.semantic_nodes(), source_nodes);
        let after = GraphHistory::read(&output.container, &output.graph).unwrap();
        for (cut, expected) in before_cuts.iter().enumerate() {
            assert_eq!(&after.at(cut as u64, false).unwrap().fingerprint, expected);
        }
        for (index, cut) in after.memory_graph_positions().into_iter().enumerate() {
            assert_eq!(
                after.at(cut, true).unwrap().fingerprint,
                before_memory_cuts[index]
            );
        }
        assert_graph_history_equal(
            &owner.container,
            &owner.graph,
            &output.container,
            &output.graph,
        );
        assert_compact_carriers_and_relocated_markers(
            &mut output.container,
            &source_refs,
            expected_orphan,
            expected_compact,
        );
    } else {
        let source = Phylactery::open(source_path).unwrap();
        source_owner_uuid = source.owner_uuid();
        source_nodes = source.graph.semantic_nodes();
        let before = GraphHistory::read(&source.container, &source.graph).unwrap();
        let before_cuts: Vec<_> = (0..=source.graph.graph_version())
            .map(|cut| before.at(cut, false).unwrap().fingerprint)
            .collect();
        let before_memory_cuts: Vec<_> = before
            .memory_graph_positions()
            .into_iter()
            .map(|cut| before.at(cut, true).unwrap().fingerprint)
            .collect();
        drop(source);

        let mut owner = Phylactery::open(source_path).unwrap();
        let report = owner.reclaim_storage(output_path).unwrap();
        assert_eq!(report.unpublished_backing_chunks, 1);
        assert_eq!(fs::read(source_path).unwrap(), source_bytes);
        let mut output = Phylactery::open(output_path).unwrap();
        assert_eq!(output.owner_uuid(), source_owner_uuid);
        assert_eq!(output.graph.semantic_nodes(), source_nodes);
        let after = GraphHistory::read(&output.container, &output.graph).unwrap();
        for (cut, expected) in before_cuts.iter().enumerate() {
            assert_eq!(&after.at(cut as u64, false).unwrap().fingerprint, expected);
        }
        for (index, cut) in after.memory_graph_positions().into_iter().enumerate() {
            assert_eq!(
                after.at(cut, true).unwrap().fingerprint,
                before_memory_cuts[index]
            );
        }
        assert_graph_history_equal(
            &owner.container,
            &owner.graph,
            &output.container,
            &output.graph,
        );
        assert_compact_carriers_and_relocated_markers(
            &mut output.container,
            &source_refs,
            expected_orphan,
            expected_compact,
        );
    }
}

fn memory_draft(mutation_id: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: mutation_id.into(),
        content: format!("content for {mutation_id}"),
        scope: "private".into(),
        lifecycle_state: "extracted".into(),
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

fn assert_compact_carriers_and_relocated_markers(
    output: &mut Container,
    source_refs: &BTreeMap<u64, reliquary_container::ObjectRef>,
    orphan: &[u8],
    expected: &BTreeMap<u64, Vec<u8>>,
) {
    let output_refs = graph_refs(output);
    let output_payloads = payloads(output);
    assert!(!output_payloads.iter().any(|payload| payload == orphan));
    for (graph_version, expected_payload) in expected {
        let old_ref = source_refs[graph_version];
        let new_ref = output_refs[graph_version];
        assert_ne!(
            old_ref, new_ref,
            "published carrier should relocate after orphan removal"
        );
        let stored = output.read(new_ref).unwrap();
        let body = reliquary_container::publication_envelope::decode(&stored)
            .unwrap()
            .map(|envelope| envelope.body)
            .unwrap_or(&stored);
        assert_eq!(body, expected_payload.as_slice());
        assert!(output_payloads.iter().any(|payload| {
            payload == expected_payload
                || reliquary_container::publication_envelope::decode(payload)
                    .unwrap()
                    .is_some_and(|envelope| envelope.body == expected_payload)
        }));
    }
}

#[test]
fn rel_compact_graph_reclamation_preserves_all_cuts_and_relocates_only_published_carriers() {
    let scratch = Scratch::new();
    let source = scratch.file("source.rel");
    let output = scratch.file("reclaimed.rel");
    let (orphan, compact) = append_rel_history(&source, false);
    assert_reclaimed_graph(&source, &output, &orphan, &compact, true);
}

#[test]
fn phy_compact_graph_reclamation_preserves_all_cuts_and_relocates_only_published_carriers() {
    let scratch = Scratch::new();
    let source = scratch.file("source.phy");
    let output = scratch.file("reclaimed.phy");
    let (orphan, compact) = append_phy_history(&source, false);
    assert_reclaimed_graph(&source, &output, &orphan, &compact, false);
}

#[test]
fn native_graph_reclamation_preserves_ordered_single_and_batch_cuts_and_source_bytes() {
    for rel in [true, false] {
        let scratch = Scratch::new();
        let source = scratch.file(if rel {
            "native-source.rel"
        } else {
            "native-source.phy"
        });
        let output = scratch.file(if rel {
            "native-reclaimed.rel"
        } else {
            "native-reclaimed.phy"
        });
        let (orphan, publications) = if rel {
            append_rel_history(&source, true)
        } else {
            append_phy_history(&source, true)
        };
        assert_eq!(publications.len(), 2);
        let batch_payload = publications.values().nth(1).unwrap();
        let batch = graph_publication_codec::decode(batch_payload, 2)
            .unwrap()
            .unwrap();
        let GraphTransaction::Batch(batch) = batch.transaction else {
            panic!("native second publication should preserve its batch shape");
        };
        assert_eq!(batch.len(), 2);
        assert_eq!(
            batch[0].kind,
            SemanticGraphRelationKind::Memory(GraphRelationKind::Causal)
        );
        assert_eq!(
            batch[1].kind,
            SemanticGraphRelationKind::Memory(GraphRelationKind::Topical)
        );
        assert_reclaimed_graph(&source, &output, &orphan, &publications, rel);
    }
}

#[test]
fn compact_graph_cuts_survive_native_rel_principal_and_attachment_repacks() {
    for native in [false, true] {
        run_graph_repack_history(native);
    }
}

#[test]
fn native_graph_fresh_rewrite_keeps_baseline_relations_and_accepts_followup_writes() {
    use crate::rewrite::{RewriteRequest, rewrite_file};
    for rel_owner in [true, false] {
        let scratch = Scratch::new();
        let source = scratch.file(if rel_owner {
            "rewrite-native.rel"
        } else {
            "rewrite-native.phy"
        });
        let output = scratch.file(if rel_owner {
            "rewritten-native.rel"
        } else {
            "rewritten-native.phy"
        });
        let ids = if rel_owner {
            let mut owner = Cva::create_project(&source).unwrap();
            let ids = ["rewrite-native-a", "rewrite-native-b", "rewrite-native-c"].map(|name| {
                crate::cva_reconcile_graph_test_support::publish_memory(&mut owner, name)
            });
            append_native_graph_facts(&mut owner.container, &owner.graph, ids, 90_001);
            owner.sync().unwrap();
            ids
        } else {
            let mut owner = Phylactery::create(&source).unwrap();
            let ids = ["rewrite-native-a", "rewrite-native-b", "rewrite-native-c"].map(|name| {
                owner
                    .publish_memory(None, 0, memory_draft(name))
                    .unwrap()
                    .0
                    .id
            });
            append_native_graph_facts(&mut owner.container, &owner.graph, ids, 90_001);
            owner.sync().unwrap();
            ids
        };
        rewrite_file(&source, &output, &RewriteRequest::default()).unwrap();
        if rel_owner {
            let mut rewritten = Cva::open(&output).unwrap();
            assert_eq!(rewritten.graph_version(), 1);
            assert_eq!(
                rewritten
                    .graph
                    .rewrite_baseline_memories()
                    .unwrap()
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>(),
                ids.into_iter().collect()
            );
            assert_eq!(rewritten.semantic_graph_relations().len(), 3);
            assert!(
                GraphHistory::read(&rewritten.container, &rewritten.graph)
                    .unwrap()
                    .at(0, false)
                    .unwrap()
                    .states
                    .is_empty()
            );
            assert_eq!(
                rewritten
                    .semantic_graph_relations()
                    .iter()
                    .filter(|r| r.active)
                    .count(),
                3
            );
            rewritten
                .set_memory_relation_with_origin(
                    ids[0],
                    ids[2],
                    GraphRelationKind::References,
                    true,
                    GraphRelationOrigin::User,
                    rewritten.graph_version(),
                )
                .unwrap();
            assert_eq!(rewritten.graph_version(), 2);
        } else {
            let mut rewritten = Phylactery::open(&output).unwrap();
            assert_eq!(rewritten.graph_version(), 1);
            assert_eq!(
                rewritten
                    .graph
                    .rewrite_baseline_memories()
                    .unwrap()
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>(),
                ids.into_iter().collect()
            );
            assert_eq!(rewritten.semantic_graph_relations().len(), 3);
            assert!(
                GraphHistory::read(&rewritten.container, &rewritten.graph)
                    .unwrap()
                    .at(0, false)
                    .unwrap()
                    .states
                    .is_empty()
            );
            assert_eq!(
                rewritten
                    .semantic_graph_relations()
                    .iter()
                    .filter(|r| r.active)
                    .count(),
                3
            );
            rewritten
                .set_memory_relation_with_origin(
                    ids[0],
                    ids[2],
                    GraphRelationKind::References,
                    true,
                    GraphRelationOrigin::User,
                    rewritten.graph_version(),
                )
                .unwrap();
            assert_eq!(rewritten.graph_version(), 2);
        }
    }
}

fn append_native_graph_facts(
    container: &mut Container,
    graph: &GraphOwner,
    ids: [crate::MemoryId; 3],
    time: i64,
) {
    assert_eq!(graph.stats().nodes, 0);
    for (ordinal, id) in ids.iter().enumerate() {
        container
            .append(&crate::graph_codec::encode_node(
                crate::graph_codec::GraphNodePayload {
                    semantic_node: crate::SemanticNodeRef::memory(*id).into(),
                    node_id: arcana::NodeId(ordinal as u32),
                },
            ))
            .unwrap();
    }
    let ordinal = |id| ids.iter().position(|candidate| *candidate == id).unwrap() as u32;
    let single = encode_mutation(GraphOrdinalMutation {
        source: ordinal(ids[0]),
        target: ordinal(ids[1]),
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
        active: true,
        origin: GraphRelationOrigin::User,
    });
    let batch = encode_batch(&[
        GraphOrdinalMutation {
            source: ordinal(ids[1]),
            target: ordinal(ids[2]),
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Causal),
            active: true,
            origin: GraphRelationOrigin::User,
        },
        GraphOrdinalMutation {
            source: ordinal(ids[2]),
            target: ordinal(ids[0]),
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Topical),
            active: true,
            origin: GraphRelationOrigin::User,
        },
    ])
    .unwrap();
    let version = graph.graph_version();
    let mut carriers = BTreeMap::new();
    append_graph_publication(container, &single, version + 1, time, &mut carriers, true);
    append_graph_publication(
        container,
        &batch,
        version + 2,
        time + 1,
        &mut carriers,
        true,
    );
}

fn run_graph_repack_history(native: bool) {
    use crate::{
        IncomingAttachment, IncomingTurn, ProjectFileRef, ProjectRepositoryKind,
        ProjectRepositoryRef, ProjectRevisionRef,
    };
    use sha2::{Digest, Sha256};
    let scratch = Scratch::new();
    let source = scratch.file(if native {
        "native-source.rel"
    } else {
        "legacy-source.rel"
    });
    append_rel_history(&source, native);
    let mut rel = Cva::open(&source).unwrap();
    let attachment_bytes = b"Graph native repack attachment";
    let ingested = rel
        .ingest_turn(IncomingTurn {
            id: "graph-native-turn".into(),
            conversation_id: "graph-native-conversation".into(),
            parent_id: None,
            role: "user".into(),
            principal_id: None,
            timestamp_ns: 10,
            content: "Use the attached source.".into(),
            attachments: vec![IncomingAttachment {
                filename: "source.txt".into(),
                mime_type: Some("text/plain".into()),
                bytes: attachment_bytes.to_vec(),
            }],
            project_attachments: Vec::new(),
        })
        .unwrap();
    rel.bind_legacy_project_file(
        ingested.attachments[0].id,
        ProjectFileRef {
            revision: ProjectRevisionRef {
                repository: ProjectRepositoryRef {
                    kind: ProjectRepositoryKind::Lore,
                    repository_id: "graph-native-repo".into(),
                    project_path: ".".into(),
                },
                revision: "graph-native-revision".into(),
            },
            path: "uploads/source.txt".into(),
            content_hash: Some(Sha256::digest(attachment_bytes).into()),
        },
    )
    .unwrap();
    rel.sync().unwrap();
    let principal = scratch.file("principal.rel");
    rel.repack_missing_user_principal(&principal, "phy-00000000-0000-0000-0000-000000000123")
        .unwrap();
    let attachments = scratch.file("attachments.rel");
    rel.repack_project_backed_attachments(&attachments).unwrap();
    for output in [principal, attachments] {
        let reopened = Cva::open(output).unwrap();
        assert_graph_history_equal(
            &rel.container,
            &rel.graph,
            &reopened.container,
            &reopened.graph,
        );
    }
}
