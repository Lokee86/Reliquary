//! Test-only Phylactery composition at a global ordering point.
//!
//! Clock-neutral owner state uses the accepted physical prefix selected by
//! the shared global frontier; global order is not treated as ancestry.

use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::historical_global_cuts::{GlobalOwnerCuts, phylactery_global_cuts};
use crate::{MemoryId, Phylactery, SemanticGraphRelation, SemanticNodeKind, SemanticNodeRef};
use serde_json::json;
use std::{collections::BTreeMap, fs, io::Read, path::PathBuf};

pub(crate) struct WholePhyCut {
    pub(crate) fingerprint: HistoricalFingerprint,
    pub(crate) cuts: GlobalOwnerCuts,
    pub(crate) ego_version: u64,
    pub(crate) current_memories: BTreeMap<MemoryId, u64>,
    pub(crate) visible_graph: Vec<SemanticGraphRelation>,
}

struct Prefix(PathBuf);

impl Drop for Prefix {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn replay_prefix(phy: &Phylactery, end: u64) -> Result<(Prefix, Phylactery), String> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("global-phy-oracle-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let guard = Prefix(directory);
    let path = guard.0.join("prefix.phy");
    let source = fs::File::open(phy.container.path()).map_err(|e| e.to_string())?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let copied = std::io::copy(&mut source.take(end), &mut output).map_err(|e| e.to_string())?;
    if copied != end {
        return Err("source changed during global Phylactery prefix copy".into());
    }
    drop(output);
    let prefix = Phylactery::open(&path).map_err(|e| e.to_string())?;
    Ok((guard, prefix))
}

pub(crate) fn whole_owner_at_phy(phy: &mut Phylactery, global: u64) -> Result<WholePhyCut, String> {
    let cuts = phylactery_global_cuts(phy, global)?;
    let end = crate::history::frontier_end(
        &crate::owner_lifecycle::HistorySource::new(&phy.container).unwrap(),
        global,
    )?;
    let (_guard, prefix) = replay_prefix(phy, end)?;
    let owner = phy
        .owner_uuid()
        .ok_or("whole-Phylactery oracle needs owner UUID")?;
    let mut projection = OwnerProjection::new(owner, "whole_owner_phy", global, None)?;

    let memory = crate::historical_memory_fingerprint::MemoryHistory::read(
        &mut phy.container,
        &phy.memories,
    )?
    .at(cuts.memory)?;
    let entity = crate::historical_entity_fingerprint::entity_cut(
        &phy.container,
        &phy.entities,
        cuts.entity,
    )?;
    let relationship = crate::historical_relationship_fingerprint::relationship_cut(
        &phy.container,
        &phy.relationships,
        cuts.relationship,
    )?;
    let mut graph_history =
        crate::historical_graph_fingerprint::GraphHistory::read(&phy.container, &phy.graph)?;
    graph_history.use_baseline_from(&prefix.graph);
    let graph = graph_history.at(cuts.graph, false)?;
    let memory_graph = graph_history.at(cuts.memory_graph, true)?;
    let ego_version = prefix.ego_version();
    let ego = crate::historical_ego_fingerprint::EgoHistory::read(&prefix.container, &prefix.ego)?
        .at(ego_version)?;
    let pairs = crate::historical_dream_pair_fingerprint::phylactery_dream_pairs(&prefix)?;
    let processing =
        crate::historical_dream_processing_fingerprint::phylactery_dream_processing(&prefix)?;

    for fingerprint in [
        memory.fingerprint,
        entity.fingerprint,
        relationship.fingerprint,
        graph.fingerprint,
        memory_graph.fingerprint,
        ego.fingerprint,
        pairs.fingerprint,
        processing.fingerprint,
    ] {
        projection.insert(
            "owner",
            fingerprint.domain.as_bytes().to_vec(),
            json!({
                "domain": fingerprint.domain,
                "local_cut": fingerprint.local_cut,
                "event_cut": fingerprint.event_cut,
                "rows": fingerprint.rows,
                "sha256": fingerprint.sha256,
            }),
        )?;
    }

    let visible = |node: SemanticNodeRef| match node.kind {
        SemanticNodeKind::Memory => memory.current.contains_key(&MemoryId(node.id)),
        SemanticNodeKind::Entity => entity.current.contains_key(&crate::EntityId(node.id)),
        SemanticNodeKind::Observation => relationship
            .current
            .contains_key(&crate::RelationshipId(node.id)),
    };
    let visible_graph: Vec<_> = graph
        .states
        .into_values()
        .filter(|relation| relation.active && visible(relation.source) && visible(relation.target))
        .collect();
    for relation in &visible_graph {
        let key = [
            relation.source.kind.code().to_be_bytes().as_slice(),
            relation.source.id.as_slice(),
            relation.target.kind.code().to_be_bytes().as_slice(),
            relation.target.id.as_slice(),
            relation.kind.code().to_be_bytes().as_slice(),
        ]
        .concat();
        projection.insert(
            "visible_graph",
            key,
            json!({
                "source_kind": relation.source.kind.code(), "source": relation.source.id,
                "target_kind": relation.target.kind.code(), "target": relation.target.id,
                "kind": relation.kind.code(), "origin": relation.origin.code(),
                "graph_version": relation.graph_version, "global_version": relation.global_version
            }),
        )?;
    }

    let genesis = prefix.rewrite_genesis().map(|g| {
        let crate::RewriteGenesis {
            destination_owner,
            generation,
            predecessor_owner,
            predecessor_generation,
            source_head_digest,
            policy_digest,
            created_at_unix_millis,
        } = g;
        json!({
            "destination_owner": destination_owner, "generation": generation.0,
            "predecessor_owner": predecessor_owner, "predecessor_generation": predecessor_generation.0,
            "source_head_digest": source_head_digest, "policy_digest": policy_digest,
            "created_at_unix_millis": created_at_unix_millis
        })
    });
    projection.insert(
        "lineage",
        b"validated",
        json!({
            "generation": prefix.history_generation().0,
            "genesis": genesis,
            "graph_baseline": prefix.graph.historical_baseline_identity()
                .map(|(owner, generation, policy)| json!([owner, generation.0, policy]))
        }),
    )?;

    Ok(WholePhyCut {
        fingerprint: projection.fingerprint()?,
        cuts,
        ego_version,
        current_memories: memory.current,
        visible_graph,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GraphRelationKind, MemoryDraft, Phylactery};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(".test-tmp")
                .join(format!("whole-phy-oracle-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn draft(mutation: &str) -> MemoryDraft {
        MemoryDraft {
            category: "fact".into(),
            memory_type: "project".into(),
            authority_kind: "direct".into(),
            temporal_status: "unknown".into(),
            title: mutation.into(),
            content: format!("content {mutation}"),
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
            mutation_id: mutation.into(),
            created_at_ns: 1,
            updated_at_ns: 1,
        }
    }

    fn assert_same(left: &WholePhyCut, right: &WholePhyCut) {
        assert_eq!(left.fingerprint, right.fingerprint);
        assert_eq!(left.cuts, right.cuts);
        assert_eq!(left.ego_version, right.ego_version);
        assert_eq!(left.current_memories, right.current_memories);
        assert_eq!(left.visible_graph, right.visible_graph);
    }

    #[test]
    fn phylactery_global_cut_composes_memory_graph_and_clock_neutral_ego_across_reopen() {
        let fixture = Fixture::new();
        let path = fixture.0.join("owner.phy");
        let mut phy = Phylactery::create(&path).unwrap();
        phy.create_ego_anchor(crate::EgoAnchorPriority::Normal, "before memory".into())
            .unwrap();

        let first = phy.publish_memory(None, 0, draft("first")).unwrap().0.id;
        let first_global = phy.latest_global_version();
        let first_cut = whole_owner_at_phy(&mut phy, first_global).unwrap();
        assert_eq!(first_cut.cuts.memory, 1);
        assert_eq!(first_cut.cuts.graph, 0);
        assert_eq!(first_cut.ego_version, 1);
        assert_eq!(first_cut.current_memories, BTreeMap::from([(first, 1)]));

        let second = phy.publish_memory(None, 0, draft("second")).unwrap().0.id;
        let second_global = phy.latest_global_version();
        phy.set_memory_relation(first, second, GraphRelationKind::Factual, true, 0)
            .unwrap();
        let graph_global = phy.latest_global_version();
        let graph_cut = whole_owner_at_phy(&mut phy, graph_global).unwrap();
        assert_eq!(graph_cut.cuts.memory, 2);
        assert_eq!(graph_cut.cuts.graph, 1);
        assert_eq!(graph_cut.cuts.memory_graph, 1);
        assert_eq!(graph_cut.visible_graph.len(), 1);
        assert!(graph_global > second_global);

        let later = phy.publish_memory(None, 0, draft("later")).unwrap().0.id;
        let later_global = phy.latest_global_version();
        phy.create_ego_anchor(crate::EgoAnchorPriority::High, "after later memory".into())
            .unwrap();
        let later_cut = whole_owner_at_phy(&mut phy, later_global).unwrap();
        assert_eq!(later_cut.ego_version, 2);
        assert_eq!(later_cut.current_memories.len(), 3);
        assert_same(
            &whole_owner_at_phy(&mut phy, graph_global).unwrap(),
            &graph_cut,
        );

        phy.tombstone_memory(first, 1).unwrap();
        let terminal_global = phy.latest_global_version();
        let terminal_cut = whole_owner_at_phy(&mut phy, terminal_global).unwrap();
        assert!(terminal_global > later_global);
        assert_eq!(terminal_cut.cuts.memory, 4);
        assert_eq!(
            terminal_cut.current_memories,
            BTreeMap::from([(second, 1), (later, 1)])
        );
        assert!(terminal_cut.visible_graph.is_empty());

        phy.sync().unwrap();
        drop(phy);
        let mut reopened = Phylactery::open(&path).unwrap();
        assert_same(
            &whole_owner_at_phy(&mut reopened, first_global).unwrap(),
            &first_cut,
        );
        assert_same(
            &whole_owner_at_phy(&mut reopened, graph_global).unwrap(),
            &graph_cut,
        );
        assert_same(
            &whole_owner_at_phy(&mut reopened, later_global).unwrap(),
            &later_cut,
        );
        assert_same(
            &whole_owner_at_phy(&mut reopened, terminal_global).unwrap(),
            &terminal_cut,
        );
        assert!(whole_owner_at_phy(&mut reopened, terminal_global + 1).is_err());
    }
}
