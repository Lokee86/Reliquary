use super::*;

impl Archive {
    pub fn retained_publications(&self) -> &[crate::RetainedArchivePublication] {
        &self.retained_publications
    }

    pub fn project_correlations(&self) -> &[crate::ProjectCorrelation] {
        &self.project_correlations
    }

    /// Accepted delayed project backing in physical order.
    pub fn project_backing_bindings(&self) -> &[crate::ProjectBackingBinding] {
        &self.project_backing_bindings
    }

    /// Find retained project backing visible at the given global cut.
    pub fn project_backing_at_global_cut(
        &self,
        attachment_id: [u8; 32],
        global_version: u64,
    ) -> Option<&crate::ProjectBackingBinding> {
        let binding =
            &self.project_backing_bindings[*self.project_backing_indices.get(&attachment_id)?];
        (binding.global_version <= global_version).then_some(binding)
    }

    pub(super) fn apply_project_backing(
        &mut self,
        binding: crate::ProjectBackingBinding,
    ) -> Result<(), ArchiveError> {
        if self
            .project_backing_indices
            .contains_key(&binding.attachment_id)
        {
            return Err(ArchiveError::CorruptRecord(
                "duplicate project backing record",
            ));
        }
        if !self
            .turns
            .iter()
            .flat_map(|turn| turn.attachments.iter())
            .any(|attachment| {
                attachment.id == binding.attachment_id
                    && attachment.content_id == binding.content_id
                    && attachment.project_reference.is_some()
            })
        {
            return Err(ArchiveError::CorruptRecord(
                "project backing lacks accepted attachment metadata",
            ));
        }
        self.project_backing_indices
            .insert(binding.attachment_id, self.project_backing_bindings.len());
        self.project_backing_bindings.push(binding);
        Ok(())
    }

    /// Resolve the source coordinate using the representation accepted at this cut.
    pub fn fragment_vector_source_version(
        &self,
        id: crate::FragmentId,
        archive_cut: u64,
    ) -> Option<u64> {
        self.fragment_vector_source(id, archive_cut)
            .map(|(_, version)| version)
    }

    /// Recover the exact persisted fragment range for a vector at its source cut.
    pub fn fragment_vector_source(
        &self,
        id: crate::FragmentId,
        archive_cut: u64,
    ) -> Option<(&crate::Fragment, u64)> {
        if let Some(definition) = self.fragment_definition(id)
            && definition.archive_cut < archive_cut
        {
            let fragment = &definition.fragment;
            let version =
                self.node_archive_version(&fragment.conversation_id, &fragment.end_node_id)?;
            return (version <= archive_cut).then_some((fragment, version));
        }
        let index = *self.retained_fragment_index.get(&id)?;
        let publication = self.retained_publications.get(index)?;
        let crate::RetainedArchiveValue::Fragment { fragment } = &publication.value else {
            return None;
        };
        (publication.archive_version <= archive_cut)
            .then_some((fragment, publication.archive_version))
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
                    || fragment.passage.is_some()
                    || self.retained_fragment_index.contains_key(&fragment.id)
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
                self.retained_fragment_index
                    .insert(fragment.id, self.retained_publications.len());
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
