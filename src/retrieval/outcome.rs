use super::{MemoryId, RetrievalAnchorReport, RetrievalDirection, RetrievalPreview};
use serde::Serialize;
use std::collections::BTreeSet;

/// Opaque content-bound handoff proposal. Only the trusted host can confirm use.
pub struct PreparedMemoryDelivery(reliquary_runtime_gateway::PreparedMemoryDelivery);

impl PreparedMemoryDelivery {
    pub(super) fn inner(&self) -> &reliquary_runtime_gateway::PreparedMemoryDelivery {
        &self.0
    }

    pub fn stable_id(&self) -> &str {
        self.0.stable_id()
    }
}

impl std::fmt::Debug for PreparedMemoryDelivery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedMemoryDelivery")
            .field("stable_id", &self.stable_id())
            .finish_non_exhaustive()
    }
}

pub type PreparedRetrieval = PreparedMemoryDelivery;

#[derive(Debug, Serialize)]
pub struct RetrievalOutcome {
    #[serde(skip)]
    pub prepared: Option<PreparedMemoryDelivery>,
    pub preview: Option<RetrievalPreview>,
    #[serde(serialize_with = "serialize_ids")]
    pub already_provided: BTreeSet<MemoryId>,
    #[serde(serialize_with = "serialize_anchors")]
    pub anchors: Vec<RetrievalAnchorReport>,
}

impl From<reliquary_runtime_gateway::RetrievalOutcome> for RetrievalOutcome {
    fn from(outcome: reliquary_runtime_gateway::RetrievalOutcome) -> Self {
        Self {
            prepared: outcome.prepared.map(PreparedMemoryDelivery),
            preview: outcome.preview,
            already_provided: outcome.already_provided,
            anchors: outcome.anchors,
        }
    }
}

fn id_string(id: &MemoryId) -> String {
    id.0.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn serialize_ids<S: serde::Serializer>(
    ids: &BTreeSet<MemoryId>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(ids.iter().map(id_string))
}

fn serialize_anchors<S: serde::Serializer>(
    anchors: &[RetrievalAnchorReport],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(anchors.iter().map(|report| serde_json::json!({
        "anchor": id_string(&report.anchor),
        "unseen_neighbors": report.unseen_neighbors,
        "declined_neighbors": report.declined_neighbors,
        "relations_truncated": report.relations.len() > 128,
        "relations": report.relations.iter().take(128).map(|edge| serde_json::json!({
            "neighbor": id_string(&edge.neighbor),
            "kind": format!("{:?}", edge.kind),
            "direction": match edge.direction { RetrievalDirection::Outgoing => "outgoing", RetrievalDirection::Incoming => "incoming" },
        })).collect::<Vec<_>>(),
        "approved": report.approved.iter().map(id_string).collect::<Vec<_>>(),
    })))
}
