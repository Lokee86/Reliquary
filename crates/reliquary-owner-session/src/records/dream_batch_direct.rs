//! Initial linkage evidence comes from accepted pair decisions, never rewires.
use super::*;
use reliquary_graph::{GraphAdmission, GraphError, GraphStore, NodeRef};

fn pair_exists(graph: &GraphStore, pair: &Pair, kind: RelationKind) -> Result<bool, GraphError> {
    if kind == RelationKind::DuplicateOf {
        return Ok(graph
            .duplicate_context(&[pair.source], 128)?
            .0
            .contains(&pair.candidate));
    }
    Ok(
        [(pair.source, pair.candidate), (pair.candidate, pair.source)]
            .into_iter()
            .any(|(a, b)| {
                graph
                    .relation(NodeRef::Memory(a), NodeRef::Memory(b), kind)
                    .is_some_and(|r| r.active)
            }),
    )
}

pub(crate) fn direct(before: &GraphStore, after: &GraphStore, pair: &Pair) -> Result<bool, String> {
    let Some(kind) = decode_kind(pair.kind)? else {
        return Ok(false);
    };
    if pair.rejected {
        return Ok(false);
    }
    if !pair_exists(after, pair, kind).map_err(|e| e.to_string())? {
        return Err(if kind == RelationKind::DuplicateOf {
            "accepted duplicate lacks committed component".into()
        } else {
            "accepted pair lacks committed relation".into()
        });
    }
    Ok(!pair_exists(before, pair, kind).map_err(|e| e.to_string())?)
}

impl DreamBatchRecord {
    /// Apply a decoded batch to an unpublished replay projection. On error
    /// the caller must discard the projection.
    pub(crate) fn replay_graph(
        &self,
        graph: &mut GraphStore,
        admission: &GraphAdmission,
    ) -> Result<(), GraphError> {
        if self.owner != graph.owner_id()
            || admission.owner_id() != Some(self.owner)
            || self.graph_version != graph.graph_version()
        {
            return Err(GraphError::WrongOwner);
        }
        let invalid_evidence = || GraphError::CorruptRecord("invented Dream direct evidence");
        let mut before = Vec::with_capacity(self.pairs.len());
        for pair in &self.pairs {
            let kind = decode_kind(pair.kind).map_err(|_| invalid_evidence())?;
            before.push(match kind {
                Some(kind) if !pair.rejected => Some((kind, pair_exists(graph, pair, kind)?)),
                _ => None,
            });
        }
        for bytes in &self.graph_records {
            graph.apply_dream_relation(
                admission,
                reliquary_graph::dream_relation_in_frame(bytes, self.owner)?,
            )?;
        }
        for (pair, prior) in self.pairs.iter().zip(before) {
            let direct = match prior {
                Some((kind, existed)) => {
                    if !pair_exists(graph, pair, kind)? {
                        return Err(invalid_evidence());
                    }
                    !existed
                }
                None => false,
            };
            if direct != pair.direct {
                return Err(invalid_evidence());
            }
        }
        Ok(())
    }
}
