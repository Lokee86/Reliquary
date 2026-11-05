//! Initial linkage evidence comes from accepted pair decisions, never rewires.
use super::*;
use reliquary_graph::{GraphAdmission, GraphStore, NodeRef};

fn connected(graph: &GraphStore, a: [u8; 32], b: [u8; 32]) -> Result<bool, String> {
    Ok(graph
        .duplicate_context(&[a], 128)
        .map_err(|e| e.to_string())?
        .0
        .contains(&b))
}
pub(crate) fn direct(before: &GraphStore, after: &GraphStore, pair: &Pair) -> Result<bool, String> {
    let Some(kind) = decode_kind(pair.kind)? else {
        return Ok(false);
    };
    if pair.rejected {
        return Ok(false);
    }
    if kind == RelationKind::DuplicateOf {
        if !connected(after, pair.source, pair.candidate)? {
            return Err("accepted duplicate lacks committed component".into());
        }
        return Ok(!connected(before, pair.source, pair.candidate)?);
    }
    let exists = |g: &GraphStore| {
        [(pair.source, pair.candidate), (pair.candidate, pair.source)]
            .into_iter()
            .any(|(a, b)| {
                g.relation(NodeRef::Memory(a), NodeRef::Memory(b), kind)
                    .is_some_and(|r| r.active)
            })
    };
    if !exists(after) {
        return Err("accepted pair lacks committed relation".into());
    }
    Ok(!exists(before))
}
impl DreamBatchRecord {
    pub fn validate_direct(&self, before: &GraphStore) -> Result<(), String> {
        let admission =
            GraphAdmission::for_changes(self.owner, self.observed.iter().map(|o| o.id), []);
        let mut after = before.clone();
        for bytes in &self.graph_records {
            after
                .apply_dream_relation(
                    &admission,
                    reliquary_graph::dream_relation_in_frame(bytes, self.owner)
                        .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
        }
        for pair in &self.pairs {
            if direct(before, &after, pair)? != pair.direct {
                return Err("invented direct Dream pair evidence".into());
            }
        }
        Ok(())
    }
}
