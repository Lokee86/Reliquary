use super::*;

impl GraphStore {
    /// Seek relation publications in an exact Graph version range, including
    /// removals. A version used only for Community metadata adds no relation.
    pub fn relation_changes_after(
        &self,
        after: u64,
        through: u64,
        limit: usize,
    ) -> impl Iterator<Item = Relation> + '_ {
        let start = self
            .history
            .partition_point(|relation| relation.graph_version <= after);
        self.history[start..]
            .iter()
            .take_while(move |relation| relation.graph_version <= through)
            .take(limit)
            .copied()
    }

    /// Return every accepted Graph publication through an exact owner-local
    /// version. Unlike `relations_at`, this preserves inactive and superseded
    /// relation states for historical proof.
    pub fn history_at(&self, version: u64) -> Result<Vec<Relation>, GraphError> {
        if version > self.graph_version {
            return Err(GraphError::InvalidRelation("future Graph version"));
        }
        Ok(self
            .history
            .iter()
            .take_while(|relation| relation.graph_version <= version)
            .copied()
            .collect())
    }

    /// Project accepted Graph state at an exact owner-local Graph version.
    pub fn relations_at(&self, version: u64) -> Result<Vec<Relation>, GraphError> {
        if version > self.graph_version {
            return Err(GraphError::InvalidRelation("future Graph version"));
        }
        let mut states = BTreeMap::new();
        for relation in self
            .history
            .iter()
            .take_while(|relation| relation.graph_version <= version)
        {
            states.insert(
                Key {
                    source: relation.source,
                    target: relation.target,
                    kind: relation.kind,
                },
                *relation,
            );
        }
        Ok(states
            .into_values()
            .filter(|relation| relation.active)
            .collect())
    }
}
