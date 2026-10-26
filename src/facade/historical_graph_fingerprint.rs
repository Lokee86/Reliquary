//! Accepted Graph publications, independent of physical node ordinals and the
//! current lifecycle exclusions owned by Memory/Entity.
use crate::graph_owner::GraphOwner;
use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::{CvaError, SemanticGraphRelation, SemanticGraphRelationKind, SemanticNodeRef};
use reliquary_container::Container;
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
    /// Disposable acceleration of the existing schema-1 oracle, not a new hash
    /// scheme or runtime checkpoint. Canonicalize each immutable row once, then
    /// compute every original cut hash over exactly the original ordered bytes.
    pub(crate) fn all_cut_fingerprints(
        &self,
        memory_only: bool,
    ) -> Result<Vec<HistoricalFingerprint>, String> {
        use crate::historical_fingerprint::{canonical_value, hash_field};
        use sha2::{Digest, Sha256};
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            mpsc,
        };
        fn field(out: &mut Vec<u8>, bytes: &[u8]) {
            out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            out.extend_from_slice(bytes);
        }
        fn row(
            out: &mut Vec<u8>,
            family: &str,
            key: &[u8],
            value: &serde_json::Value,
        ) -> Result<(), String> {
            field(out, family.as_bytes());
            field(out, key);
            field(out, &canonical_value(value)?);
            Ok(())
        }
        let mut baseline = Vec::new();
        let mut ids = std::collections::BTreeSet::new();
        for id in &self.baseline_memories {
            if !ids.insert(*id) {
                return Err("duplicate baseline Memory".into());
            }
        }
        for id in &ids {
            row(&mut baseline, "baseline_memory", &id.0, &json!(id.0))?;
        }
        let mut publications = Vec::new();
        let mut contexts = vec![(0u64, 0u64, 0usize, 0u64)];
        let mut clock = 0;
        let mut count = 0;
        for (index, transaction) in self.transactions.iter().enumerate() {
            let cut = index as u64 + 1;
            let qualifies = transaction
                .iter()
                .any(|r| matches!(r.kind, SemanticGraphRelationKind::Memory(_)));
            if qualifies {
                clock = cut;
            }
            let selected: Vec<_> = transaction
                .iter()
                .filter(|r| !memory_only || matches!(r.kind, SemanticGraphRelationKind::Memory(_)))
                .collect();
            if !selected.is_empty() {
                row(
                    &mut publications,
                    "publication",
                    &cut.to_be_bytes(),
                    &json!({"global_version":transaction[0].global_version,"time_ns":self.times[index],"changes":selected.iter().map(|r|relation_value(**r)).collect::<Vec<_>>()}),
                )?;
                count += 1;
            }
            if !memory_only || qualifies {
                contexts.push((cut, clock, publications.len(), count));
            }
        }
        let domain = if memory_only { "memory_graph" } else { "graph" };
        let workers = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .min(8)
            .min(contexts.len());
        let cursor = AtomicUsize::new(0);
        let (sender, receiver) = mpsc::channel();
        let mut values = vec![None; contexts.len()];
        std::thread::scope(|scope| {
            for _ in 0..workers {
                let sender = sender.clone();
                let contexts = &contexts;
                let cursor = &cursor;
                let baseline = &baseline;
                let publications = &publications;
                let baseline_count = ids.len() as u64;
                scope.spawn(move || {
                    loop {
                        let index = cursor.fetch_add(1, Ordering::Relaxed);
                        let Some(&(cut, clock, end, count)) = contexts.get(index) else {
                            break;
                        };
                        let rows = baseline_count + 1 + count;
                        let mut hash = Sha256::new();
                        hash.update(b"reliquary:historical-owner-state:v1\0");
                        hash.update(1u32.to_le_bytes());
                        hash.update(self.owner);
                        hash_field(&mut hash, domain.as_bytes());
                        hash.update(cut.to_le_bytes());
                        hash.update([0]);
                        hash.update(rows.to_le_bytes());
                        hash.update(baseline);
                        hash_field(&mut hash, b"clock");
                        hash_field(&mut hash, b"memory_graph");
                        hash_field(&mut hash, clock.to_string().as_bytes());
                        hash.update(&publications[..end]);
                        sender
                            .send((
                                index,
                                HistoricalFingerprint {
                                    schema: 1,
                                    owner_uuid: self.owner,
                                    domain: domain.into(),
                                    local_cut: cut,
                                    event_cut: None,
                                    rows,
                                    sha256: hash.finalize().into(),
                                },
                            ))
                            .unwrap();
                    }
                });
            }
            drop(sender);
            for (index, value) in receiver {
                values[index] = Some(value);
            }
        });
        values
            .into_iter()
            .map(|v| v.ok_or_else(|| "missing cached Graph cut".into()))
            .collect()
    }
    pub(crate) fn use_baseline_from(&mut self, store: &GraphOwner) {
        self.baseline_memories = store
            .rewrite_baseline_memories()
            .unwrap_or_default()
            .to_vec();
        self.baseline_memories.sort();
    }
    pub(crate) fn read(container: &Container, store: &GraphOwner) -> Result<Self, String> {
        let scan = container.read_source().map_err(|e| e.to_string())?;
        let objects = container.read_source().map_err(|e| e.to_string())?;
        let mut transactions = Vec::new();
        let mut times = Vec::new();
        (|| -> Result<(), CvaError> {
            let mut physical_cursor = scan.physical_cursor()?;
            while let Some(physical_frame) = physical_cursor.next_frame()? {
                let chunk = physical_frame.object;
                let payload = physical_frame.payload.as_slice();
                (|| -> Result<(), CvaError> {
                    let Some(version) =
                        crate::graph_compact_codec::publication::version_at(chunk, payload)?
                    else {
                        return Ok(());
                    };
                    if version.graph_version != transactions.len() as u64 + 1
                        || (version.mutation != chunk && !version.mutation.precedes(chunk))
                        || store.transaction_global_versions().get(transactions.len())
                            != Some(&version.global_version)
                    {
                        return Err(crate::GraphError::InvalidGraphVersion.into());
                    }
                    let bytes = objects.read(version.mutation)?;
                    let mutations = store
                        .decode_stored_mutations(&bytes)?
                        .ok_or(crate::GraphError::InvalidGraphVersion)?;
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
                            source: source.into(),
                            target: target.into(),
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
                })()?;
            }
            Ok(())
        })()
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
