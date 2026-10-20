use crate::graph_codec::{GraphVersionPayload, encode_version};
use crate::graph_compact_codec::{encode_batch, encode_mutation};
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::{
    Cva, GraphRelationKind, GraphRelationOrigin, MemoryDraft, MemoryId, SemanticGraphRelationKind,
};
use std::{collections::BTreeSet, fs, path::PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("compact-dream-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft(name: &str, lifecycle_state: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: name.into(),
        content: format!("compact Dream fixture {name}"),
        scope: "project".into(),
        lifecycle_state: lifecycle_state.into(),
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
        mutation_id: format!("compact-dream-{name}"),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn publish(rel: &mut Cva, name: &str, state: &str) -> MemoryId {
    rel.publish_memory(None, 0, draft(name, state))
        .unwrap()
        .0
        .id
}

fn append_compact_transaction(
    rel: &mut Cva,
    graph_version: u64,
    changes: &[(
        MemoryId,
        MemoryId,
        GraphRelationKind,
        bool,
        GraphRelationOrigin,
    )],
) {
    let values = changes
        .iter()
        .map(
            |(source, target, kind, active, origin)| GraphOrdinalMutation {
                source: rel.graph.node_id(*source).unwrap().0,
                target: rel.graph.node_id(*target).unwrap().0,
                kind: SemanticGraphRelationKind::Memory(*kind),
                active: *active,
                origin: *origin,
            },
        )
        .collect::<Vec<_>>();
    let carrier = if values.len() == 1 {
        rel.container.append(&encode_mutation(values[0])).unwrap()
    } else {
        rel.container
            .append(&encode_batch(&values).unwrap())
            .unwrap()
    };
    let global_version = rel
        .container
        .allocate_version_with_transaction_time(Some(9_000 + graph_version as i64))
        .unwrap();
    rel.container
        .append(&encode_version(GraphVersionPayload {
            global_version,
            graph_version,
            mutation: carrier,
        }))
        .unwrap();
}

#[test]
fn compact_dream_reads_accepted_graph_cut_and_filters_by_origin_and_stable_candidates() {
    let scratch = Scratch::new();
    let path = scratch.0.join("compact-dream.rel");
    let mut rel = Cva::create(&path).unwrap();
    let root = publish(&mut rel, "root", "knowledge");
    let late_candidate = publish(&mut rel, "late-candidate", "knowledge");
    let unrelated = publish(&mut rel, "unrelated", "knowledge");
    let future_neighbor = publish(&mut rel, "future-neighbor", "knowledge");
    let source = publish(&mut rel, "source", "extracted");

    // These ordinary publications create stable physical Graph ordinals and
    // leave the initial Dream pass with a genuine prior graph cut.
    rel.set_memory_relation_with_origin(
        source,
        root,
        GraphRelationKind::Factual,
        true,
        GraphRelationOrigin::User,
        rel.graph_version(),
    )
    .unwrap();
    rel.set_memory_relation_with_origin(
        source,
        late_candidate,
        GraphRelationKind::Factual,
        true,
        GraphRelationOrigin::User,
        rel.graph_version(),
    )
    .unwrap();
    rel.set_memory_relation_with_origin(
        unrelated,
        future_neighbor,
        GraphRelationKind::Topical,
        true,
        GraphRelationOrigin::User,
        rel.graph_version(),
    )
    .unwrap();

    let before = rel.graph_version();
    assert_eq!(before, 3);
    let pass_id = crate::dream_freshness::initial_pass_id(rel.owner_uuid().unwrap(), source);
    let candidates = BTreeSet::from([root, unrelated]);
    rel.begin_initial_dream_pass(pass_id.clone(), source, 1, 0, candidates.clone(), before)
        .unwrap();
    append_compact_transaction(
        &mut rel,
        before + 1,
        &[
            (
                source,
                root,
                GraphRelationKind::Factual,
                true,
                GraphRelationOrigin::Dream,
            ),
            (
                source,
                late_candidate,
                GraphRelationKind::Factual,
                true,
                GraphRelationOrigin::Dream,
            ),
            (
                source,
                root,
                GraphRelationKind::Causal,
                true,
                GraphRelationOrigin::User,
            ),
        ],
    );
    drop(rel);

    let mut rel = Cva::open(&path).unwrap();
    let pass = rel.pending_initial_dream_pass(source).unwrap();
    assert_eq!(pass.graph_version_before, before);
    rel.record_initial_dream_roots(&pass_id, BTreeSet::from([root]))
        .unwrap();
    rel.accept_initial_dream_settlement(&pass_id, 0).unwrap();
    assert_eq!(rel.graph_version(), before + 1);

    // A later compact publication after acceptance must not enter either the
    // committed-root proof or the graph snapshot pinned to the accepted cut.
    append_compact_transaction(
        &mut rel,
        before + 2,
        &[(
            source,
            unrelated,
            GraphRelationKind::Supersedes,
            true,
            GraphRelationOrigin::Dream,
        )],
    );
    drop(rel);
    let rel = Cva::open(&path).unwrap();
    let pass = rel.pending_initial_dream_pass(source).unwrap();
    let (roots, duplicate_origin) = rel.initial_dream_committed_evidence(&pass).unwrap();
    assert_eq!(roots, BTreeSet::from([root]));
    assert!(!duplicate_origin);

    let (proof, adjacency, memberships) = rel.initial_dream_graph_snapshot(&pass_id).unwrap();
    assert_eq!(proof.graph_version, before + 1);
    assert_eq!(proof.community_generation, None);
    let neighbors = adjacency.get(&source).cloned().unwrap_or_default();
    assert!(neighbors.contains(&root));
    assert!(neighbors.contains(&late_candidate));
    assert!(!neighbors.contains(&unrelated));
    assert!(!neighbors.contains(&future_neighbor));
    assert!(memberships.is_empty());
}
