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
            .turns
            .iter()
            .take_while(|turn| turn.archive_version <= version)
            .map(|turn| {
                Ok(crate::HistoricalArchiveTurn {
                    content: self
                        .read_text(turn.node.content_id)?
                        .map(Arc::unwrap_or_clone),
                    attachments: self.read_attachments(&turn.attachments)?,
                    activity_position: turn.activity.map(NonZeroU64::get),
                    node: turn.node.clone(),
                })
            })
            .collect::<Result<Vec<_>, ArchiveError>>()?;
        Ok(crate::ArchiveHistorySnapshot {
            archive_version: version,
            publication_bindings: self
                .publication_bindings
                .iter()
                .take_while(|binding| binding.archive_version <= version)
                .copied()
                .collect(),
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
            retained_publications: self
                .retained_publications
                .iter()
                .take_while(|record| record.archive_version <= version)
                .cloned()
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
