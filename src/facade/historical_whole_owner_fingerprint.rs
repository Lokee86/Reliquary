//! Test-only composition at G; global ordering is never semantic ancestry.
//!
//! Clocked publications use their own retained coordinates, even inside grouped
//! frames. Clock-neutral state uses the fixed source's accepted replay frontier:
//! all frames before the first frame whose observed global high-water exceeds G.
//! Later clock-neutral appends at the same head therefore belong to that head.
use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::historical_global_cuts::{GlobalOwnerCuts, reliquary_global_cuts};
use crate::{Cva, MemoryId, SemanticGraphRelation, SemanticNodeKind, SemanticNodeRef};
use serde_json::json;
use std::{collections::BTreeMap, fs, io::Read, path::PathBuf};

#[derive(Debug)]
pub(crate) enum GlobalProjectionError {
    MissingArchiveReference,
    Other(String),
}
impl std::fmt::Display for GlobalProjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingArchiveReference => f.write_str("selected source prefix has Archive content without a visible payload or repository-file binding"),
            Self::Other(error) => f.write_str(error),
        }
    }
}
impl From<String> for GlobalProjectionError {
    fn from(error: String) -> Self {
        Self::Other(error)
    }
}
impl From<&str> for GlobalProjectionError {
    fn from(error: &str) -> Self {
        Self::Other(error.into())
    }
}

pub(crate) struct WholeOwnerCut {
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

fn replay_prefix(cva: &Cva, end: u64) -> Result<(Prefix, Cva), GlobalProjectionError> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("global-oracle-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let guard = Prefix(directory);
    let path = guard.0.join("prefix.rel");
    let source = fs::File::open(cva.container.path()).map_err(|e| e.to_string())?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let copied = std::io::copy(&mut source.take(end), &mut output).map_err(|e| e.to_string())?;
    if copied != end {
        return Err("source changed during global prefix copy".into());
    }
    drop(output);
    let prefix = Cva::open(&path).map_err(|error| match error {
        crate::CvaError::Archive(crate::ArchiveError::MissingContent) => {
            GlobalProjectionError::MissingArchiveReference
        }
        error => GlobalProjectionError::Other(error.to_string()),
    })?;
    Ok((guard, prefix))
}

pub(crate) fn whole_owner_at(
    cva: &mut Cva,
    global: u64,
) -> Result<WholeOwnerCut, GlobalProjectionError> {
    let cuts = reliquary_global_cuts(cva, global)?;
    let end = crate::historical_global_frontier::frontier_end(&cva.container, global)?;
    let (_guard, prefix) = replay_prefix(cva, end)?;
    let owner = cva
        .owner_uuid()
        .ok_or("whole-owner oracle needs owner UUID")?;
    let mut projection = OwnerProjection::new(owner, "whole_owner", global, None)?;
    let archive = crate::historical_archive_fingerprint::archive_cut(cva, cuts.archive)?;
    let memory = crate::historical_memory_fingerprint::MemoryHistory::read(
        &mut cva.container,
        &cva.memories,
        &cva.external_publications,
    )?
    .at(cuts.memory)?;
    let entity = crate::historical_entity_fingerprint::entity_cut(
        &cva.container,
        &cva.entities,
        cuts.entity,
    )?;
    let relationship = crate::historical_relationship_fingerprint::relationship_cut(
        &cva.container,
        &cva.relationships,
        cuts.relationship,
    )?;
    let mut history =
        crate::historical_graph_fingerprint::GraphHistory::read(&cva.container, &cva.graph)?;
    history.use_baseline_from(&prefix.graph);
    let graph = history.at(cuts.graph, false)?;
    let memory_graph = history.at(cuts.memory_graph, true)?;
    let vector = crate::historical_vector_generation_fingerprint::VectorHistory::read(cva)?
        .at(cuts.vector_generation)?;
    let ego_version = prefix.ego_version();
    let ego = crate::historical_ego_fingerprint::EgoHistory::read(&prefix.container, &prefix.ego)?
        .at(ego_version)?;
    let freshness = crate::historical_freshness_fingerprint::freshness_at_turn(
        &prefix,
        prefix.rel_turn_count(),
    )?;
    let dream = crate::historical_dream_proof_fingerprint::dream_proof_state(&prefix)?;
    let pairs = crate::historical_dream_pair_fingerprint::reliquary_dream_pairs(&prefix)?;
    let processing =
        crate::historical_dream_processing_fingerprint::reliquary_dream_processing(&prefix)?;
    for fingerprint in [
        archive.fingerprint,
        memory.fingerprint,
        entity.fingerprint,
        relationship.fingerprint,
        graph.fingerprint,
        memory_graph.fingerprint,
        vector.fingerprint,
        ego.fingerprint,
        freshness.fingerprint,
        dream,
        pairs.fingerprint,
        processing.fingerprint,
    ] {
        projection.insert("owner", fingerprint.domain.as_bytes().to_vec(), json!({
            "domain": fingerprint.domain, "local_cut": fingerprint.local_cut,
            "event_cut": fingerprint.event_cut, "rows": fingerprint.rows, "sha256": fingerprint.sha256,
        }))?;
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
        .filter(|r| r.active && visible(r.source) && visible(r.target))
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
        projection.insert("visible_graph", key, json!({"source_kind":relation.source.kind.code(),"source":relation.source.id,
            "target_kind":relation.target.kind.code(),"target":relation.target.id,"kind":relation.kind.code(),
            "origin":relation.origin.code(),"graph_version":relation.graph_version,"global_version":relation.global_version}))?;
    }
    // Open has already validated these semantic lineage identities. Physical
    // byte offsets, frame encodings and the disposable copy never enter rows.
    let genesis = prefix.rewrite_genesis().map(|g| {
        let crate::RewriteGenesis { destination_owner, generation, predecessor_owner, predecessor_generation,
            source_head_digest, policy_digest, created_at_unix_millis } = g;
        json!({"destination_owner":destination_owner,"generation":generation.0,"predecessor_owner":predecessor_owner,
            "predecessor_generation":predecessor_generation.0,"source_head_digest":source_head_digest,
            "policy_digest":policy_digest,"created_at_unix_millis":created_at_unix_millis})
    });
    projection.insert("lineage", b"validated", json!({"generation":prefix.history_generation().0,"genesis":genesis,
        "graph_baseline":prefix.graph.historical_baseline_identity().map(|(owner,generation,policy)| json!([owner,generation.0,policy]))}))?;
    Ok(WholeOwnerCut {
        fingerprint: projection.fingerprint()?,
        cuts,
        ego_version,
        current_memories: memory.current,
        visible_graph,
    })
}

#[path = "historical_whole_owner_grouped_tests.rs"]
mod grouped_tests;
#[path = "historical_whole_owner_lineage_tests.rs"]
mod lineage_tests;
#[path = "historical_whole_owner_fingerprint_tests.rs"]
mod tests;
