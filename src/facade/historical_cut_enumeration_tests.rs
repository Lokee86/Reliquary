use super::*;
use crate::{EntityDraft, GraphRelationKind, MemoryDraft, Phylactery};
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("historical-cuts-{}", uuid::Uuid::new_v4()));
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

fn draft(mutation: &str) -> MemoryDraft {
    MemoryDraft {
        category: "preference".into(),
        memory_type: "user".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: "Cut enumeration fixture".into(),
        content: mutation.into(),
        scope: "user".into(),
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
        source_time_ns: Some(1),
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn assert_positions(cuts: &[u64], head: u64) {
    assert_eq!(cuts, &(0..=head).collect::<Vec<_>>());
}

fn entity_draft(mutation: &str) -> EntityDraft {
    EntityDraft {
        canonical_name: "Cut enumeration entity".into(),
        aliases: Vec::new(),
        kind: "person".into(),
        summary: "Graph-only transaction fixture".into(),
        mutation_id: mutation.into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

#[test]
fn synthetic_cutsets_include_empty_prefix_and_only_qualifying_memory_graph_values() {
    let fixture = Fixture::new();
    let mut rel = Cva::create(fixture.path("owner.rel")).unwrap();
    let rel_initial = reliquary_cuts(&mut rel).unwrap();
    assert_eq!(rel_initial.archive, Some(vec![0]));
    assert_eq!(rel_initial.memory, vec![0]);
    assert_eq!(rel_initial.entity, vec![0]);
    assert_eq!(rel_initial.relationship, vec![0]);
    assert_eq!(rel_initial.graph, vec![0]);
    assert_eq!(rel_initial.memory_graph, vec![0]);
    assert_eq!(rel_initial.vector_generation, Some(vec![0]));
    assert_eq!(rel_initial.ego, vec![0]);
    assert_eq!(rel_initial.freshness_activity, Some(vec![0]));
    assert_eq!(rel_initial.freshness_events, Some(Vec::new()));
    assert_eq!(rel_initial.global, vec![0]);

    let left = rel.publish_memory(None, 0, draft("rel-left")).unwrap().0.id;
    let right = rel
        .publish_memory(None, 0, draft("rel-right"))
        .unwrap()
        .0
        .id;
    let third = rel
        .publish_memory(None, 0, draft("rel-third"))
        .unwrap()
        .0
        .id;
    let entity = rel
        .publish_entity(None, 0, entity_draft("rel-entity"))
        .unwrap()
        .0;
    rel.set_memory_relation(left, right, GraphRelationKind::Factual, true, 0)
        .unwrap();
    rel.set_entity_association(left, entity.id, true, 1)
        .unwrap();
    rel.set_memory_relation(right, third, GraphRelationKind::Causal, true, 2)
        .unwrap();
    let rel_after = reliquary_cuts(&mut rel).unwrap();
    assert_positions(&rel_after.graph, rel.graph_version());
    assert_eq!(rel_after.graph, vec![0, 1, 2, 3]);
    assert_eq!(rel_after.memory_graph, vec![0, 1, 3]);
    assert_positions(&rel_after.memory, rel.memory_version());
    assert_positions(rel_after.global.as_slice(), rel.latest_global_version());
    assert_eq!(rel_after, reliquary_cuts(&mut rel).unwrap());

    let mut phy = Phylactery::create(fixture.path("owner.phy")).unwrap();
    let phy_initial = phylactery_cuts(&phy).unwrap();
    assert_eq!(phy_initial.archive, None);
    assert_eq!(phy_initial.vector_generation, None);
    assert_eq!(phy_initial.freshness_activity, None);
    assert_eq!(phy_initial.freshness_events, None);
    assert_eq!(phy_initial.memory_graph, vec![0]);
    let left = phy.publish_memory(None, 0, draft("phy-left")).unwrap().0.id;
    let right = phy
        .publish_memory(None, 0, draft("phy-right"))
        .unwrap()
        .0
        .id;
    let third = phy
        .publish_memory(None, 0, draft("phy-third"))
        .unwrap()
        .0
        .id;
    let entity = phy
        .publish_entity(None, 0, entity_draft("phy-entity"))
        .unwrap()
        .0;
    phy.set_memory_relation(left, right, GraphRelationKind::Factual, true, 0)
        .unwrap();
    phy.set_entity_association(left, entity.id, true, 1)
        .unwrap();
    phy.set_memory_relation(right, third, GraphRelationKind::Causal, true, 2)
        .unwrap();
    let phy_after = phylactery_cuts(&phy).unwrap();
    assert_positions(&phy_after.graph, phy.graph_version());
    assert_eq!(phy_after.graph, vec![0, 1, 2, 3]);
    assert_eq!(phy_after.memory_graph, vec![0, 1, 3]);
    assert_eq!(phy_after, phylactery_cuts(&phy).unwrap());
}

#[test]
#[cfg(feature = "history-oracle-fixtures")]
fn authoritative_cutsets_are_stable_across_reopen_for_all_fixtures() {
    let root = PathBuf::from(std::env::var_os("RELIQUARY_HISTORY_FIXTURES").expect("fixture root"));
    let fixture = Fixture::new();
    for (index, corpus) in [
        "chatgpt-first14d",
        "chatgpt-first28d",
        "rhelm-david-r-ellis-pre-relationship",
    ]
    .iter()
    .enumerate()
    {
        for (kind, name) in [("rel", "project.prj.rel"), ("phy", "user.phy")] {
            let path = fixture.path(&format!("{index}.{kind}"));
            let mut source = fs::File::open(root.join(corpus).join(name)).unwrap();
            let mut destination = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            std::io::copy(&mut source, &mut destination).unwrap();
            drop(destination);

            if kind == "rel" {
                let mut rel = Cva::open(&path).unwrap();
                let cuts = reliquary_cuts(&mut rel).unwrap();
                assert_eq!(
                    cuts,
                    reliquary_cuts(&mut rel).unwrap(),
                    "{corpus}/{kind} deterministic"
                );
                if let Some(events) = &cuts.freshness_events {
                    for event_cut in events {
                        assert!(event_cut.rel_turn <= rel.rel_turn_count());
                        assert!(event_cut.graph_version <= rel.memory_graph_version());
                        assert!(
                            event_cut
                                .event_id_inclusive
                                .as_ref()
                                .is_some_and(|id| !id.is_empty())
                        );
                    }
                }
                rel.sync().unwrap();
                drop(rel);
                let mut reopened = Cva::open(&path).unwrap();
                assert_eq!(
                    cuts,
                    reliquary_cuts(&mut reopened).unwrap(),
                    "{corpus}/{kind} reopen"
                );
            } else {
                let phy = Phylactery::open(&path).unwrap();
                let cuts = phylactery_cuts(&phy).unwrap();
                assert_eq!(
                    cuts,
                    phylactery_cuts(&phy).unwrap(),
                    "{corpus}/{kind} deterministic"
                );
                drop(phy);
                let reopened = Phylactery::open(&path).unwrap();
                assert_eq!(
                    cuts,
                    phylactery_cuts(&reopened).unwrap(),
                    "{corpus}/{kind} reopen"
                );
            }
        }
    }
}
