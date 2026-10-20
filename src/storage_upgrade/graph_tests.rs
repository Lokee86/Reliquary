use super::shell::stage_graph_storage_upgrade;
use super::{StorageUpgradeRequest, plan_storage_upgrade};
use crate::graph_codec::{
    GraphMutationPayload, GraphNodePayload, GraphVersionPayload, encode_batch, encode_mutation,
    encode_node, encode_version,
};
use crate::historical_graph_fingerprint::GraphHistory;
use crate::{
    Container, ContainerError, Cva, FileKind, GraphRelationKind, GraphRelationOrigin, MemoryDraft,
    MemoryId, Phylactery, SemanticGraphRelationKind, SemanticNodeRef,
};
use arcana::NodeId;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("graph-upgrade-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn downgrade_header_to_v1(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

fn draft(name: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: name.into(),
        content: format!("legacy body {name}"),
        scope: "project".into(),
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
        mutation_id: format!("legacy-{name}"),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn publish_legacy(
    container: &mut Container,
    memories: &mut crate::memory_store::MemoryStore,
) -> [MemoryId; 3] {
    let a = memories
        .publish_legacy_fixture(container, None, 0, draft("a"))
        .unwrap()
        .0
        .id;
    let b = memories
        .publish_legacy_fixture(container, None, 0, draft("b"))
        .unwrap()
        .0
        .id;
    let c = memories
        .publish_legacy_fixture(container, None, 0, draft("c"))
        .unwrap()
        .0
        .id;
    [a, b, c]
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

fn append_mapping(container: &mut Container, id: MemoryId, node_id: u32) {
    container
        .append(&encode_node(GraphNodePayload {
            semantic_node: SemanticNodeRef::memory(id),
            node_id: NodeId(node_id),
        }))
        .unwrap();
}

fn append_ticketed_marker(
    container: &mut Container,
    mutation: crate::ObjectRef,
    graph_version: u64,
) {
    let global_version = container
        .allocate_version_with_transaction_time(Some(40_000 + graph_version as i64))
        .unwrap();
    container
        .append(&encode_version(GraphVersionPayload {
            global_version,
            graph_version,
            mutation,
        }))
        .unwrap();
}

fn append_legacy_graph_history(container: &mut Container, ids: [MemoryId; 3]) -> Vec<Vec<u8>> {
    let mut carriers = Vec::new();
    // The first accepted V1 carrier deliberately precedes its dense mappings.
    let first = encode_mutation(relation(
        ids[0],
        ids[1],
        GraphRelationKind::Factual,
        true,
        GraphRelationOrigin::Dream,
    ));
    assert!(first.starts_with(b"CVAGMUT1"));
    let carrier = container.append(&first).unwrap();
    append_mapping(container, ids[0], 0);
    append_mapping(container, ids[1], 1);
    append_mapping(container, ids[2], 2);
    append_ticketed_marker(container, carrier, 1);
    carriers.push(first);

    // Raw identical postimages remain accepted publications and advance both clocks.
    let repeated = encode_mutation(relation(
        ids[0],
        ids[1],
        GraphRelationKind::Factual,
        true,
        GraphRelationOrigin::Dream,
    ));
    let carrier = container.append(&repeated).unwrap();
    append_ticketed_marker(container, carrier, 2);
    carriers.push(repeated);

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
    assert!(batch.starts_with(b"CVAGBAT1"));
    let carrier = container.append(&batch).unwrap();
    append_ticketed_marker(container, carrier, 3);
    carriers.push(batch);

    let deletion = encode_mutation(relation(
        ids[0],
        ids[1],
        GraphRelationKind::Factual,
        false,
        GraphRelationOrigin::User,
    ));
    let carrier = container.append(&deletion).unwrap();
    append_ticketed_marker(container, carrier, 4);
    carriers.push(deletion);

    // No version marker: the unresolved carrier is retained but inert.
    let orphan = encode_mutation(relation(
        MemoryId([0xEE; 32]),
        MemoryId([0xEF; 32]),
        GraphRelationKind::References,
        true,
        GraphRelationOrigin::Dream,
    ));
    let orphan_ref = container.append(&orphan).unwrap();
    let _ = orphan_ref;
    carriers.push(orphan);
    carriers
}

fn make_valid_v1(path: &Path, kind: FileKind) -> ([MemoryId; 3], Vec<Vec<u8>>) {
    let owner_memories = match kind {
        FileKind::Reliquary => {
            let mut owner = Cva::create(path).unwrap();
            let ids = publish_legacy(&mut owner.container, &mut owner.memories);
            let carriers = append_legacy_graph_history(&mut owner.container, ids);
            owner.sync().unwrap();
            (ids, carriers)
        }
        FileKind::Phylactery => {
            let mut owner = Phylactery::create(path).unwrap();
            let ids = publish_legacy(&mut owner.container, &mut owner.memories);
            let carriers = append_legacy_graph_history(&mut owner.container, ids);
            owner.sync().unwrap();
            (ids, carriers)
        }
    };
    downgrade_header_to_v1(path);
    owner_memories
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Publication {
    global: u64,
    graph: u64,
    time: Option<i64>,
    batch: bool,
    mutations: usize,
}

fn publications(path: &Path, v1: bool) -> Vec<Publication> {
    let mut container = if v1 {
        Container::open_v1_read_only(path).unwrap()
    } else {
        Container::open_compact(path).unwrap()
    };
    let times = (1..=container.latest_version())
        .map(|v| (v, container.transaction_time_ns(v)))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut objects = Container::open_read_only_for_scan(path).unwrap();
    let mut result = Vec::new();
    container
        .visit_payloads::<crate::CvaError>(|_, payload| {
            if let Some(version) = crate::graph_codec::decode_version(payload)? {
                let carrier = objects.read(version.mutation)?;
                let is_batch = if v1 {
                    carrier.starts_with(b"CVAGBAT1")
                } else {
                    carrier.starts_with(b"CVAGCB01")
                };
                let count = if v1 {
                    if is_batch {
                        crate::graph_codec::decode_batch(&carrier)?.map_or(0, |v| v.len())
                    } else {
                        usize::from(crate::graph_codec::decode_mutation(&carrier)?.is_some())
                    }
                } else {
                    crate::graph_compact_codec::decode(&carrier)?.map_or(0, |v| v.len())
                };
                let (batch, count) = (is_batch, count);
                result.push(Publication {
                    global: version.global_version,
                    graph: version.graph_version,
                    time: times[&version.global_version],
                    batch,
                    mutations: count,
                });
            }
            Ok(())
        })
        .unwrap();
    result
}

fn cuts_rel(
    path: &Path,
    v1: bool,
) -> Vec<(
    crate::historical_fingerprint::HistoricalFingerprint,
    crate::historical_fingerprint::HistoricalFingerprint,
)> {
    let owner = if v1 {
        Cva::open_v1_read_only(path).unwrap()
    } else {
        Cva::open(path).unwrap()
    };
    let history = GraphHistory::read(&owner.container, &owner.graph).unwrap();
    (0..=owner.graph_version())
        .map(|cut| {
            (
                history.at(cut, false).unwrap().fingerprint,
                history.at(cut, true).unwrap().fingerprint,
            )
        })
        .collect()
}

fn cuts_phy(
    path: &Path,
    v1: bool,
) -> Vec<(
    crate::historical_fingerprint::HistoricalFingerprint,
    crate::historical_fingerprint::HistoricalFingerprint,
)> {
    let owner = if v1 {
        Phylactery::open_v1_read_only(path).unwrap()
    } else {
        Phylactery::open(path).unwrap()
    };
    let history = GraphHistory::read(&owner.container, &owner.graph).unwrap();
    (0..=owner.graph_version())
        .map(|cut| {
            (
                history.at(cut, false).unwrap().fingerprint,
                history.at(cut, true).unwrap().fingerprint,
            )
        })
        .collect()
}

fn clocks_rel(path: &Path, v1: bool) -> ([u8; 16], u64, Vec<Option<i64>>) {
    let owner = if v1 {
        Cva::open_v1_read_only(path).unwrap()
    } else {
        Cva::open(path).unwrap()
    };
    (
        owner.owner_uuid().unwrap(),
        owner.latest_global_version(),
        (1..=owner.latest_global_version())
            .map(|v| owner.transaction_time_ns(v))
            .collect(),
    )
}

fn clocks_phy(path: &Path, v1: bool) -> ([u8; 16], u64, Vec<Option<i64>>) {
    let owner = if v1 {
        Phylactery::open_v1_read_only(path).unwrap()
    } else {
        Phylactery::open(path).unwrap()
    };
    (
        owner.owner_uuid().unwrap(),
        owner.latest_global_version(),
        (1..=owner.latest_global_version())
            .map(|v| owner.transaction_time_ns(v))
            .collect(),
    )
}

#[test]
fn graph_upgrade_preserves_legacy_rel_and_phy_publication_cuts_and_marker_facts() {
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
        let fixture = Fixture::new();
        let source = fixture.0.join(if kind == FileKind::Reliquary {
            "source.rel"
        } else {
            "source.phy"
        });
        let destination = fixture.0.join("staged-not-published");
        let (_ids, carriers) = make_valid_v1(&source, kind);
        let source_bytes = fs::read(&source).unwrap();
        let source_hash = Sha256::digest(&source_bytes);
        let source_modified = fs::metadata(&source).unwrap().modified().unwrap();
        let before_publications = publications(&source, true);
        let before_cuts = if kind == FileKind::Reliquary {
            cuts_rel(&source, true)
        } else {
            cuts_phy(&source, true)
        };
        let before_clocks = if kind == FileKind::Reliquary {
            clocks_rel(&source, true)
        } else {
            clocks_phy(&source, true)
        };
        assert_eq!(before_publications.len(), 4);
        assert_eq!(
            before_publications
                .iter()
                .map(|p| (p.graph, p.mutations))
                .collect::<Vec<_>>(),
            vec![(1, 1), (2, 1), (3, 2), (4, 1)]
        );

        let plan = plan_storage_upgrade(StorageUpgradeRequest {
            source: source.clone(),
            destination: destination.clone(),
        })
        .unwrap();
        let staged = stage_graph_storage_upgrade(&plan).unwrap();
        let staged_path = staged.test_path();
        let after_publications = publications(staged_path, false);
        let after_cuts = if kind == FileKind::Reliquary {
            cuts_rel(staged_path, false)
        } else {
            cuts_phy(staged_path, false)
        };
        let after_clocks = if kind == FileKind::Reliquary {
            clocks_rel(staged_path, false)
        } else {
            clocks_phy(staged_path, false)
        };
        assert_eq!(after_publications, before_publications);
        assert_eq!(
            after_cuts, before_cuts,
            "all retained Graph and MemoryGraph cuts"
        );
        assert_eq!(after_clocks, before_clocks);
        assert!(
            after_publications
                .iter()
                .all(|publication| publication.graph > 0)
        );

        let mut staged_container = Container::open_compact(staged_path).unwrap();
        let mut payloads = Vec::new();
        staged_container
            .visit_payloads::<ContainerError>(|_, payload| {
                payloads.push(payload.to_vec());
                Ok(())
            })
            .unwrap();
        for raw in &carriers[..4] {
            assert!(
                !payloads.iter().any(|payload| payload == raw),
                "marked V1 legacy carrier survived"
            );
        }
        assert!(
            payloads.iter().any(|payload| payload == &carriers[4]),
            "unmarked orphan carrier must remain inert and preserved"
        );
        assert!(
            payloads
                .iter()
                .any(|payload| payload.starts_with(b"CVAGCM01"))
        );
        assert!(
            payloads
                .iter()
                .any(|payload| payload.starts_with(b"CVAGCB01"))
        );
        assert_eq!(fs::read(&source).unwrap(), source_bytes);
        assert_eq!(Sha256::digest(fs::read(&source).unwrap()), source_hash);
        assert_eq!(
            fs::metadata(&source).unwrap().modified().unwrap(),
            source_modified
        );
        assert!(!destination.exists());
        drop(staged);
        assert!(!destination.exists());
    }
}

fn invalid_v1(path: &Path, kind: FileKind, case: &str) {
    let append = |container: &mut Container, ids: [MemoryId; 3]| {
        match case {
            "missing-mapping" => append_mapping(container, ids[0], 0),
            _ => {
                append_mapping(container, ids[0], 0);
                append_mapping(container, ids[1], 1);
                append_mapping(container, ids[2], 2);
            }
        }
        let mutation = if case == "unknown-compact" {
            let bytes = crate::graph_compact_codec::encode_mutation(
                crate::graph_compact_contract::GraphOrdinalMutation {
                    source: 0,
                    target: 1,
                    kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
                    active: true,
                    origin: GraphRelationOrigin::User,
                },
            );
            bytes
        } else if case == "duplicate-keys" {
            encode_batch(&[
                relation(
                    ids[0],
                    ids[1],
                    GraphRelationKind::Factual,
                    true,
                    GraphRelationOrigin::User,
                ),
                relation(
                    ids[0],
                    ids[1],
                    GraphRelationKind::Factual,
                    false,
                    GraphRelationOrigin::Dream,
                ),
            ])
        } else {
            encode_mutation(relation(
                ids[0],
                ids[1],
                GraphRelationKind::Factual,
                true,
                GraphRelationOrigin::User,
            ))
        };
        let carrier = container.append(&mutation).unwrap();
        let marker_ref = if case == "bad-link" {
            crate::ObjectRef::from_legacy_bytes([0; 16])
        } else {
            carrier
        };
        append_ticketed_marker(container, marker_ref, 1);
    };
    match kind {
        FileKind::Reliquary => {
            let mut owner = Cva::create(path).unwrap();
            let ids = publish_legacy(&mut owner.container, &mut owner.memories);
            append(&mut owner.container, ids);
            owner.sync().unwrap();
        }
        FileKind::Phylactery => {
            let mut owner = Phylactery::create(path).unwrap();
            let ids = publish_legacy(&mut owner.container, &mut owner.memories);
            append(&mut owner.container, ids);
            owner.sync().unwrap();
        }
    }
    downgrade_header_to_v1(path);
}

#[test]
fn invalid_published_graph_histories_fail_without_source_or_stage_debris() {
    for (kind, case) in [
        (FileKind::Reliquary, "bad-link"),
        (FileKind::Reliquary, "missing-mapping"),
        (FileKind::Phylactery, "duplicate-keys"),
        (FileKind::Phylactery, "unknown-compact"),
    ] {
        let fixture = Fixture::new();
        let source = fixture.0.join("invalid-source");
        let destination = fixture.0.join("never-publish");
        invalid_v1(&source, kind, case);
        let bytes = fs::read(&source).unwrap();
        let digest = Sha256::digest(&bytes);
        let modified = fs::metadata(&source).unwrap().modified().unwrap();
        let plan = plan_storage_upgrade(StorageUpgradeRequest {
            source: source.clone(),
            destination: destination.clone(),
        })
        .unwrap();
        assert!(
            stage_graph_storage_upgrade(&plan).is_err(),
            "{kind:?} {case}"
        );
        assert_eq!(fs::read(&source).unwrap(), bytes);
        assert_eq!(Sha256::digest(fs::read(&source).unwrap()), digest);
        assert_eq!(fs::metadata(&source).unwrap().modified().unwrap(), modified);
        assert!(!destination.exists());
        assert!(fs::read_dir(&fixture.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".storage-upgrade-")
        }));
    }
}
