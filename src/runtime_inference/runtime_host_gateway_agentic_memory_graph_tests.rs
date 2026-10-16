use super::*;
use crate::{GraphRelationKind, MemoryId};

fn fixture() -> (GatewayRuntime, Arc<InstanceRuntime>, Vec<MemoryRef>) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-graph-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut rel = crate::Cva::create_project(root.join("owner.rel")).unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    let ids: Vec<MemoryId> = (0..4)
        .map(|n| {
            crate::community_test_support::rel_memory(&mut rel, &episode, &format!("graph-{n}"))
        })
        .collect();
    for (a, b, kind) in [
        (0, 1, GraphRelationKind::Topical),
        (1, 0, GraphRelationKind::Causal),
        (0, 1, GraphRelationKind::References),
        (1, 2, GraphRelationKind::Factual),
    ] {
        rel.set_memory_relation(ids[a], ids[b], kind, true, rel.graph_version())
            .unwrap();
    }
    let gateway = GatewayRuntime::new(
        root.join("state"),
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    );
    let owner = gateway.mount_rel(InteractionRuntime::new(rel)).unwrap();
    let instance = gateway
        .attach_instance(
            "graph-reader".into(),
            None,
            BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    gateway.select_rel(&instance, &owner).unwrap();
    (
        gateway,
        instance,
        ids.into_iter()
            .map(|memory_id| MemoryRef {
                owner_id: owner.clone(),
                memory_id,
            })
            .collect(),
    )
}

#[test]
fn agentic_graph_evidence_is_complete_bidirectional_one_hop_and_owner_qualified() {
    let (gateway, instance, ids) = fixture();
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    let evidence = gateway.agentic_anchor_evidence(&envelope, &ids[0]).unwrap();
    assert_eq!(evidence.len(), 3);
    assert!(evidence.iter().all(|edge| edge.neighbor == ids[1]));
    assert_eq!(
        evidence
            .iter()
            .filter(|edge| edge.direction == GraphDirection::Incoming)
            .count(),
        1
    );
    assert_eq!(
        evidence
            .iter()
            .filter(|edge| edge.direction == GraphDirection::Outgoing)
            .count(),
        2
    );
    let mut invalid = ids[0].clone();
    invalid.owner_id = "ungranted-owner".into();
    assert!(
        gateway
            .agentic_anchor_evidence(&envelope, &invalid)
            .is_err()
    );
    invalid = ids[0].clone();
    invalid.memory_id = MemoryId([255; 32]);
    assert!(
        gateway
            .agentic_anchor_evidence(&envelope, &invalid)
            .is_err()
    );
}
