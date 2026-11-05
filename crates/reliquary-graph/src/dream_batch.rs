//! Validated standard Graph records embedded by the owning session.
use super::*;
impl GraphStore {
    pub fn snapshot_at(&self, version: u64) -> Result<Self, GraphError> {
        if version > self.graph_version {
            return Err(GraphError::CorruptRecord("Graph snapshot ahead of owner"));
        }
        let mut result = self.clone();
        result.graph_version = 0;
        result.states.clear();
        result.history.clear();
        result.outgoing.clear();
        result.incoming.clear();
        result.duplicate_incident.clear();
        for relation in self.history.iter().filter(|r| r.graph_version <= version) {
            result.apply_relation(*relation)?;
        }
        Ok(result)
    }

    pub fn prepare_dream_changes(
        &self,
        admission: &GraphAdmission,
        changes: &[RelationChange],
    ) -> Result<Vec<Vec<u8>>, GraphError> {
        if changes.len() > 256 {
            return Err(GraphError::InvalidRelation("Dream batch limit"));
        }
        let mut staged = self.clone();
        let mut records = Vec::new();
        for change in changes {
            let relation = Relation {
                source: change.source,
                target: change.target,
                kind: change.kind,
                active: change.active,
                origin: change.origin,
                graph_version: staged
                    .graph_version
                    .checked_add(1)
                    .ok_or(GraphError::VersionExhausted)?,
            };
            staged.apply_dream_relation(admission, relation)?;
            records.push(encode_graph(self.owner, relation));
        }
        Ok(records)
    }
    pub fn apply_dream_relation(
        &mut self,
        admission: &GraphAdmission,
        relation: Relation,
    ) -> Result<(), GraphError> {
        if admission.owner_id != Some(self.owner) {
            return Err(GraphError::WrongOwner);
        }
        validate_change(
            admission,
            RelationChange {
                source: relation.source,
                target: relation.target,
                kind: relation.kind,
                active: relation.active,
                origin: relation.origin,
            },
            false,
        )?;
        if relation.origin != RelationOrigin::Dream
            || !matches!(
                relation.kind,
                RelationKind::Topical
                    | RelationKind::Factual
                    | RelationKind::Causal
                    | RelationKind::Recurrent
                    | RelationKind::DuplicateOf
                    | RelationKind::Supersedes
            )
            || self
                .relation(relation.source, relation.target, relation.kind)
                .is_some_and(|r| r.origin == RelationOrigin::User)
        {
            return Err(GraphError::InvalidRelation(
                "Dream batch cannot replace user relations",
            ));
        }
        self.apply_replay(relation, admission)
    }
}
pub fn dream_relation_in_frame(payload: &[u8], owner: [u8; 16]) -> Result<Relation, GraphError> {
    let (record_owner, relation) =
        decode_graph(payload)?.ok_or(GraphError::CorruptRecord("missing Dream Graph record"))?;
    if record_owner != owner {
        return Err(GraphError::WrongOwner);
    }
    Ok(relation)
}
