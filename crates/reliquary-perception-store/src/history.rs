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
        let mut entity_revisions = self
            .entity_history
            .iter()
            .filter_map(|(key, value)| {
                self.entity_versions.get(key).and_then(|seen| {
                    (*seen <= version).then(|| HistoricalEntityRevision {
                        perception_version: *seen,
                        value: value.clone(),
                    })
                })
            })
            .collect::<Vec<_>>();
        entity_revisions.sort_by_key(|revision| revision.perception_version);
        let mut mention_revisions = self
            .mention_history
            .iter()
            .filter_map(|(key, value)| {
                self.mention_versions.get(key).and_then(|seen| {
                    (*seen <= version).then(|| HistoricalMentionRevision {
                        perception_version: *seen,
                        value: value.clone(),
                    })
                })
            })
            .collect::<Vec<_>>();
        mention_revisions.sort_by_key(|revision| revision.perception_version);
        let mut relationship_revisions = self
            .relationship_history
            .iter()
            .filter_map(|(key, value)| {
                self.relationship_versions.get(key).and_then(|seen| {
                    (*seen <= version).then(|| HistoricalRelationshipRevision {
                        perception_version: *seen,
                        value: value.clone(),
                    })
                })
            })
            .collect::<Vec<_>>();
        relationship_revisions.sort_by_key(|revision| revision.perception_version);
        Ok(PerceptionHistorySnapshot {
            perception_version: version,
            entity_revisions,
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
            mention_revisions,
            mentions: mentions.into_values().collect(),
            relationship_revisions,
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

impl PerceptionStore {
    /// Resolve the Entity active at an admitted physical-prefix version.
    pub fn entity_at_cut(&self, id: EntityId, version: u64) -> Option<&Entity> {
        if self
            .entity_tombstones
            .get(&id)
            .is_some_and(|row| row.perception_version <= version)
        {
            return None;
        }
        self.entity_history
            .range((id, 0)..=(id, u64::MAX))
            .rev()
            .find(|(key, _)| {
                self.entity_versions
                    .get(key)
                    .is_some_and(|seen| *seen <= version)
            })
            .map(|(_, entity)| entity)
    }

    /// Resolve the Relationship active at an admitted physical-prefix version.
    pub fn relationship_at_cut(&self, id: RelationshipId, version: u64) -> Option<&Relationship> {
        if self
            .relationship_tombstone_versions
            .get(&id)
            .is_some_and(|seen| *seen <= version)
        {
            return None;
        }
        self.relationship_history
            .range((id, 0)..=(id, u64::MAX))
            .rev()
            .find(|(key, _)| {
                self.relationship_versions
                    .get(key)
                    .is_some_and(|seen| *seen <= version)
            })
            .map(|(_, relationship)| relationship)
    }
}

/// The version advanced by a perception carrier already admitted by the store.
pub fn perception_version_in_frame(payload: &[u8]) -> Result<Option<u64>, PerceptionError> {
    if let Some(publication) = clocked_entity_in_frame(payload)? {
        return Ok(Some(match publication.accepted {
            RetainedEntity::Revision { entity_version, .. } => entity_version,
            RetainedEntity::Terminal { terminal, .. } => terminal.perception_version,
        }));
    }
    if let Some(record) = relationship_codec::RelationshipRecord::decode(payload)? {
        return Ok(match record {
            relationship_codec::RelationshipRecord::Revision {
                perception_version, ..
            }
            | relationship_codec::RelationshipRecord::Tombstone {
                perception_version, ..
            } => Some(perception_version),
            relationship_codec::RelationshipRecord::Prune { .. } => None,
        });
    }
    if let Some((_, tombstone)) = decode_entity_tombstone(payload)? {
        return Ok(Some(tombstone.perception_version));
    }
    Ok(Record::decode(payload)?.map(|record| record.version()))
}
