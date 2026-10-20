use crate::graph_codec::{GraphVersionPayload, decode_version, encode_version};
use crate::graph_compact_codec::{encode_batch, encode_mutation};
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::graph_store::GraphStore;
use crate::historical_graph_fingerprint::GraphHistory;
use crate::{
    Container, ContainerError, Cva, GraphRelationKind, GraphRelationOrigin, MemoryDraft,
    Phylactery, SemanticGraphRelationKind,
};
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
) -> BTreeMap<u64, Vec<u8>> {
    *unmarked = encode_mutation(GraphOrdinalMutation {
        source: u32::MAX,
        target: u32::MAX - 1,
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
        active: true,
        origin: GraphRelationOrigin::Dream,
    });
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
    append_graph_publication(container, &batch, graph_version + 2, 710_002, &mut carriers);
    carriers
}

fn append_graph_publication(
    container: &mut Container,
    carrier: &[u8],
    graph_version: u64,
    time: i64,
    carriers: &mut BTreeMap<u64, Vec<u8>>,
) {
    let mutation = container.append(carrier).unwrap();
    let global_version = container
        .allocate_version_with_transaction_time(Some(time))
        .unwrap();
    container
        .append(&encode_version(GraphVersionPayload {
            global_version,
            graph_version,
            mutation,
        }))
        .unwrap();
    carriers.insert(graph_version, carrier.to_vec());
}

fn graph_refs(container: &mut Container) -> BTreeMap<u64, crate::ObjectRef> {
    container
        .chunks()
        .unwrap()
        .into_iter()
        .filter_map(|object| {
            let payload = container.read(object).unwrap();
            decode_version(&payload)
                .unwrap()
                .map(|version| (version.graph_version, version.mutation))
        })
        .collect()
}

fn payloads(container: &mut Container) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    container
        .visit_payloads::<ContainerError>(|_, payload| {
            result.push(payload.to_vec());
            Ok(())
        })
        .unwrap();
    result
}

fn assert_graph_history_equal(
    before_container: &Container,
    before_graph: &GraphStore,
    after_container: &Container,
    after_graph: &GraphStore,
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
    graph: &GraphStore,
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
        let Some(version) = decode_version(&payload).unwrap() else {
            continue;
        };
        let bytes = container.read(version.mutation).unwrap();
        let mutations = graph.decode_stored_mutations(&bytes).unwrap().unwrap();
        for mutation in mutations {
            let relation = crate::SemanticGraphRelation {
                source: mutation.source,
                target: mutation.target,
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

fn append_rel_history(path: &Path) -> (Vec<u8>, BTreeMap<u64, Vec<u8>>) {
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
    let carriers = append_compact_history(&mut rel.container, graph_version, &mut orphan);
    rel.sync().unwrap();
    drop(rel);
    (orphan, carriers)
}

fn append_phy_history(path: &Path) -> (Vec<u8>, BTreeMap<u64, Vec<u8>>) {
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
    let carriers = append_compact_history(&mut phy.container, graph_version, &mut orphan);
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
    source_refs: &BTreeMap<u64, crate::ObjectRef>,
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
        assert_eq!(output.read(new_ref).unwrap(), *expected_payload);
        assert!(
            output_payloads
                .iter()
                .any(|payload| payload == expected_payload)
        );
    }
}

#[test]
fn rel_compact_graph_reclamation_preserves_all_cuts_and_relocates_only_published_carriers() {
    let scratch = Scratch::new();
    let source = scratch.file("source.rel");
    let output = scratch.file("reclaimed.rel");
    let (orphan, compact) = append_rel_history(&source);
    assert_reclaimed_graph(&source, &output, &orphan, &compact, true);
}

#[test]
fn phy_compact_graph_reclamation_preserves_all_cuts_and_relocates_only_published_carriers() {
    let scratch = Scratch::new();
    let source = scratch.file("source.phy");
    let output = scratch.file("reclaimed.phy");
    let (orphan, compact) = append_phy_history(&source);
    assert_reclaimed_graph(&source, &output, &orphan, &compact, false);
}

#[test]
fn phy_semantic_migration_resolves_compact_ordinals_to_source_ids_and_clocks() {
    let scratch = Scratch::new();
    let source_path = scratch.file("legacy-source.phy");
    let output_path = scratch.file("migrated.phy");
    let mut source = Phylactery::create_legacy_typed(&source_path).unwrap();
    let (first, _) = source
        .publish_memory(None, 0, memory_draft("migration-a"))
        .unwrap();
    let (second, _) = source
        .publish_memory(None, 0, memory_draft("migration-b"))
        .unwrap();
    source
        .set_memory_relation_with_origin(
            first.id,
            second.id,
            GraphRelationKind::Causal,
            true,
            GraphRelationOrigin::User,
            source.graph_version(),
        )
        .unwrap();
    let mut orphan = Vec::new();
    let graph_version = source.graph_version();
    append_compact_history(&mut source.container, graph_version, &mut orphan);
    source.sync().unwrap();
    drop(source);

    let mut source = Phylactery::open(&source_path).unwrap();
    let result = crate::migrate_file(&source_path, &output_path).unwrap();
    assert!(result.owner_id.starts_with("phy-"));
    let migrated = Phylactery::open(&output_path).unwrap();
    let migrated_history = GraphHistory::read(&migrated.container, &migrated.graph).unwrap();
    assert_eq!(source.graph_version(), migrated.graph_version());
    assert_eq!(
        source.graph.transaction_global_versions(),
        migrated.graph.transaction_global_versions()
    );
    let source_cuts = accepted_graph_cuts(&mut source.container, &source.graph);
    for (cut, (source_states, source_memory_graph_version)) in source_cuts.iter().enumerate() {
        let migrated_cut = migrated_history.at(cut as u64, false).unwrap();
        assert_eq!(
            *source_states, migrated_cut.states,
            "Graph state at cut {cut}"
        );
        assert_eq!(
            *source_memory_graph_version, migrated_cut.memory_graph_version,
            "MemoryGraph head at cut {cut}"
        );
        for relation in source_states.values() {
            assert_eq!(
                source.transaction_time_ns(relation.global_version),
                migrated.transaction_time_ns(relation.global_version),
                "transaction time at Graph cut {cut}"
            );
        }
    }
}

#[test]
fn compact_graph_cuts_survive_native_rel_principal_and_attachment_repacks() {
    use crate::{
        IncomingAttachment, IncomingTurn, ProjectFileRef, ProjectRepositoryKind,
        ProjectRepositoryRef, ProjectRevisionRef,
    };
    use sha2::{Digest, Sha256};
    let scratch = Scratch::new();
    let source = scratch.file("native-source.rel");
    append_rel_history(&source);
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
