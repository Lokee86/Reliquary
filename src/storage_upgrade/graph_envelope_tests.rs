//! Synthetic coverage for Graph V1 carrier to publication-envelope staging.
use super::shell::stage_graph_envelope_storage_upgrade;
use super::{StorageUpgradeRequest, plan_storage_upgrade};
use crate::container::publication_envelope::{self, PublicationClock};
use crate::graph_codec::{
    GraphMutationPayload, GraphNodePayload, GraphVersionPayload, decode_node, encode_batch,
    encode_mutation, encode_node, encode_version,
};
use crate::graph_compact_codec::publication::{self, GraphTransaction};
use crate::historical_graph_fingerprint::GraphHistory;
use crate::{
    Container, ContainerError, Cva, FileKind, GraphRelationKind, GraphRelationOrigin, MemoryDraft,
    MemoryId, Phylactery, SemanticGraphRelationKind, SemanticNodeRef,
};
use arcana::NodeId;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("graph-envelope-upgrade-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn request(&self, kind: FileKind) -> StorageUpgradeRequest {
        StorageUpgradeRequest {
            source: self.0.join(if kind == FileKind::Reliquary {
                "source.rel"
            } else {
                "source.phy"
            }),
            destination: self.0.join("unpublished-output"),
        }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft(name: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: name.into(),
        content: format!("graph envelope fixture {name}"),
        scope: "private".into(),
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
        mutation_id: format!("graph-envelope-{name}"),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn publish_legacy(
    container: &mut Container,
    memories: &mut crate::memory_store::MemoryStore,
) -> [MemoryId; 3] {
    ["a", "b", "c"].map(|name| {
        memories
            .publish_legacy_fixture(container, None, 0, draft(name))
            .unwrap()
            .0
            .id
    })
}

fn relation(
    source: MemoryId,
    target: MemoryId,
    kind: GraphRelationKind,
    active: bool,
    origin: GraphRelationOrigin,
) -> GraphMutationPayload {
    GraphMutationPayload {
        source: SemanticNodeRef::memory(source),
        target: SemanticNodeRef::memory(target),
        kind: SemanticGraphRelationKind::Memory(kind),
        active,
        origin,
    }
}

fn append_mapping(container: &mut Container, id: MemoryId, ordinal: u32) {
    container
        .append(&encode_node(GraphNodePayload {
            semantic_node: SemanticNodeRef::memory(id),
            node_id: NodeId(ordinal),
        }))
        .unwrap();
}

fn append_marker(
    container: &mut Container,
    carrier: crate::ObjectRef,
    graph_version: u64,
    time: i64,
) {
    let global_version = container
        .allocate_version_with_transaction_time(Some(time))
        .unwrap();
    container
        .append(&encode_version(GraphVersionPayload {
            global_version,
            graph_version,
            mutation: carrier,
        }))
        .unwrap();
}

fn append_publication(
    container: &mut Container,
    payload: &[u8],
    graph_version: u64,
    time: i64,
) -> crate::ObjectRef {
    let carrier = container.append(payload).unwrap();
    append_marker(container, carrier, graph_version, time);
    carrier
}

fn append_valid_history(
    container: &mut Container,
    ids: [MemoryId; 3],
    gap_global: bool,
) -> Vec<Vec<u8>> {
    let single = encode_mutation(relation(
        ids[0],
        ids[1],
        GraphRelationKind::Factual,
        true,
        GraphRelationOrigin::Dream,
    ));
    let first_carrier = container.append(&single).unwrap();
    // The first accepted carrier precedes its mappings, as permitted by V1.
    append_mapping(container, ids[0], 0);
    append_mapping(container, ids[1], 1);
    append_mapping(container, ids[2], 2);
    append_marker(container, first_carrier, 1, 900);
    let repeated_postimage = encode_batch(&[relation(
        ids[0],
        ids[1],
        GraphRelationKind::Factual,
        true,
        GraphRelationOrigin::Dream,
    )]);
    if gap_global {
        container
            .allocate_version_with_transaction_time(Some(-20))
            .unwrap();
    }
    append_publication(container, &repeated_postimage, 2, 100);
    let batch = encode_batch(&[
        relation(
            ids[0],
            ids[2],
            GraphRelationKind::Topical,
            true,
            GraphRelationOrigin::Dream,
        ),
        relation(
            ids[1],
            ids[0],
            GraphRelationKind::Causal,
            true,
            GraphRelationOrigin::User,
        ),
    ]);
    let batch_carrier = container.append(&batch).unwrap();
    let global_version = container
        .allocate_version_with_transaction_time(Some(40))
        .unwrap();
    container
        .append(&encode_version(GraphVersionPayload {
            global_version,
            graph_version: 3,
            mutation: batch_carrier,
        }))
        .unwrap();
    let mixed = encode_mutation(relation(
        ids[0],
        ids[1],
        GraphRelationKind::Factual,
        false,
        GraphRelationOrigin::User,
    ));
    append_publication(container, &mixed, 4, -100);
    let orphan = encode_mutation(relation(
        MemoryId([0xEE; 32]),
        MemoryId([0xEF; 32]),
        GraphRelationKind::References,
        true,
        GraphRelationOrigin::Dream,
    ));
    container.append(&orphan).unwrap();
    vec![single, repeated_postimage, batch, mixed, orphan]
}

fn downgrade_header(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

fn make_source(path: &Path, kind: FileKind, gap_global: bool) -> ([MemoryId; 3], Vec<Vec<u8>>) {
    let (ids, payloads) = match kind {
        FileKind::Reliquary => {
            let mut owner = Cva::create(path).unwrap();
            let ids = publish_legacy(&mut owner.container, &mut owner.memories);
            let payloads = append_valid_history(&mut owner.container, ids, gap_global);
            owner.sync().unwrap();
            (ids, payloads)
        }
        FileKind::Phylactery => {
            let mut owner = Phylactery::create(path).unwrap();
            let ids = publish_legacy(&mut owner.container, &mut owner.memories);
            let payloads = append_valid_history(&mut owner.container, ids, gap_global);
            owner.sync().unwrap();
            (ids, payloads)
        }
    };
    downgrade_header(path);
    (ids, payloads)
}

fn mapping_payloads(path: &Path, v1: bool) -> Vec<Vec<u8>> {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    let mut result = Vec::new();
    container
        .visit_payloads::<ContainerError>(|_, bytes| {
            if decode_node(bytes).unwrap().is_some() {
                result.push(bytes.to_vec());
            }
            Ok(())
        })
        .unwrap();
    result
}

fn time_signature(path: &Path, v1: bool) -> Vec<(u64, Option<i64>)> {
    let container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open(path).unwrap()
    };
    (1..=container.latest_version())
        .map(|global| (global, container.transaction_time_ns(global)))
        .collect()
}

fn source_history(
    path: &Path,
    kind: FileKind,
) -> (
    Vec<crate::SemanticGraphRelation>,
    Vec<crate::SemanticNodeRef>,
    Vec<(
        crate::historical_fingerprint::HistoricalFingerprint,
        crate::historical_fingerprint::HistoricalFingerprint,
    )>,
) {
    match kind {
        FileKind::Reliquary => {
            let owner = Cva::open_v1_read_only(path).unwrap();
            let history = GraphHistory::read(&owner.container, &owner.graph).unwrap();
            let cuts: Vec<_> = (0..=owner.graph_version())
                .map(|cut| {
                    (
                        history.at(cut, false).unwrap().fingerprint,
                        history.at(cut, true).unwrap().fingerprint,
                    )
                })
                .collect();
            (
                owner.semantic_graph_relations(),
                owner.graph.semantic_nodes().into_iter().collect::<Vec<_>>(),
                cuts,
            )
        }
        FileKind::Phylactery => {
            let owner = Phylactery::open_v1_read_only(path).unwrap();
            let history = GraphHistory::read(&owner.container, &owner.graph).unwrap();
            let cuts: Vec<_> = (0..=owner.graph_version())
                .map(|cut| {
                    (
                        history.at(cut, false).unwrap().fingerprint,
                        history.at(cut, true).unwrap().fingerprint,
                    )
                })
                .collect();
            (
                owner.semantic_graph_relations(),
                owner.graph.semantic_nodes().into_iter().collect::<Vec<_>>(),
                cuts,
            )
        }
    }
}

fn verify_owner(path: &Path, kind: FileKind, expected_ids: [MemoryId; 3], inert: &[u8]) {
    match kind {
        FileKind::Reliquary => {
            let owner = Cva::open(path).unwrap();
            assert_eq!(owner.graph_version(), 4);
            verify_graph(
                &owner.container,
                &owner.graph,
                owner.semantic_graph_relations(),
                expected_ids,
                inert,
            );
        }
        FileKind::Phylactery => {
            let owner = Phylactery::open(path).unwrap();
            assert_eq!(owner.graph_version(), 4);
            verify_graph(
                &owner.container,
                &owner.graph,
                owner.semantic_graph_relations(),
                expected_ids,
                inert,
            );
        }
    }
}

fn verify_graph(
    container: &Container,
    graph: &crate::graph_store::GraphStore,
    relations: Vec<crate::SemanticGraphRelation>,
    ids: [MemoryId; 3],
    inert: &[u8],
) {
    assert_eq!(relations.len(), 2); // current visibility excludes the inactive postimage
    let payloads = {
        let mut c = Container::open_read_only_for_scan(container.path()).unwrap();
        let mut p = Vec::new();
        c.visit_payloads::<ContainerError>(|_, b| {
            p.push(b.to_vec());
            Ok(())
        })
        .unwrap();
        p
    };
    assert!(payloads.iter().any(|p| p == inert));
    let mut native = Vec::new();
    for payload in &payloads {
        if let Some(envelope) = publication_envelope::decode(payload).unwrap() {
            assert_eq!(envelope.clock, PublicationClock::BindReserved);
            if let Some(pubn) = publication::decode(envelope.body, envelope.global_version).unwrap()
            {
                native.push(pubn);
            }
        }
    }
    assert_eq!(native.len(), 4);
    assert!(matches!(native[0].transaction, GraphTransaction::Single(_)));
    assert!(matches!(&native[1].transaction, GraphTransaction::Batch(rows) if rows.len() == 1));
    let GraphTransaction::Batch(batch) = &native[2].transaction else {
        panic!("legacy batch must remain a native batch")
    };
    assert_eq!(batch.len(), 2);
    assert_eq!(
        batch[0].kind,
        SemanticGraphRelationKind::Memory(GraphRelationKind::Topical)
    );
    assert_eq!(
        batch[1].kind,
        SemanticGraphRelationKind::Memory(GraphRelationKind::Causal)
    );
    assert_eq!(batch[0].origin, GraphRelationOrigin::Dream);
    assert_eq!(batch[1].origin, GraphRelationOrigin::User);
    let GraphTransaction::Single(last) = native[3].transaction else {
        panic!("last transaction should be single")
    };
    assert!(!last.active);
    assert_eq!(last.origin, GraphRelationOrigin::User);
    assert_eq!(
        native[0].mutations()[0].source,
        graph.node_id(ids[0]).unwrap().0
    );
    assert_eq!(
        native[0].mutations()[0].target,
        graph.node_id(ids[1]).unwrap().0
    );
}

#[test]
fn graph_envelope_upgrade_preserves_rel_phy_cuts_clocks_order_mapping_and_shape() {
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
        let scratch = Scratch::new();
        let request = scratch.request(kind);
        let (ids, carriers) = make_source(&request.source, kind, true);
        let bytes = fs::read(&request.source).unwrap();
        let digest = Sha256::digest(&bytes);
        let modified = fs::metadata(&request.source).unwrap().modified().unwrap();
        let raw_mappings = mapping_payloads(&request.source, true);
        let times = time_signature(&request.source, true);
        let before = source_history(&request.source, kind);
        let plan = plan_storage_upgrade(request.clone()).unwrap();
        let staged = stage_graph_envelope_storage_upgrade(&plan).unwrap();
        let mut after = Container::open(staged.test_path()).unwrap();
        let (relations, nodes, cuts) = match kind {
            FileKind::Reliquary => {
                let owner = Cva::open(staged.test_path()).unwrap();
                let history = GraphHistory::read(&owner.container, &owner.graph).unwrap();
                let cuts: Vec<_> = (0..=owner.graph_version())
                    .map(|cut| {
                        (
                            history.at(cut, false).unwrap().fingerprint,
                            history.at(cut, true).unwrap().fingerprint,
                        )
                    })
                    .collect();
                (
                    owner.semantic_graph_relations(),
                    owner.graph.semantic_nodes().into_iter().collect::<Vec<_>>(),
                    cuts,
                )
            }
            FileKind::Phylactery => {
                let owner = Phylactery::open(staged.test_path()).unwrap();
                let history = GraphHistory::read(&owner.container, &owner.graph).unwrap();
                let cuts: Vec<_> = (0..=owner.graph_version())
                    .map(|cut| {
                        (
                            history.at(cut, false).unwrap().fingerprint,
                            history.at(cut, true).unwrap().fingerprint,
                        )
                    })
                    .collect();
                (
                    owner.semantic_graph_relations(),
                    owner.graph.semantic_nodes().into_iter().collect::<Vec<_>>(),
                    cuts,
                )
            }
        };
        assert_eq!(relations, before.0);
        assert_eq!(nodes, before.1);
        assert_eq!(cuts, before.2);
        assert_eq!(mapping_payloads(staged.test_path(), false), raw_mappings);
        assert_eq!(time_signature(staged.test_path(), false), times);
        verify_owner(staged.test_path(), kind, ids, &carriers[4]);
        assert_eq!(fs::read(&request.source).unwrap(), bytes);
        assert_eq!(Sha256::digest(fs::read(&request.source).unwrap()), digest);
        assert_eq!(
            fs::metadata(&request.source).unwrap().modified().unwrap(),
            modified
        );
        assert!(!request.destination.exists());
        drop(after);
        drop(staged);
        assert!(fs::read_dir(&scratch.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".storage-upgrade-")
        }));
    }
}

