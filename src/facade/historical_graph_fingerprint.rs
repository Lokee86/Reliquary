//! Accepted Graph publications, independent of physical node ordinals and the
//! current lifecycle exclusions owned by Memory/Entity.
use crate::graph_codec::{decode_batch, decode_mutation, decode_version};
use crate::graph_store::GraphStore;
use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::{
    Container, CvaError, SemanticGraphRelation, SemanticGraphRelationKind, SemanticNodeRef,
};
use serde_json::json;
use std::collections::BTreeMap;

type RelationKey = (SemanticNodeRef, SemanticNodeRef, SemanticGraphRelationKind);

pub(crate) struct GraphHistory {
    owner: [u8; 16],
    transactions: Vec<Vec<SemanticGraphRelation>>,
    times: Vec<Option<i64>>,
    baseline_memories: Vec<crate::MemoryId>,
}

pub(crate) struct GraphCut {
    pub(crate) fingerprint: HistoricalFingerprint,
    pub(crate) states: BTreeMap<RelationKey, SemanticGraphRelation>,
    pub(crate) memory_graph_version: u64,
}

impl GraphHistory {
    pub(crate) fn use_baseline_from(&mut self, store: &GraphStore) {
        self.baseline_memories = store
            .rewrite_baseline_memories()
            .unwrap_or_default()
            .to_vec();
        self.baseline_memories.sort();
    }
    pub(crate) fn read(container: &Container, store: &GraphStore) -> Result<Self, String> {
        let mut scan =
            Container::open_read_only_for_scan(container.path()).map_err(|e| e.to_string())?;
        let mut objects =
            Container::open_read_only_for_scan(container.path()).map_err(|e| e.to_string())?;
        let mut transactions = Vec::new();
        let mut times = Vec::new();
        scan.visit_payloads::<CvaError>(|chunk, payload| {
            let Some(version) = decode_version(payload)? else {
                return Ok(());
            };
            if version.graph_version != transactions.len() as u64 + 1
                || !version.mutation.precedes(chunk)
                || store.transaction_global_versions().get(transactions.len())
                    != Some(&version.global_version)
            {
                return Err(crate::GraphError::InvalidGraphVersion.into());
            }
            let bytes = objects.read(version.mutation)?;
            let mutations = if let Some(batch) = decode_batch(&bytes)? {
                batch
            } else if let Some(single) = decode_mutation(&bytes)? {
                vec![single]
            } else {
                return Err(crate::GraphError::InvalidGraphVersion.into());
            };
            let mut transaction = Vec::new();
            for mutation in mutations {
                let crate::graph_codec::GraphMutationPayload {
                    source,
                    target,
                    kind,
                    active,
                    origin,
                } = mutation;
                transaction.push(SemanticGraphRelation {
                    source,
                    target,
                    kind,
                    active,
                    origin,
                    global_version: version.global_version,
                    graph_version: version.graph_version,
                });
            }
            transaction.sort_by_key(|r| (r.source, r.target, r.kind));
            if transaction.is_empty()
                || transaction.windows(2).any(|pair| {
                    (pair[0].source, pair[0].target, pair[0].kind)
                        == (pair[1].source, pair[1].target, pair[1].kind)
                })
            {
                return Err(crate::GraphError::InvalidGraphVersion.into());
            }
            transactions.push(transaction);
            times.push(container.transaction_time_ns(version.global_version));
            Ok(())
        })
        .map_err(|e| e.to_string())?;
        if transactions.len() as u64 != store.graph_version() {
            return Err("Graph oracle publication count differs from accepted replay".into());
        }
        let mut baseline_memories = store
            .rewrite_baseline_memories()
            .unwrap_or_default()
            .to_vec();
        baseline_memories.sort();
        Ok(Self {
            owner: container
                .owner_uuid()
                .ok_or("Graph oracle needs owner UUID")?,
            transactions,
            times,
            baseline_memories,
        })
    }

    pub(crate) fn at(&self, cut: u64, memory_only: bool) -> Result<GraphCut, String> {
        if cut > self.transactions.len() as u64 {
            return Err("Graph cut exceeds retained head".into());
        }
        if memory_only
            && cut != 0
            && !self.transactions[cut as usize - 1]
                .iter()
                .any(|r| matches!(r.kind, SemanticGraphRelationKind::Memory(_)))
        {
            return Err("MemoryGraph cut is not a qualifying Graph publication".into());
        }
        let mut projection = OwnerProjection::new(
            self.owner,
            if memory_only { "memory_graph" } else { "graph" },
            cut,
            None,
        )?;
        let mut states = BTreeMap::new();
        let mut memory_graph_version = 0;
        // A fresh-history baseline admits isolated Memory nodes at version zero.
        for memory in &self.baseline_memories {
            projection.insert("baseline_memory", memory.0, json!(memory.0))?;
        }
        for (index, transaction) in self.transactions.iter().take(cut as usize).enumerate() {
            if transaction
                .iter()
                .any(|r| matches!(r.kind, SemanticGraphRelationKind::Memory(_)))
            {
                memory_graph_version = index as u64 + 1;
            }
            let selected: Vec<_> = transaction
                .iter()
                .filter(|r| !memory_only || matches!(r.kind, SemanticGraphRelationKind::Memory(_)))
                .collect();
            if selected.is_empty() {
                continue;
            }
            projection.insert(
                "publication",
                (index as u64 + 1).to_be_bytes(),
                json!({
                    "global_version": transaction[0].global_version, "time_ns": self.times[index],
                    "changes": selected.iter().map(|r| relation_value(**r)).collect::<Vec<_>>()
                }),
            )?;
            for relation in selected {
                states.insert((relation.source, relation.target, relation.kind), *relation);
            }
        }
        projection.insert(
            "clock",
            b"memory_graph".to_vec(),
            json!(memory_graph_version),
        )?;
        Ok(GraphCut {
            fingerprint: projection.fingerprint()?,
            states,
            memory_graph_version,
        })
    }

    /// Full-Graph positions that actually advance the MemoryGraph projection.
    /// This scans the accepted transactions once and avoids fingerprinting
    /// every intermediate cut merely to discover qualifying values.
    pub(crate) fn memory_graph_positions(&self) -> Vec<u64> {
        let mut positions = vec![0];
        for (index, transaction) in self.transactions.iter().enumerate() {
            if transaction
                .iter()
                .any(|relation| matches!(relation.kind, SemanticGraphRelationKind::Memory(_)))
            {
                positions.push(index as u64 + 1);
            }
        }
        positions
    }
}

fn relation_value(relation: SemanticGraphRelation) -> serde_json::Value {
    let SemanticGraphRelation {
        source,
        target,
        kind,
        active,
        origin,
        global_version,
        graph_version,
    } = relation;
    json!({"source_kind": source.kind.code(), "source": source.id,
        "target_kind": target.kind.code(), "target": target.id,
        "kind": kind.code(), "active": active, "origin": origin.code(),
        "global_version": global_version, "graph_version": graph_version})
}

#[path = "historical_graph_fingerprint_tests.rs"]
mod tests;
