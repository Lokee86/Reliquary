use crate::graph_codec::{GraphNodePayload, GraphVersionPayload, encode_node, encode_version};
use crate::graph_compact_codec::encode_mutation;
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::owner_lifecycle::{LifecycleTarget, TombstoneOutcome};
use crate::{
    Container, Cva, GraphRelationKind, GraphRelationOrigin, HistoryPruneRequest, MemoryDraft,
    MemoryId, Phylactery, SemanticGraphRelationKind, SemanticNodeRef,
};
use arcana::NodeId;
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("graph-compact-proof-{}", uuid::Uuid::new_v4()));
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

fn draft(mutation_id: &str, title: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: title.into(),
        content: format!("Body for {title} ").repeat(1024),
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
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn append_compact_graph_tail(container: &mut Container, graph_version: u64) {
    let carrier = encode_mutation(GraphOrdinalMutation {
        source: 0,
        target: 1,
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
        active: true,
        origin: GraphRelationOrigin::User,
    });
    let mutation = container.append(&carrier).unwrap();
    let global_version = container
        .allocate_version_with_transaction_time(Some(70_000))
        .unwrap();
    container
        .append(&encode_version(GraphVersionPayload {
            global_version,
            graph_version,
            mutation,
        }))
        .unwrap();
}

fn install_compact_graph_tail_rel(path: &Path) {
    let mut owner = Cva::open(path).unwrap();
    let graph_version = owner.graph_version() + 1;
    append_compact_graph_tail(&mut owner.container, graph_version);
    owner.sync().unwrap();
}

fn install_compact_graph_tail_phy(path: &Path) {
    let mut owner = Phylactery::open(path).unwrap();
    let graph_version = owner.graph_version() + 1;
    append_compact_graph_tail(&mut owner.container, graph_version);
    owner.sync().unwrap();
}

fn add_graph_base_rel(owner: &mut Cva, a: MemoryId, b: MemoryId) {
    owner
        .set_memory_relation(
            a,
            b,
            GraphRelationKind::Factual,
            true,
            owner.graph_version(),
        )
        .unwrap();
}

fn add_graph_base_phy(owner: &mut Phylactery, a: MemoryId, b: MemoryId) {
    owner
        .set_memory_relation(
            a,
            b,
            GraphRelationKind::Factual,
            true,
            owner.graph_version(),
        )
        .unwrap();
}

fn swap_valid_memory_mappings(path: &Path) {
    let mut container = Container::open(path).unwrap();
    let chunks = container.chunks().unwrap();
    let mappings = chunks
        .into_iter()
        .filter_map(|object| {
            let payload = container.read(object).unwrap();
            (payload.starts_with(b"CVAGNODE") && payload.len() == 44).then_some((object, payload))
        })
        .take(2)
        .collect::<Vec<_>>();
    assert_eq!(mappings.len(), 2, "fixture should have two Memory mappings");
    let (left_object, mut left) = mappings[0].clone();
    let (right_object, mut right) = mappings[1].clone();
    assert_eq!(&left[40..44], &0_u32.to_le_bytes());
    assert_eq!(&right[40..44], &1_u32.to_le_bytes());
    left[8..40].copy_from_slice(&right[8..40]);
    right[8..40].copy_from_slice(&mappings[0].1[8..40]);
    container.write_chunk_prefix(left_object, &left).unwrap();
    container.write_chunk_prefix(right_object, &right).unwrap();
    container.sync().unwrap();
}

fn prune_rel_with_compact_graph(scratch: &Scratch) -> (PathBuf, PathBuf) {
    let source = scratch.file("source.rel");
    let output = scratch.file("pruned.rel");
    let mut owner = Cva::create(&source).unwrap();
    let (a, _) = owner
        .publish_memory(None, 0, draft("a-create", "A"))
        .unwrap();
    let (b, _) = owner
        .publish_memory(None, 0, draft("b-create", "B"))
        .unwrap();
    add_graph_base_rel(&mut owner, a.id, b.id);
    owner.sync().unwrap();
    drop(owner);
    install_compact_graph_tail_rel(&source);

    let mut owner = Cva::open(&source).unwrap();
    assert!(matches!(
        owner.tombstone_memory(a.id, a.revision).unwrap(),
        TombstoneOutcome::Published(_)
    ));
    owner
        .prune_tombstoned_history(
            &output,
            HistoryPruneRequest {
                targets: vec![LifecycleTarget::Memory(a.id)],
            },
        )
        .unwrap();
    drop(owner);
    (source, output)
}

fn prune_phy_with_compact_graph(scratch: &Scratch) -> (PathBuf, PathBuf) {
    let source = scratch.file("source.phy");
    let output = scratch.file("pruned.phy");
    let mut owner = Phylactery::create(&source).unwrap();
    let (a, _) = owner
        .publish_memory(None, 0, draft("a-create", "A"))
        .unwrap();
    let (b, _) = owner
        .publish_memory(None, 0, draft("b-create", "B"))
        .unwrap();
    add_graph_base_phy(&mut owner, a.id, b.id);
    owner.sync().unwrap();
    drop(owner);
    install_compact_graph_tail_phy(&source);

    let mut owner = Phylactery::open(&source).unwrap();
    assert!(matches!(
        owner.tombstone_memory(a.id, a.revision).unwrap(),
        TombstoneOutcome::Published(_)
    ));
    owner
        .prune_tombstoned_history(
            &output,
            HistoryPruneRequest {
                targets: vec![LifecycleTarget::Memory(a.id)],
            },
        )
        .unwrap();
    drop(owner);
    (source, output)
}

#[test]
fn compact_graph_prune_proofs_reopen_and_reject_valid_mapping_substitution_for_rel_and_phy() {
    let scratch = Scratch::new();
    let (rel_source, rel_pruned) = prune_rel_with_compact_graph(&scratch);
    assert_eq!(
        Cva::open(&rel_pruned).unwrap().latest_global_version(),
        Cva::open(&rel_source).unwrap().latest_global_version()
    );
    swap_valid_memory_mappings(&rel_pruned);
    let error = Cva::open(&rel_pruned).err().unwrap().to_string();
    assert!(
        error.contains("maintenance proof"),
        "unexpected error: {error}"
    );

    let (phy_source, phy_pruned) = prune_phy_with_compact_graph(&scratch);
    assert_eq!(
        Phylactery::open(&phy_pruned)
            .unwrap()
            .latest_global_version(),
        Phylactery::open(&phy_source)
            .unwrap()
            .latest_global_version()
    );
    swap_valid_memory_mappings(&phy_pruned);
    let error = Phylactery::open(&phy_pruned).err().unwrap().to_string();
    assert!(
        error.contains("maintenance proof"),
        "unexpected error: {error}"
    );
}

#[test]
fn compact_graph_semantic_digest_ignores_ordinal_assignment_but_binds_mutation_fields() {
    let scratch = Scratch::new();
    let path = scratch.file("digest.rel");
    let mut owner = Cva::create(&path).unwrap();
    let (a, _) = owner.publish_memory(None, 0, draft("a", "A")).unwrap();
    let (b, _) = owner.publish_memory(None, 0, draft("b", "B")).unwrap();
    add_graph_base_rel(&mut owner, a.id, b.id);
    let forward = encode_mutation(GraphOrdinalMutation {
        source: 0,
        target: 1,
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
        active: true,
        origin: GraphRelationOrigin::User,
    });
    let reverse = encode_mutation(GraphOrdinalMutation {
        source: 1,
        target: 0,
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
        active: true,
        origin: GraphRelationOrigin::User,
    });
    let mut reversed = crate::graph_store::GraphStore::empty();
    reversed
        .insert_node_rebuilt(crate::graph_model::GraphNodeRecord {
            semantic_node: SemanticNodeRef::memory(b.id),
            node_id: NodeId(0),
        })
        .unwrap();
    reversed
        .insert_node_rebuilt(crate::graph_model::GraphNodeRecord {
            semantic_node: SemanticNodeRef::memory(a.id),
            node_id: NodeId(1),
        })
        .unwrap();
    assert_eq!(
        owner.graph.compact_carrier_digest(&forward).unwrap(),
        reversed.compact_carrier_digest(&reverse).unwrap(),
        "equivalent endpoint assignment under reversed ordinals should hash identically"
    );
    assert_ne!(
        owner.graph.compact_carrier_digest(&forward).unwrap(),
        owner.graph.compact_carrier_digest(&reverse).unwrap(),
        "reversing stable semantic endpoints must change the digest"
    );
    for altered in [
        encode_mutation(GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
            active: false,
            origin: GraphRelationOrigin::User,
        }),
        encode_mutation(GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
            active: true,
            origin: GraphRelationOrigin::Dream,
        }),
        encode_mutation(GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Causal),
            active: true,
            origin: GraphRelationOrigin::User,
        }),
    ] {
        assert_ne!(
            owner.graph.compact_carrier_digest(&forward).unwrap(),
            owner.graph.compact_carrier_digest(&altered).unwrap()
        );
    }
    let reordered = crate::graph_compact_codec::encode_batch(&[
        GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
            active: true,
            origin: GraphRelationOrigin::User,
        },
        GraphOrdinalMutation {
            source: 1,
            target: 0,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
            active: false,
            origin: GraphRelationOrigin::Dream,
        },
    ])
    .unwrap();
    let reordered_again = crate::graph_compact_codec::encode_batch(&[
        GraphOrdinalMutation {
            source: 1,
            target: 0,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
            active: false,
            origin: GraphRelationOrigin::Dream,
        },
        GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
            active: true,
            origin: GraphRelationOrigin::User,
        },
    ])
    .unwrap();
    assert_ne!(
        owner.graph.compact_carrier_digest(&reordered).unwrap(),
        owner
            .graph
            .compact_carrier_digest(&reordered_again)
            .unwrap(),
        "mutation order is retained as semantic publication evidence"
    );
}

