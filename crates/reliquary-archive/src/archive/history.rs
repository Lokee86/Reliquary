use super::*;

impl Archive {
    /// Project retained Archive semantics at an exact owner-local version.
    pub fn history_at(&self, version: u64) -> Result<crate::ArchiveHistorySnapshot, ArchiveError> {
        if version > self.archive_version {
            return Err(ArchiveError::InvalidField("future Archive version"));
        }
        let mut branches = BTreeMap::new();
        for (_, branch) in self
            .branch_history
            .iter()
            .take_while(|(accepted, _)| *accepted <= version)
        {
            branches.insert(
                (branch.conversation_id.clone(), branch.id.clone()),
                branch.clone(),
            );
        }
        let turns = self
            .accepted_node_order
            .iter()
            .filter(|key| {
                self.node_archive_versions
                    .get(*key)
                    .is_some_and(|seen| *seen <= version)
            })
            .map(|key| {
                let node = self.nodes[key].clone();
                crate::HistoricalArchiveTurn {
                    content: self.content.get(&node.content_id).cloned(),
                    attachments: self
                        .source_attachments
                        .get(key)
                        .cloned()
                        .unwrap_or_default(),
                    activity_position: self.activity_position_by_node.get(key).copied(),
                    node,
                }
            })
            .collect::<Vec<_>>();
        Ok(crate::ArchiveHistorySnapshot {
            archive_version: version,
            activity_cut: turns
                .iter()
                .filter_map(|turn| turn.activity_position)
                .max()
                .unwrap_or(0),
            turns,
            branch_revisions: self
                .branch_history
                .iter()
                .take_while(|(accepted, _)| *accepted <= version)
                .map(|(archive_version, branch)| crate::HistoricalArchiveBranch {
                    archive_version: *archive_version,
                    branch: branch.clone(),
                })
                .collect(),
            branches: branches.into_values().collect(),
            episodes: self
                .episodes
                .iter()
                .filter(|(id, _)| {
                    self.episode_versions
                        .get(id)
                        .is_some_and(|seen| *seen <= version)
                })
                .map(|(_, episode)| episode.clone())
                .collect(),
            import_receipts: self
                .import_receipts
                .iter()
                .filter(|(key, _)| {
                    self.import_receipt_versions
                        .get(*key)
                        .is_some_and(|seen| *seen <= version)
                })
                .map(|(_, receipt)| receipt.clone())
                .collect(),
            fragment_definitions: self
                .fragment_definitions
                .iter()
                .filter(|(id, _)| {
                    self.fragment_versions
                        .get(id)
                        .is_some_and(|seen| *seen <= version)
                })
                .map(|(_, definition)| definition.clone())
                .collect(),
            conversation_tombstones: self
                .conversation_tombstones
                .values()
                .filter(|tombstone| tombstone.archive_version <= version)
                .cloned()
                .collect(),
        })
    }
}