fn invalid_source(path: &Path, kind: FileKind, mode: &str) {
    match kind {
        FileKind::Reliquary => {
            let mut owner = Cva::create(path).unwrap();
            let ids = publish_legacy(&mut owner.container, &mut owner.memories);
            invalid_history(&mut owner.container, ids, mode);
            owner.sync().unwrap();
        }
        FileKind::Phylactery => {
            let mut owner = Phylactery::create(path).unwrap();
            let ids = publish_legacy(&mut owner.container, &mut owner.memories);
            invalid_history(&mut owner.container, ids, mode);
            owner.sync().unwrap();
        }
    }
    downgrade_header(path);
}

fn invalid_history(container: &mut Container, ids: [MemoryId; 3], mode: &str) -> Vec<u8> {
    append_mapping(container, ids[0], 0);
    append_mapping(container, ids[1], 1);
    append_mapping(container, ids[2], 2);
    let bytes = encode_mutation(relation(
        ids[0],
        ids[1],
        GraphRelationKind::Factual,
        true,
        GraphRelationOrigin::User,
    ));
    let first = container.append(&bytes).unwrap();
    append_marker(container, first, 1, 1);
    match mode {
        "repeat" => {
            append_marker(container, first, 2, 2);
        }
        "gap" => {
            let next = container.append(&bytes).unwrap();
            append_marker(container, next, 3, 3);
        }
        "mapping" => {
            let other = encode_mutation(relation(
                ids[0],
                MemoryId([0xFF; 32]),
                GraphRelationKind::Causal,
                true,
                GraphRelationOrigin::User,
            ));
            let next = container.append(&other).unwrap();
            append_marker(container, next, 2, 2);
        }
        _ => unreachable!(),
    }
    bytes
}

#[test]
fn invalid_graph_mapping_repeat_and_gap_leave_source_and_stage_clean() {
    for (kind, mode) in [
        (FileKind::Reliquary, "mapping"),
        (FileKind::Phylactery, "repeat"),
        (FileKind::Reliquary, "gap"),
    ] {
        let scratch = Scratch::new();
        let request = scratch.request(kind);
        invalid_source(&request.source, kind, mode);
        let bytes = fs::read(&request.source).unwrap();
        let digest = Sha256::digest(&bytes);
        let modified = fs::metadata(&request.source).unwrap().modified().unwrap();
        let plan = plan_storage_upgrade(request.clone()).unwrap();
        assert!(
            stage_graph_envelope_storage_upgrade(&plan).is_err(),
            "{kind:?} {mode}"
        );
        assert_eq!(fs::read(&request.source).unwrap(), bytes);
        assert_eq!(Sha256::digest(fs::read(&request.source).unwrap()), digest);
        assert_eq!(
            fs::metadata(&request.source).unwrap().modified().unwrap(),
            modified
        );
        assert!(!request.destination.exists());
        assert!(fs::read_dir(&scratch.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".storage-upgrade-")
        }));
    }
}
