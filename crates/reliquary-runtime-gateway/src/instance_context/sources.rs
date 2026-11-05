use super::*;

impl Instance {
    pub(super) fn read_context_attachments(
        &self,
        owner: OwnerId,
        selectors: &[ProjectContextAttachmentRef],
    ) -> Result<Vec<reliquary_owner_session::model::SourceAttachment>, GatewayError> {
        let mut result = Vec::with_capacity(selectors.len());
        for selector in selectors {
            self.authorize_queue_owner(owner)?;
            let attachments = match self.with_rel_writer(owner, |writer| {
                writer
                    .read_session()
                    .archive_source_attachments(&selector.conversation_id, &selector.source_node_id)
            }) {
                Ok(attachments) => attachments,
                Err(GatewayError::WriterUnavailable) => {
                    self.reliquary(owner)?.archive_source_attachments(
                        &selector.conversation_id,
                        &selector.source_node_id,
                    )
                }
                Err(error) => return Err(error),
            };
            let attachment = attachments
                .into_iter()
                .find(|attachment| attachment.id == selector.attachment_id)
                .ok_or_else(|| {
                    GatewayError::ProjectSource(
                        "project attachment is not retained by this REL".into(),
                    )
                })?;
            result.push(attachment);
        }
        Ok(result)
    }

    pub(super) fn read_context_memory(
        &self,
        source: ContextMemoryRef,
    ) -> Result<(ContextMemoryEvidence, ContextMemoryBinding), GatewayError> {
        let history = match source.kind {
            ContextMemoryOwnerKind::Rel => {
                let id = reliquary_owner_session::model::MemoryId(source.memory_id);
                match self.with_rel_writer(source.owner, |writer| {
                    writer.read_session().memory_history(id)
                }) {
                    Ok(result) => result?,
                    Err(GatewayError::WriterUnavailable) => {
                        self.reliquary(source.owner)?.memory_history(id)?
                    }
                    Err(error) => return Err(error),
                }
            }
            ContextMemoryOwnerKind::Phy => {
                self.ensure_credential_active()?;
                if !self.phy_grants.contains(&source.owner) {
                    return Err(GatewayError::UnauthorizedPhy {
                        principal: self.principal.to_string(),
                        owner: source.owner,
                    });
                }
                let id = reliquary_owner_session::model::MemoryId(source.memory_id);
                let mounted = self
                    .owners
                    .read()
                    .map_err(|_| GatewayError::RegistryPoisoned)?
                    .get(&source.owner)
                    .cloned()
                    .ok_or(GatewayError::UnknownOwner(source.owner))?;
                match mounted {
                    MountedOwner::Phy(session) => session.memory_history(id)?,
                    MountedOwner::PhyWriter(writer) => writer
                        .writer
                        .lock()
                        .map_err(|_| GatewayError::RegistryPoisoned)?
                        .memory_history(id)?,
                    _ => return Err(GatewayError::WrongOwnerKind(source.owner)),
                }
            }
        }
        .ok_or_else(|| GatewayError::ProjectSource("selected Memory is unavailable".into()))?;
        if history.tombstone.is_some() {
            return Err(GatewayError::ProjectSource(
                "selected Memory is tombstoned".into(),
            ));
        }
        let revision = history.revisions.last().ok_or_else(|| {
            GatewayError::ProjectSource("selected Memory has no current revision".into())
        })?;
        if revision.record.archived {
            return Err(GatewayError::ProjectSource(
                "selected Memory is archived".into(),
            ));
        }
        let body = match &revision.payload {
            reliquary_owner_session::model::PayloadState::Available(body) => body,
            reliquary_owner_session::model::PayloadState::Absent => {
                return Err(GatewayError::ProjectSource(
                    "selected Memory body is unavailable".into(),
                ));
            }
        };
        let binding = ContextMemoryBinding {
            source,
            revision: revision.record.revision,
            body_id: revision.record.body_id.0,
        };
        let evidence = ContextMemoryEvidence {
            scope: match source.kind {
                ContextMemoryOwnerKind::Rel => ContextMemoryScope::Rel,
                ContextMemoryOwnerKind::Phy => ContextMemoryScope::Phy,
            },
            owner_id: source.owner.as_bytes(),
            memory_id: source.memory_id,
            revision: binding.revision,
            body_id: binding.body_id,
            title: body.title.clone(),
            content: body.content.clone(),
        };
        Ok((evidence, binding))
    }
}
