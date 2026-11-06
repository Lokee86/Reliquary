use super::*;

impl EgoStore {
    /// Replay retained private-owner state at an exact Ego version.
    pub fn history_at(&self, version: u64) -> Result<crate::EgoHistorySnapshot, EgoError> {
        if version > self.version {
            return Err(EgoError::InvalidRecord("future Ego version"));
        }
        let mut projected = Self::empty(self.owner_kind);
        projected.owner_id = self.owner_id;
        projected.container_identity = self.container_identity;
        for record in self
            .records
            .iter()
            .take_while(|record| record.version() <= version)
        {
            projected.apply_replayed(record.clone())?;
        }
        Ok(crate::EgoHistorySnapshot {
            ego_version: version,
            records: self
                .records
                .iter()
                .take_while(|record| record.version() <= version)
                .map(|record| match record {
                    EgoRecord::Identity {
                        version,
                        id,
                        revision,
                        deleted,
                        name,
                        text,
                    } => crate::EgoHistoryRecord::Identity {
                        ego_version: *version,
                        id: *id,
                        revision: *revision,
                        deleted: *deleted,
                        name: name.clone(),
                        text: text.clone(),
                    },
                    EgoRecord::Personality {
                        version,
                        revision,
                        source_memory_version,
                        text,
                    } => crate::EgoHistoryRecord::Personality {
                        ego_version: *version,
                        revision: *revision,
                        source_memory_version: *source_memory_version,
                        text: text.clone(),
                    },
                    EgoRecord::Anchor {
                        version,
                        id,
                        revision,
                        deleted,
                        priority,
                        text,
                    } => crate::EgoHistoryRecord::Anchor {
                        ego_version: *version,
                        id: *id,
                        revision: *revision,
                        deleted: *deleted,
                        priority: *priority,
                        text: text.clone(),
                    },
                    EgoRecord::Synthesis {
                        version,
                        revision,
                        source_memory_version,
                        text,
                    } => crate::EgoHistoryRecord::Synthesis {
                        ego_version: *version,
                        revision: *revision,
                        source_memory_version: *source_memory_version,
                        text: text.clone(),
                    },
                })
                .collect(),
            identities: projected.identities.into_values().collect(),
            tombstoned_identities: projected.identity_tombstones.into_keys().collect(),
            personality: projected.personality,
            synthesis: projected.synthesis,
            anchors: projected.anchors.into_values().collect(),
            tombstoned_anchors: projected.anchor_tombstones.into_keys().collect(),
        })
    }
}
