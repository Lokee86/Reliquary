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
        external::model::identifier(&namespace)?;
        let state = self
            .with_rel(&token, |r| {
                r.cva
                    .external_index_state(&namespace)
                    .map_err(operation)?
                    .ok_or_else(|| operation(ExternalSourceError::RebuildRequired))
            })
            .map_err(|_| ExternalSourceError::RebuildRequired)?;
        let descriptors = external::processing::descriptors(&state, refs)?;
        super::external::validate_external_evidence(source, instance.principal_id(), &state, refs)?;
        external::model::identifier(scope)?;
        let window = external::processing::window_key(owner, scope, &descriptors);
        let phy = instance
            .attached_phy_owner_id()
            .map(|id| self.authorize(instance, id, OwnerGrant::UseOwnPhy))
            .transpose()
            .map_err(|_| ExternalSourceError::Unauthorized)?;
        let receipt = self
            .with_rel(&token, |r| {
                r.cva
                    .memories
                    .external_window(&mut r.cva.container, window)
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
                        p.memories
                            .external_window(&mut p.container, window)
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
            None => external::processing::prepare(
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
                            &mut p.container,
                            &mut p.memories,
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
                let memories = r
                    .cva
                    .publish_external_window(window, &prepared.project)
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
