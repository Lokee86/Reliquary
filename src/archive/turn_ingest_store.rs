use crate::archive_store::{hash_content, validate_text};
use crate::file_store::build_stored_file;
use crate::project_file_binding_store::ProjectFileStore;
use crate::turn_ingest_codec::encode_ingested_turn;
use crate::{Archive, ArchiveError, Container, IncomingTurn, IngestedTurn, Node, StoredFile};
use std::collections::HashSet;

impl Archive {
    pub(crate) fn ingest_turn(
        &mut self,
        container: &mut Container,
        project_files: &ProjectFileStore,
        incoming: IncomingTurn,
    ) -> Result<crate::archive_activity::TurnAcceptance<IngestedTurn>, ArchiveError> {
        validate_text(&incoming.id, "node id")?;
        validate_text(&incoming.conversation_id, "conversation id")?;
        validate_text(&incoming.role, "role")?;
        if let Some(principal_id) = incoming.principal_id.as_deref() {
            validate_text(principal_id, "principal id")?;
        }
        self.validate_parent(&incoming.conversation_id, incoming.parent_id.as_deref())?;

        let node = Node {
            id: incoming.id,
            conversation_id: incoming.conversation_id,
            parent_id: incoming.parent_id,
            role: incoming.role,
            principal_id: incoming.principal_id,
            timestamp_ns: incoming.timestamp_ns,
            content_id: hash_content(incoming.content.as_bytes()),
        };
        let mut prepared = Vec::with_capacity(incoming.attachments.len());
        let mut seen = HashSet::new();
        for attachment in incoming.attachments {
            let file =
                build_stored_file(attachment.filename, attachment.mime_type, &attachment.bytes)?;
            if !seen.insert(file.id) {
                return Err(ArchiveError::DuplicateTurnAttachment);
            }
            if let Some(existing) = self.files.get(file.id)
                && existing != &file
            {
                return Err(ArchiveError::ConflictingFile);
            }
            prepared.push((file, attachment.bytes));
        }
        let mut project_attachments = incoming.project_attachments;
        for file in &project_attachments {
            if !project_files.contains(file.id) {
                return Err(ArchiveError::MissingProjectFile);
            }
            if !seen.insert(file.id) {
                return Err(ArchiveError::DuplicateTurnAttachment);
            }
            match self.files.get(file.id) {
                Some(existing) if existing == file => {}
                Some(_) => return Err(ArchiveError::ConflictingFile),
                None => return Err(ArchiveError::MissingProjectFile),
            }
        }
        let mut attachments = prepared
            .iter()
            .map(|(file, _)| file.clone())
            .collect::<Vec<_>>();
        attachments.append(&mut project_attachments);
        let ingested = IngestedTurn { node, attachments };

        if let Some(existing) = self
            .nodes
            .get(&ingested.node.conversation_id, &ingested.node.id)
        {
            if existing != &ingested.node {
                return Err(ArchiveError::ConflictingNode);
            }
            let ids = ingested
                .attachments
                .iter()
                .map(|file| file.id)
                .collect::<Vec<_>>();
            let current = self
                .source_attachments
                .get(&ingested.node.conversation_id, &ingested.node.id);
            if current.is_some_and(|existing_ids| existing_ids == ids)
                || (current.is_none() && ids.is_empty())
            {
                let ordinal = self
                    .nodes
                    .ordinal(&ingested.node.conversation_id, &ingested.node.id)
                    .unwrap();
                return Ok(self.activity.acceptance(ingested, ordinal, false));
            }
            return Err(ArchiveError::ConflictingTurnIngest);
        }

        let file_ids = ingested
            .attachments
            .iter()
            .map(|file| file.id)
            .collect::<Vec<_>>();
        if self
            .source_attachments
            .get(&ingested.node.conversation_id, &ingested.node.id)
            .is_some_and(|existing| existing != file_ids)
        {
            return Err(ArchiveError::ConflictingTurnIngest);
        }
        let prepared_activity = self
            .activity
            .prepare_node(&mut self.nodes, &ingested.node)?
            .expect("novel turn was validated above");
        let ordinal = self.nodes.len();
        let payload = encode_ingested_turn(&ingested)?;
        self.put_content(container, ingested.node.content_id, &incoming.content)?;
        for (file, bytes) in &prepared {
            self.put_content_bytes(container, file.content_id, bytes)?;
        }
        let record = container.append(&payload)?;
        let version = self.publish_record(container, record)?;
        self.nodes.insert(ingested.node.clone())?;
        for file in &ingested.attachments {
            self.files.insert(file.clone())?;
        }
        self.source_attachments.insert(
            &ingested.node.conversation_id,
            &ingested.node.id,
            file_ids,
        )?;
        self.activity
            .commit_new(prepared_activity, version.archive_version);
        Ok(self.activity.acceptance(ingested, ordinal, true))
    }

    pub(crate) fn files_for_source(&self, conversation_id: &str, node_id: &str) -> Vec<StoredFile> {
        self.source_attachments
            .get(conversation_id, node_id)
            .into_iter()
            .flatten()
            .filter_map(|file_id| self.files.get(*file_id).cloned())
            .collect()
    }
}
