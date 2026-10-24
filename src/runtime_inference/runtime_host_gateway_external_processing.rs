use crate::external_index::model::identifier;
use crate::external_index::store::SourceState;
use crate::external_source::processing::{
    self as source_processing, PreparedExternalMemory, PreparedExternalProcessing,
};
use crate::external_source::{
    ExternalConversationSource, ExternalMemoryEvidence, ExternalSourceError,
};
use crate::{GeneralEndpoint, InsomniaExtractor, MemoryDraft, MemoryRoutingMetadata};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn prepare<E: GeneralEndpoint>(
    owner: &str,
    principal: &str,
    scope: &str,
    state: SourceState,
    refs: &[ExternalMessageRef],
    source: &impl ExternalConversationSource,
    extractor: &InsomniaExtractor<E>,
    now_ns: i64,
) -> Result<PreparedExternalProcessing, ExternalSourceError> {
    identifier(scope)?;
    let expected = source_processing::descriptors(&state, refs)?;
    let window = source_processing::window_key(owner, scope, &expected);
    let hydrated = source.hydrate(principal, &expected)?;
    if hydrated.len() != expected.len() {
        return Err(ExternalSourceError::InvalidContract);
    }
    let mut bytes = 0;
    for (message, descriptor) in hydrated.iter().zip(&expected) {
        message.validate(descriptor)?;
        bytes += message.content.len();
    }
    if bytes > 256 * 1024 {
        return Err(ExternalSourceError::Bounds);
    }
    // Legacy internal field node_id is a transient source anchor here. It is
    // never persisted as an Archive ID or carried into a MemoryDraft.
    let turns = hydrated
        .iter()
        .map(|m| crate::ResolvedTurn {
            node_id: m.descriptor.reference.message_id.clone(),
            role: m.descriptor.role.clone(),
            principal_id: m.descriptor.principal_id.clone(),
            timestamp_ns: m.descriptor.timestamp_ns,
            content: m.content.clone(),
        })
        .collect::<Vec<_>>();
    let extraction = extractor
        .extract_external_input(window, &state.namespace, &refs[0].conversation_id, &turns)
        .map_err(|_| ExternalSourceError::Unavailable)?;
    let mut project = Vec::new();
    let mut user = Vec::new();
    let inferencer = crate::TemporalInferencer::new(extractor.temporal_endpoint());
    let through = expected.iter().map(|d| d.timestamp_ns).max().unwrap();
    for candidate in extraction.candidates {
        let authority = source_processing::evidence_chunk(
            &hydrated,
            &candidate.source_node_id,
            &candidate.source_quote,
        )?;
        let anchor = hydrated
            .iter()
            .find(|m| m.descriptor.reference == authority.message)
            .unwrap();
        if anchor.descriptor.role != "user" {
            return Err(ExternalSourceError::InvalidContract);
        }
        if candidate.ownership == crate::InsomniaOwnership::User
            && anchor.descriptor.principal_id.as_deref() != Some(principal)
        {
            return Err(ExternalSourceError::Unauthorized);
        }
        let content = source_processing::optional_chunk(
            &hydrated,
            candidate.authority_source_node_id.as_deref(),
            candidate.authority_source_quote.as_deref(),
        )?;
        let grounding = source_processing::optional_chunk(
            &hydrated,
            candidate.grounding_source_node_id.as_deref(),
            candidate.grounding_source_quote.as_deref(),
        )?;
        for conversation in [
            candidate.authority_source_conversation_id.as_deref(),
            candidate.grounding_source_conversation_id.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if conversation != refs[0].conversation_id {
                return Err(ExternalSourceError::InvalidContract);
            }
        }
        let source_time = content
            .as_ref()
            .map_or(anchor.descriptor.timestamp_ns, |c| {
                hydrated
                    .iter()
                    .find(|m| m.descriptor.reference == c.message)
                    .unwrap()
                    .descriptor
                    .timestamp_ns
            });
        let draft = MemoryDraft {
            category: candidate.category,
            memory_type: candidate.memory_type,
            authority_kind: candidate.authority_kind,
            temporal_status: candidate.temporal_status,
            title: candidate.title,
            content: candidate.content,
            scope: if candidate.ownership == crate::InsomniaOwnership::User {
                "private".to_owned()
            } else {
                scope.to_owned()
            },
            lifecycle_state: "extracted".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_time_ns: Some(source_time),
            mutation_id: format!("external-insomnia:{}:{}", hex(&window), candidate.key),
            created_at_ns: through,
            updated_at_ns: now_ns.max(through),
        };
        let routing = candidate.routing_metadata.map(|r| MemoryRoutingMetadata {
            memory_id: crate::memory_model::memory_id(&draft.mutation_id),
            body_id: crate::memory_model::memory_body_id(&draft.title, &draft.content),
            entity_mentions: r.entity_mentions,
        });
        let mut prepared = crate::insomnia::PreparedMemory {
            temporal: crate::insomnia::temporal::assess_draft(&draft),
            draft,
            temporal_inference: None,
            routing_metadata: routing,
        };
        crate::insomnia::temporal::infer_memory(&inferencer, &mut prepared)
            .map_err(|_| ExternalSourceError::Unavailable)?;
        let temporal = prepared.bound_temporal_inference();
        let evidence = ExternalMemoryEvidence {
            source_owner_id: owner.to_owned(),
            principal_id: anchor
                .descriptor
                .principal_id
                .clone()
                .unwrap_or_else(|| principal.to_owned()),
            processing_window: window,
            authority,
            content,
            grounding,
        };
        evidence.validate()?;
        let value = PreparedExternalMemory {
            draft: prepared.draft,
            temporal,
            routing: prepared.routing_metadata,
            evidence,
        };
        match candidate.ownership {
            crate::InsomniaOwnership::Project => project.push(value),
            crate::InsomniaOwnership::User => user.push(value),
        }
    }
    Ok(PreparedExternalProcessing {
        window,
        expected: state,
        project,
        user,
        rejected: extraction.rejected.len(),
    })
}