#[test]
fn compact_graph_digest_is_excluded_for_orphan_and_post_watermark_markers() {
    let scratch = Scratch::new();
    let path = scratch.file("watermark.rel");
    let mut owner = Cva::create(&path).unwrap();
    let (a, _) = owner.publish_memory(None, 0, draft("a", "A")).unwrap();
    let (b, _) = owner.publish_memory(None, 0, draft("b", "B")).unwrap();
    add_graph_base_rel(&mut owner, a.id, b.id);
    let watermark = owner.latest_global_version();
    let graph = &owner.graph;
    let before = super::owner_history_digest(&mut owner.container, watermark, graph).unwrap();

    owner
        .container
        .append(&encode_node(GraphNodePayload {
            semantic_node: SemanticNodeRef::observation([0x77; 32]),
            node_id: NodeId(2),
        }))
        .unwrap();
    let after_unused_mapping =
        super::owner_history_digest(&mut owner.container, watermark, &owner.graph).unwrap();
    assert_eq!(before, after_unused_mapping);

    let orphan = encode_mutation(GraphOrdinalMutation {
        source: 0,
        target: 1,
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
        active: true,
        origin: GraphRelationOrigin::User,
    });
    owner.container.append(&orphan).unwrap();
    let after_orphan =
        super::owner_history_digest(&mut owner.container, watermark, &owner.graph).unwrap();
    assert_eq!(before, after_orphan);

    let mutation = owner.container.append(&orphan).unwrap();
    let global = owner
        .container
        .allocate_version_with_transaction_time(Some(90_000))
        .unwrap();
    let graph_version = owner.graph_version() + 1;
    owner
        .container
        .append(&encode_version(GraphVersionPayload {
            global_version: global,
            graph_version,
            mutation,
        }))
        .unwrap();
    let after_later_marker =
        super::owner_history_digest(&mut owner.container, watermark, &owner.graph).unwrap();
    assert_eq!(before, after_later_marker);
}

