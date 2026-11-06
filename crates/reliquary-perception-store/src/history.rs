use super::*;

impl PerceptionStore {
    /// Project retained Entity, mention, and Relationship state at one cut.
    pub fn history_at(&self, version: u64) -> Result<PerceptionHistorySnapshot, PerceptionError> {
        if version > self.version {
            return Err(PerceptionError::InvalidField("future perception version"));
        }
        let mut entities = BTreeMap::new();
        for (key @ (id, _), value) in &self.entity_history {
            if self
                .entity_versions
                .get(key)
                .is_some_and(|seen| *seen <= version)
                && self
                    .entity_tombstones
                    .get(id)
                    .is_none_or(|tombstone| tombstone.perception_version > version)
            {
                entities.insert(*id, value.clone());
            }
        }
        let mut mentions = BTreeMap::new();
        for (key @ (mention, _), value) in &self.mention_history {
            if self
                .mention_versions
                .get(key)
                .is_some_and(|seen| *seen <= version)
            {
                mentions.insert(*mention, value.clone());
            }
        }
        let mut relationships = BTreeMap::new();
        for (key @ (id, _), value) in &self.relationship_history {
            if self
                .relationship_versions
                .get(key)
                .is_some_and(|seen| *seen <= version)
                && self
                    .relationship_tombstone_versions
                    .get(id)
                    .is_none_or(|seen| *seen > version)
            {
                relationships.insert(*id, value.clone());
            }
        }
        Ok(PerceptionHistorySnapshot {
            perception_version: version,
            entities: entities.into_values().collect(),
            entity_tombstones: self
                .entity_tombstones
                .values()
                .filter(|value| value.perception_version <= version)
                .copied()
                .collect(),
            entity_description_prunes: self
                .pruned_descriptions
                .iter()
                .filter(|(key, _)| {
                    self.entity_versions
                        .get(key)
                        .is_some_and(|seen| *seen <= version)
                })
                .map(|(_, value)| *value)
                .collect(),
            mentions: mentions.into_values().collect(),
            relationships: relationships.into_values().collect(),
            relationship_tombstones: self
                .relationship_tombstones
                .iter()
                .filter(|(id, _)| {
                    self.relationship_tombstone_versions
                        .get(id)
                        .is_some_and(|seen| *seen <= version)
                })
                .map(|(_, value)| *value)
                .collect(),
            relationship_description_prunes: self
                .pruned_relationship_descriptions
                .iter()
                .filter(|(key, _)| {
                    self.relationship_versions
                        .get(key)
                        .is_some_and(|seen| *seen <= version)
                })
                .map(|(_, value)| *value)
                .collect(),
        })
    }
}