use super::mutation_events::RelMutationFamily;
use super::{GatewayRuntime, InstanceRuntime, OwnerGrant, operation};
use crate::external_source::{self as external, *};

impl GatewayRuntime {
    pub fn hydrate_external_chunks_for(
        &self,
        instance: &InstanceRuntime,
        owner: &str,
        source: &impl ExternalConversationSource,
        refs: &[ExternalChunkRef],
    ) -> Result<Vec<HydratedExternalChunk>, ExternalSourceError> {
        if refs.is_empty() || refs.len() > MAX_EXTERNAL_BATCH {
            return Err(ExternalSourceError::Bounds);
        }
        source.authorize(instance.principal_id())?;
        let (token, state, _) =
            self.external_read_state(instance, owner, source.source_namespace())?;
        let mut descriptors = Vec::new();
        for reference in refs {
            reference.message.validate()?;
            if reference.start >= reference.end || reference.end > MAX_EXTERNAL_MESSAGE_BYTES as u64
            {
                return Err(ExternalSourceError::Bounds);
            }
            descriptors.push(
                state
                    .messages
                    .iter()
                    .find(|m| m.descriptor.active && m.descriptor.reference == reference.message)
                    .ok_or(ExternalSourceError::StaleEvidence)?
                    .descriptor
                    .clone(),
            );
        }
        let bodies = source.hydrate(instance.principal_id(), &descriptors)?;
        if bodies.len() != refs.len() {
            return Err(ExternalSourceError::InvalidContract);
        }
        let mut result = Vec::new();
        for ((body, expected), reference) in bodies.iter().zip(&descriptors).zip(refs) {
            body.validate(expected)?;
            let content = body
                .content
                .get(reference.start as usize..reference.end as usize)
                .ok_or(ExternalSourceError::StaleEvidence)?
                .to_owned();
            result.push(HydratedExternalChunk {
                reference: reference.clone(),
                content,
            });
        }
        source.authorize(instance.principal_id())?;
        self.with_rel(&token, |r| {
            if r.cva.external_index.sources.get(&state.namespace) != Some(&state) {
                return Err(operation(ExternalSourceError::StaleEvidence));
            }
            Ok(result)
        })
        .map_err(|_| ExternalSourceError::StaleEvidence)
    }