#[test]
fn legacy_graph_carrier_digest_remains_raw_sha256() {
    use sha2::{Digest, Sha256};
    let scratch = Scratch::new();
    let path = scratch.file("legacy-digest-owner.rel");
    let mut container = Container::create_with_identity(
        &path,
        crate::ContainerIdentity {
            file_kind: crate::FileKind::Reliquary,
            scope: None,
        },
    )
    .unwrap();
    let legacy = crate::graph_codec::encode_mutation(crate::graph_codec::GraphMutationPayload {
        source: SemanticNodeRef::observation([0x11; 32]),
        target: SemanticNodeRef::observation([0x22; 32]),
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
        active: true,
        origin: GraphRelationOrigin::User,
    });
    let mutation = container.append(&legacy).unwrap();
    let global_version = container
        .allocate_version_with_transaction_time(Some(1_234_567))
        .unwrap();
    assert_eq!(global_version, 1);
    container
        .append(&encode_version(GraphVersionPayload {
            global_version,
            graph_version: 1,
            mutation,
        }))
        .unwrap();

    // Frozen pre-compact digest calculation: initial watermark/global/time
    // prefix, normalized GraphVersion carrier link (zero ObjectRef), and the
    // length-prefixed SHA-256 of the exact raw full-ID carrier.
    let mut expected = Sha256::new();
    expected.update(1_u64.to_le_bytes()); // watermark
    expected.update(1_u64.to_le_bytes()); // global coordinate
    expected.update([1]); // transaction time is present
    expected.update(1_234_567_i64.to_le_bytes());
    let mut normalized_marker = [0_u8; 40];
    normalized_marker[..8].copy_from_slice(b"CVAGVER1");
    normalized_marker[8..16].copy_from_slice(&1_u64.to_le_bytes());
    normalized_marker[16..24].copy_from_slice(&1_u64.to_le_bytes());
    // bytes 24..40 are the normalized zero ObjectRef
    expected.update((normalized_marker.len() as u64).to_le_bytes());
    expected.update(normalized_marker);
    let raw_carrier_digest: [u8; 32] = Sha256::digest(&legacy).into();
    expected.update((raw_carrier_digest.len() as u64).to_le_bytes());
    expected.update(raw_carrier_digest);
    let expected: [u8; 32] = expected.finalize().into();
    assert_eq!(
        super::owner_history_digest(&mut container, 1, &crate::graph_store::GraphStore::empty(),)
            .unwrap(),
        expected,
        "legacy full-ID Graph proof digest must remain byte-for-byte compatible"
    );
}
