use super::mutation_events::RelMutationFamily;
use super::{GatewayRuntime, InstanceRuntime, OwnerGrant, OwnerKind, operation};
use crate::external_source::{self as external, *};

impl GatewayRuntime {
    pub(super) fn external_read_state(
        &self,
        instance: &InstanceRuntime,
        owner: &str,
        namespace: &str,
    ) -> Result<
        (
            super::OwnerAccessToken,
            external::store::SourceState,
            crate::CompatibilityProfile,
        ),
        ExternalSourceError,
    > {
        let token = self
            .authorize(instance, owner, OwnerGrant::ViewRel)
            .map_err(|_| ExternalSourceError::Unauthorized)?;
        let (state, profile) = self
            .with_rel(&token, |r| {
                let state = r
                    .cva
                    .external_index_state(namespace)
                    .map_err(operation)?
                    .ok_or_else(|| operation(ExternalSourceError::RebuildRequired))?;
                let profile = r.cva.external_profile(&state).map_err(operation)?;
                Ok((state, profile))
            })
            .map_err(|_| ExternalSourceError::RebuildRequired)?;
        Ok((token, state, profile))
    }
    pub fn external_index_status_for(
        &self,
        instance: &InstanceRuntime,
        owner: &str,
        namespace: &str,
    ) -> Result<ExternalIndexStatus, ExternalSourceError> {
        let (token, state, _) = self.external_read_state(instance, owner, namespace)?;
        self.with_rel(&token, |_| Ok(state.status()))
            .map_err(|_| ExternalSourceError::Unauthorized)
    }
    pub fn update_external_index_for(
        &self,
        instance: &InstanceRuntime,
        owner: &str,
        source: &impl ExternalConversationSource,
        endpoint: &impl crate::EmbeddingEndpoint,
    ) -> Result<ExternalIndexStatus, ExternalSourceError> {
        let token = self
            .authorize(instance, owner, OwnerGrant::ManageRel)
            .map_err(|_| ExternalSourceError::Unauthorized)?;
        let (_, old, profile) =
            self.external_read_state(instance, owner, source.source_namespace())?;
        let prepared = external::index::prepare_incremental(
            source,
            instance.principal_id(),
            endpoint,
            profile,
            old,
        )?;
        source.authorize(instance.principal_id())?;
        self.install_external(&token, prepared)
    }
    pub fn rebuild_external_index_for(
        &self,
        instance: &InstanceRuntime,
        owner: &str,
        source: &impl ExternalConversationSource,
        endpoint: &impl crate::EmbeddingEndpoint,
    ) -> Result<ExternalIndexStatus, ExternalSourceError> {
        let token = self
            .authorize(instance, owner, OwnerGrant::ManageRel)
            .map_err(|_| ExternalSourceError::Unauthorized)?;
        let namespace = source.source_namespace().to_owned();
        external::model::identifier(&namespace)?;
        let old = self
            .with_rel(&token, |r| {
                Ok(r.cva.external_index.sources.get(&namespace).cloned())
            })
            .map_err(|_| ExternalSourceError::Unauthorized)?;
        let profile = if let Some(old) = &old {
            self.with_rel(&token, |r| r.cva.external_profile(old).map_err(operation))
                .map_err(|_| ExternalSourceError::RebuildRequired)?
        } else {
            crate::compatibility_profile_probe::profile_from_endpoint(endpoint)
                .map_err(|_| ExternalSourceError::Unavailable)?
        };
        let prepared = external::index::prepare_rebuild(
            source,
            instance.principal_id(),
            endpoint,
            profile,
            old.as_ref(),
        )?;
        source.authorize(instance.principal_id())?;
        self.install_external(&token, prepared)
    }
    fn install_external(
        &self,
        token: &super::OwnerAccessToken,
        prepared: external::index::PreparedIndex,
    ) -> Result<ExternalIndexStatus, ExternalSourceError> {
        let before = prepared.expected_revision;
        self.with_rel_mutation(token, RelMutationFamily::Retrieval, |r| {
            let status = r.cva.external_install(prepared).map_err(operation)?;
            let changed = before != Some(status.revision);
            Ok((status, changed, Vec::new()))
        })
        .map_err(|_| ExternalSourceError::Conflict)
    }
    pub fn search_external_source_for(
        &self,
        instance: &InstanceRuntime,
        owner: &str,
        source: &impl ExternalConversationSource,
        endpoint: &impl crate::EmbeddingEndpoint,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ExternalSearchHit>, ExternalSourceError> {
        if limit == 0 || limit > MAX_EXTERNAL_RESULTS {
            return Err(ExternalSourceError::Bounds);
        }
        source.authorize(instance.principal_id())?;
        let (token, state, profile) =
            self.external_read_state(instance, owner, source.source_namespace())?;
        let vector = external::index::query_vector(endpoint, &profile, query)?;
        let hits = self
            .with_rel(&token, |r| {
                if r.cva.external_index.sources.get(&state.namespace) != Some(&state) {
                    return Err(operation(ExternalSourceError::Conflict));
                }
                r.cva
                    .external_search(&state, Some(&vector), query, limit)
                    .map_err(operation)
            })
            .map_err(|_| ExternalSourceError::Conflict)?;
        validate_external_evidence(
            source,
            instance.principal_id(),
            &state,
            &hits
                .iter()
                .map(|h| h.reference.message.clone())
                .collect::<Vec<_>>(),
        )?;
        self.with_rel(&token, |r| {
            if r.cva.external_index.sources.get(&state.namespace) != Some(&state) {
                return Err(operation(ExternalSourceError::Conflict));
            }
            Ok(hits)
        })
        .map_err(|_| ExternalSourceError::Conflict)
    }
    pub fn search_external_lexical_for(
        &self,
        instance: &InstanceRuntime,
        owner: &str,
        source: &impl ExternalConversationSource,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ExternalSearchHit>, ExternalSourceError> {
        source.authorize(instance.principal_id())?;
        let (token, state, _) =
            self.external_read_state(instance, owner, source.source_namespace())?;
        let hits = self
            .with_rel(&token, |r| {
                if r.cva.external_index.sources.get(&state.namespace) != Some(&state) {
                    return Err(operation(ExternalSourceError::Conflict));
                }
                r.cva
                    .external_search(&state, None, query, limit)
                    .map_err(operation)
            })
            .map_err(|_| ExternalSourceError::Unavailable)?;
        validate_external_evidence(
            source,
            instance.principal_id(),
            &state,
            &hits
                .iter()
                .map(|h| h.reference.message.clone())
                .collect::<Vec<_>>(),
        )?;
        self.with_rel(&token, |r| {
            if r.cva.external_index.sources.get(&state.namespace) != Some(&state) {
                return Err(operation(ExternalSourceError::Conflict));
            }
            Ok(hits)
        })
        .map_err(|_| ExternalSourceError::Conflict)
    }
    pub fn read_external_memory_evidence_for(
        &self,
        instance: &InstanceRuntime,
        owner: &str,
        id: crate::MemoryId,
    ) -> Result<Option<ExternalMemoryEvidence>, ExternalSourceError> {
        let grant = if owner.starts_with("phy-") {
            OwnerGrant::UseOwnPhy
        } else {
            OwnerGrant::ViewRel
        };
        let token = self
            .authorize(instance, owner, grant)
            .map_err(|_| ExternalSourceError::Unauthorized)?;
        match token.kind {
            OwnerKind::Rel => self.with_rel(&token, |r| {
                r.cva.external_memory_evidence(id).map_err(operation)
            }),
            OwnerKind::Phy => self.with_phy(&token, |p| {
                p.external_memory_evidence(id).map_err(operation)
            }),
        }
        .map_err(|_| ExternalSourceError::Unavailable)
    }
}

impl GatewayRuntime {
    /// Return identifier-only provenance without hydrating external canonical text.
    pub fn read_memory_source_evidence_for(
        &self,
        instance: &InstanceRuntime,
        owner: &str,
        id: crate::MemoryId,
    ) -> Result<Option<SourceEvidenceRef>, ExternalSourceError> {
        let grant = if owner.starts_with("phy-") {
            OwnerGrant::UseOwnPhy
        } else {
            OwnerGrant::ViewRel
        };
        let token = self
            .authorize(instance, owner, grant)
            .map_err(|_| ExternalSourceError::Unauthorized)?;
        match token.kind {
            OwnerKind::Rel => self.with_rel(&token, |r| {
                if let Some(evidence) = r.cva.external_memory_evidence(id).map_err(operation)? {
                    return Ok(Some(SourceEvidenceRef::External(evidence)));
                }
                let memory = r.cva.memory(id).map_err(operation)?;
                if let Some(source) = memory.source_ref {
                    return Ok(Some(SourceEvidenceRef::Reliquary(source)));
                }
                match (memory.source_episode_id, memory.source_node_id) {
                    (Some(episode), Some(node)) => {
                        Ok(Some(SourceEvidenceRef::Reliquary(crate::MemorySourceRef {
                            owner_id: owner.to_owned(),
                            principal_id: None,
                            source_episode_id: episode,
                            source_node_id: node,
                            content_source_conversation_id: memory.content_source_conversation_id,
                            content_source_node_id: memory.content_source_node_id,
                            grounding_source_conversation_id: memory
                                .grounding_source_conversation_id,
                            grounding_source_node_id: memory.grounding_source_node_id,
                        })))
                    }
                    _ => Ok(None),
                }
            }),
            OwnerKind::Phy => self.with_phy(&token, |p| {
                if let Some(evidence) = p.external_memory_evidence(id).map_err(operation)? {
                    return Ok(Some(SourceEvidenceRef::External(evidence)));
                }
                Ok(p.memory(id)
                    .map_err(operation)?
                    .source_ref
                    .map(SourceEvidenceRef::Reliquary))
            }),
        }
        .map_err(|_| ExternalSourceError::Unavailable)
    }
}

/// Check message-level authority at the host's fresh manifest cut before delivery
/// or publication. Namespace authorization alone does not authorize every row.
pub(super) fn validate_external_evidence(
    source: &impl ExternalConversationSource,
    principal: &str,
    state: &external::store::SourceState,
    refs: &[ExternalMessageRef],
) -> Result<(), ExternalSourceError> {
    source.authorize(principal)?;
    let manifest = source.snapshot_manifest(principal)?;
    if manifest.source_namespace != state.namespace
        || manifest.contract_version != EXTERNAL_SOURCE_CONTRACT
        || manifest.messages.len() > MAX_EXTERNAL_MANIFEST
    {
        return Err(ExternalSourceError::InvalidContract);
    }
    let mut current = std::collections::BTreeMap::new();
    for descriptor in &manifest.messages {
        descriptor.validate(&state.namespace)?;
        let key = (
            &descriptor.reference.conversation_id,
            &descriptor.reference.message_id,
        );
        if current.insert(key, descriptor).is_some() {
            return Err(ExternalSourceError::InvalidContract);
        }
    }
    for reference in refs {
        let expected = state
            .messages
            .iter()
            .find(|m| m.descriptor.active && m.descriptor.reference == *reference)
            .ok_or(ExternalSourceError::StaleEvidence)?;
        let descriptor = current
            .get(&(&reference.conversation_id, &reference.message_id))
            .ok_or(ExternalSourceError::Unauthorized)?;
        if **descriptor != expected.descriptor {
            return Err(ExternalSourceError::StaleEvidence);
        }
    }
    source.authorize(principal)
}