    pub fn process_external_window_for<E: crate::GeneralEndpoint>(
        &self,
        instance: &InstanceRuntime,
        owner: &str,
        scope: &str,
        source: &impl ExternalConversationSource,
        refs: &[ExternalMessageRef],
        extractor: &crate::InsomniaExtractor<E>,
        now_ns: i64,
    ) -> Result<ExternalProcessingResult, ExternalSourceError> {
        let token = self
            .authorize(instance, owner, OwnerGrant::ManageRel)
            .map_err(|_| ExternalSourceError::Unauthorized)?;
        source.authorize(instance.principal_id())?;
        let namespace = source.source_namespace().to_owned();
        crate::external_index::model::identifier(&namespace)?;
        let state = self
            .with_rel(&token, |r| {
                r.cva
                    .external_index
                    .read_state(&namespace)
                    .map_err(operation)?
                    .ok_or_else(|| operation(ExternalSourceError::RebuildRequired))
            })
            .map_err(|_| ExternalSourceError::RebuildRequired)?;
        let descriptors = external::processing::descriptors(&state, refs)?;
        super::external::validate_external_evidence(source, instance.principal_id(), &state, refs)?;
        crate::external_index::model::identifier(scope)?;
        let window = external::processing::window_key(owner, scope, &descriptors);
        let phy = instance
            .attached_phy_owner_id()
            .map(|id| self.authorize(instance, id, OwnerGrant::UseOwnPhy))
            .transpose()
            .map_err(|_| ExternalSourceError::Unauthorized)?;
        let receipt = self
            .with_rel(&token, |r| {
                r.cva
                    .owner
                    .memories
                    .external_window(&mut r.cva.owner.container, window)
                    .map_err(operation)
            })
            .map_err(|_| ExternalSourceError::Storage)?;
        if let Some(memories) = receipt {
            let plan_path = super::external_plan::path(&self.state_dir, owner, window);
            let plan = super::external_plan::ExternalWindowPlan::load(&plan_path)?
                .ok_or(ExternalSourceError::Storage)?;

            let private = match &phy {
                Some(p) => self
                    .with_phy(p, |p| {
                        p.owner
                            .memories
                            .external_window(&mut p.owner.container, window)
                            .map_err(operation)
                    })
                    .map_err(|_| ExternalSourceError::Unauthorized)?
                    .unwrap_or_default(),
                None => Vec::new(),
            };
            plan.verify_receipts(
                window,
                instance.principal_id(),
                phy.as_ref().map(|p| p.owner_id()),
                &memories,
                &private,
            )?;
            source.authorize(instance.principal_id())?;
            if let Some(p) = &phy {
                self.with_phy(p, |_| Ok(()))
                    .map_err(|_| ExternalSourceError::Unauthorized)?;
            }
            return self
                .with_rel(&token, |_| {
                    Ok(ExternalProcessingResult {
                        project: memory_refs(owner, &memories),
                        user: phy
                            .as_ref()
                            .map_or_else(Vec::new, |p| memory_refs(p.owner_id(), &private)),
                        rejected: None,
                        replayed: true,
                    })
                })
                .map_err(|_| ExternalSourceError::Unauthorized);
        }
        let plan_path = super::external_plan::path(&self.state_dir, owner, window);
        let destination = phy.as_ref().map(|p| p.owner_id());
        let candidate = match super::external_plan::ExternalWindowPlan::load(&plan_path)? {
            Some(plan) => plan.thaw(window, instance.principal_id(), destination, state.clone())?,
            None => prepare(
                owner,
                instance.principal_id(),
                scope,
                state.clone(),
                refs,
                source,
                extractor,
                now_ns,
            )?,
        };
        if !candidate.user.is_empty() && phy.is_none() {
            return Err(ExternalSourceError::Unauthorized);
        }
        source.authorize(instance.principal_id())?;
        let frozen = super::external_plan::ExternalWindowPlan::freeze(
            &candidate,
            instance.principal_id(),
            destination,
        )?;
        let prepared = self
            .with_rel_distribution_admission(&token, |sequencer| {
                let _gate = sequencer.gate(token.mount_epoch)?;
                self.with_rel(&token, |r| {
                    if r.cva
                        .external_index
                        .sources
                        .get(&candidate.expected.namespace)
                        != Some(&candidate.expected)
                    {
                        return Err(operation(ExternalSourceError::Conflict));
                    }
                    Ok(())
                })?;
                frozen.install(&plan_path).map_err(operation)?;
                super::external_plan::ExternalWindowPlan::load(&plan_path)
                    .map_err(operation)?
                    .ok_or_else(|| operation(ExternalSourceError::Storage))?
                    .thaw(window, instance.principal_id(), destination, state)
                    .map_err(operation)
            })
            .map_err(|_| ExternalSourceError::Conflict)?;
        super::external::validate_external_evidence(
            source,
            instance.principal_id(),
            &prepared.expected,
            refs,
        )?;
        let users = self
            .with_rel_distribution_admission(&token, |sequencer| {
                let _gate = sequencer.gate(token.mount_epoch)?;
                self.with_rel(&token, |r| {
                    if r.cva
                        .external_index
                        .sources
                        .get(&prepared.expected.namespace)
                        != Some(&prepared.expected)
                    {
                        return Err(operation(ExternalSourceError::Conflict));
                    }
                    Ok(())
                })?;
                match &phy {
                    Some(p) => self.with_phy(p, |p| {
                        self.validate_rel_distribution_token(&token)?;
                        external::publication::publish(
                            &mut p.owner.container,
                            &mut p.owner.memories,
                            window,
                            &prepared.user,
                            None,
                        )
                        .map_err(operation)
                    }),
                    None => Ok(Vec::new()),
                }
            })
            .map_err(|_| ExternalSourceError::Conflict)?;
        let projects = self
            .with_rel_mutation(&token, RelMutationFamily::Semantic, |r| {
                if r.cva
                    .external_index
                    .sources
                    .get(&prepared.expected.namespace)
                    != Some(&prepared.expected)
                {
                    return Err(operation(ExternalSourceError::Conflict));
                }
                let owner_uuid = r.cva.owner.container.owner_uuid();
                let turn = r.cva.owner.archive.rel_turn_count();
                let memories = crate::external_owner_access::publish_window(
                    &mut r.cva.owner.container,
                    &mut r.cva.owner.memories,
                    &mut r.cva.owner.freshness,
                    &mut r.cva.owner.dream_freshness,
                    owner_uuid,
                    turn,
                    window,
                    &prepared.project,
                )
                .map_err(operation)?;
                Ok((memories, true, Vec::new()))
            })
            .map_err(|_| ExternalSourceError::Conflict)?;
        let _ = self.wake_rel(owner);
        if let Some(p) = &phy {
            let _ = self.wake_phy(p.owner_id());
        }
        source.authorize(instance.principal_id())?;
        if let Some(p) = &phy {
            self.with_phy(p, |_| Ok(()))
                .map_err(|_| ExternalSourceError::Unauthorized)?;
        }
        self.with_rel(&token, |_| {
            Ok(ExternalProcessingResult {
                project: memory_refs(owner, &projects),
                user: phy
                    .as_ref()
                    .map_or_else(Vec::new, |p| memory_refs(p.owner_id(), &users)),
                rejected: Some(prepared.rejected),
                replayed: false,
            })
        })
        .map_err(|_| ExternalSourceError::Unauthorized)
    }
}
fn memory_refs(owner: &str, memories: &[crate::Memory]) -> Vec<crate::MemoryRef> {
    memories
        .iter()
        .map(|m| crate::MemoryRef {
            owner_id: owner.to_owned(),
            memory_id: m.id,
        })
        .collect()
}
