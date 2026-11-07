use super::*;

impl Archive {
    pub fn retained_publications(&self) -> &[crate::RetainedArchivePublication] {
        &self.retained_publications
    }

    pub fn project_correlations(&self) -> &[crate::ProjectCorrelation] {
        &self.project_correlations
    }

    /// Resolve the source coordinate using the representation accepted at this cut.
    pub fn fragment_vector_source_version(
        &self,
        id: crate::FragmentId,
        archive_cut: u64,
    ) -> Option<u64> {
        if let Some(definition) = self.fragment_definition(id)
            && definition.archive_cut < archive_cut
        {
            return self
                .node_archive_version(
                    &definition.fragment.conversation_id,
                    &definition.fragment.end_node_id,
                )
                .filter(|version| *version <= archive_cut);
        }
        self.retained_fragment_versions
            .get(&id)
            .copied()
            .filter(|version| *version <= archive_cut)
    }

    pub(super) fn apply_retained_publication(
        &mut self,
        value: crate::RetainedArchiveValue,
    ) -> Result<(), ArchiveError> {
        let archive_version = self.next_archive_version()?;
        match &value {
            crate::RetainedArchiveValue::Fragment { fragment } => {
                if self
                    .conversation_tombstones
                    .contains_key(&fragment.conversation_id)
                    || self.retained_fragment_versions.contains_key(&fragment.id)
                    || self.fragment_definitions.contains_key(&fragment.id)
                    || fragment.id
                        != crate::fragment_id(
                            &fragment.conversation_id,
                            &fragment.start_node_id,
                            &fragment.end_node_id,
                        )
                    || !self
                        .branch_path(&fragment.conversation_id, &fragment.end_node_id)?
                        .iter()
                        .any(|node| node.id == fragment.start_node_id)
                {
                    return Err(ArchiveError::CorruptRecord(
                        "invalid retained Archive fragment",
                    ));
                }
                self.retained_fragment_versions
                    .insert(fragment.id, archive_version);
            }
            crate::RetainedArchiveValue::ConversationMetadata {
                conversation_id,
                title,
                policy_revision,
                ..
            } => {
                validate_text(conversation_id, "conversation id")?;
                if self.conversation_tombstones.contains_key(conversation_id)
                    || *policy_revision == 0
                {
                    return Err(ArchiveError::CorruptRecord(
                        "invalid retained conversation metadata",
                    ));
                }
                if let Some(title) = title {
                    validate_text(title, "conversation title")?;
                    self.conversation_titles.insert(
                        conversation_id.clone(),
                        crate::ConversationTitle {
                            conversation_id: conversation_id.clone(),
                            title: title.clone(),
                            archive_version,
                        },
                    );
                } else {
                    self.conversation_titles.remove(conversation_id);
                }
            }
        }
        self.retained_publications
            .push(crate::RetainedArchivePublication {
                archive_version,
                value,
            });
        self.archive_version = archive_version;
        Ok(())
    }

    pub(super) fn apply_project_correlation(
        &mut self,
        correlation: crate::ProjectCorrelation,
    ) -> Result<(), ArchiveError> {
        let revision = &correlation.project_revision;
        if correlation.sequence != self.project_correlations.len() as u64 + 1
            || revision.repository.repository_id.trim().is_empty()
            || revision.revision.trim().is_empty()
            || correlation.archive_version > self.archive_version
            || self.project_correlations.first().is_some_and(|first| {
                let known = &first.project_revision.repository;
                known.kind != revision.repository.kind
                    || known.repository_id != revision.repository.repository_id
            })
        {
            return Err(ArchiveError::CorruptRecord(
                "invalid retained project correlation",
            ));
        }
        validate_repo_relative_path(&revision.repository.project_path, true)?;
        self.project_correlations.push(correlation);
        Ok(())
    }
}
