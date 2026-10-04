//! REL-owned recoverable publication through the canonical gateway PHY registry.
use super::owner_operations::*;
use super::publication_codec::{freeze_memory, thaw_memory, verify_memory, verify_memory_record};
use super::*;
use crate::insomnia::{
    PreparedApplication, PreparedUserMemory, RuntimeInsomniaClaim, UserPublication,
};
use crate::{InsomniaRejection, MemoryRef, MemorySourceRef};
use sha2::{Digest, Sha256};

pub(super) struct PublicationCoordinator {
    pub(super) rel_id: String,
    pub(super) journal: Arc<Mutex<OwnerOperationJournal>>,
    pub(super) phys: Arc<RwLock<BTreeMap<DurableOwnerId, PhyExecution>>>,
    pub(super) bindings: Arc<Mutex<BTreeMap<String, String>>>,
    pub(super) policy: Arc<RwLock<()>>,
    pub(super) rel_fence: Arc<LeaseGate>,
}

pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn source_value(source: &MemorySourceRef) -> serde_json::Value {
    serde_json::json!({
        "owner_id": source.owner_id, "principal_id": source.principal_id,
        "source_episode_id": hex(&source.source_episode_id.0),
        "source_node_id": source.source_node_id,
        "content_source_conversation_id": source.content_source_conversation_id,
        "content_source_node_id": source.content_source_node_id,
        "grounding_source_conversation_id": source.grounding_source_conversation_id,
        "grounding_source_node_id": source.grounding_source_node_id,
    })
}
fn fingerprint(claim: &RuntimeInsomniaClaim) -> Result<String, ReliquaryRuntimeHostError> {
    let turns = claim
        .turns
        .iter()
        .map(|turn| {
            serde_json::json!([
                turn.node_id,
                turn.role,
                turn.principal_id,
                turn.timestamp_ns,
                turn.content
            ])
        })
        .collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&serde_json::json!([
        hex(&claim.episode.id.0),
        claim.episode.conversation_id,
        turns
    ]))
    .map_err(operation)?;
    Ok(hex(&Sha256::digest(bytes)))
}
fn receipt_id(
    episode: &str,
    model: &str,
    project: &[String],
    external: &[OwnerMemoryRef],
    rejected: usize,
) -> Result<String, ReliquaryRuntimeHostError> {
    let bytes = serde_json::to_vec(&serde_json::json!([
        "RQ-EPISODE-RECEIPT-V1",
        episode,
        model,
        crate::INSOMNIA_EXTRACTOR_CONTRACT_VERSION,
        project,
        external,
        rejected
    ]))
    .map_err(operation)?;
    Ok(hex(&Sha256::digest(bytes)))
}

impl PublicationCoordinator {
    pub(super) fn plan(
        &self,
        claim: &RuntimeInsomniaClaim,
    ) -> Result<Option<EpisodePlan>, ReliquaryRuntimeHostError> {
        let journal = self
            .journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let plan = journal.episode_plan(&hex(&claim.episode.id.0)).cloned();
        if plan.is_none()
            && claim.turns.iter().any(|turn| {
                turn.role == "user"
                    && turn.principal_id.is_some()
                    && journal
                        .source_attribution(
                            &claim.episode.conversation_id,
                            &turn.node_id,
                            ActorClass::User,
                        )
                        .is_none()
            })
        {
            return Err(operation(
                "episode lacks durable source authority; operational-state repair required",
            ));
        }
        if plan
            .as_ref()
            .is_some_and(|plan| fingerprint(claim).map_or(true, |fp| fp != plan.source_fingerprint))
        {
            return Err(operation("frozen episode source fingerprint conflicts"));
        }
        Ok(plan)
    }

    /// Only durable explicit source attribution is routing authority.
    fn source_route(
        &self,
        cva: &crate::Cva,
        conversation: &str,
        source: &MemorySourceRef,
    ) -> Result<(String, String, Vec<String>), ReliquaryRuntimeHostError> {
        if source.owner_id != self.rel_id {
            return Err(operation("personal provenance has another source REL"));
        }
        let mut coordinates = vec![(conversation.to_owned(), source.source_node_id.clone())];
        for (conversation, node) in [
            (
                &source.content_source_conversation_id,
                &source.content_source_node_id,
            ),
            (
                &source.grounding_source_conversation_id,
                &source.grounding_source_node_id,
            ),
        ] {
            match (conversation, node) {
                (Some(c), Some(n)) => coordinates.push((c.clone(), n.clone())),
                (None, None) => {}
                _ => return Err(operation("incomplete personal source coordinates")),
            }
        }
        let journal = self
            .journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let mut principal = None::<String>;
        let mut destination = None::<String>;
        let mut user_supported = false;
        let mut authorizing_generations = BTreeSet::new();
        for (conversation, node_id) in coordinates {
            let node = cva
                .archive
                .nodes
                .get(&conversation, &node_id)
                .ok_or_else(|| operation("personal source is absent"))?;
            let actor = match node.role.as_str() {
                "user" => ActorClass::User,
                "assistant" | "agent" => ActorClass::Agent,
                _ => {
                    return Err(operation(
                        "service or unknown source cannot authorize personal memory",
                    ));
                }
            };
            let attribution = journal
                .source_attribution(&conversation, &node_id, actor)
                .ok_or_else(|| operation("personal source attribution is unknown"))?;
            if attribution.admission.personal.is_none() {
                return Err(operation("source lacks captured UseOwnPhy authority"));
            }
            if journal.authority_revoked(attribution) {
                return Err(operation("captured personal source authority was revoked"));
            }
            authorizing_generations.insert(
                attribution
                    .initiating_generation_id
                    .clone()
                    .unwrap_or_else(|| attribution.generation_id.clone()),
            );
            let resolved = match actor {
                ActorClass::User => {
                    user_supported = true;
                    if node.principal_id != attribution.actor_principal {
                        return Err(operation("archive actor differs from durable attribution"));
                    }
                    attribution.actor_principal.as_ref()
                }
                ActorClass::Agent => attribution.generation_initiator.as_ref(),
                ActorClass::Service => None,
            }
            .ok_or_else(|| operation("personal source principal is unknown"))?;
            if principal.as_ref().is_some_and(|value| value != resolved) {
                return Err(operation("personal evidence crosses principals"));
            }
            let phy = attribution
                .admission
                .phy_owner_id
                .as_ref()
                .ok_or_else(|| operation("personal source has no verified PHY destination"))?;
            if destination.as_ref().is_some_and(|value| value != phy) {
                return Err(operation(
                    "personal source has conflicting exact destinations",
                ));
            }
            principal = Some(resolved.clone());
            destination = Some(phy.clone());
        }
        if !user_supported {
            return Err(operation("personal memory needs an authored User source"));
        }
        let principal =
            principal.ok_or_else(|| operation("personal source principal is unknown"))?;
        let phy = destination.ok_or_else(|| operation("personal destination is unknown"))?;
        validate_owner_id(&phy, "phy-")?;
        let bindings = self
            .bindings
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if bindings.get(&principal) != Some(&phy) {
            return Err(operation(
                "verified principal binding is unavailable or changed",
            ));
        }
        Ok((
            principal,
            phy,
            authorizing_generations.into_iter().collect(),
        ))
    }

    pub(super) fn freeze(
        &self,
        runtime: &mut InteractionRuntime,
        claim: &RuntimeInsomniaClaim,
        prepared: PreparedApplication,
    ) -> Result<EpisodePlan, ReliquaryRuntimeHostError> {
        let _policy = self
            .policy
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if let Some(existing) = self.plan(claim)? {
            return Ok(existing);
        }
        let mut frozen = FrozenPreparedApplication {
            project_drafts: Vec::new(),
            personal_drafts: Vec::new(),
            rejected: prepared
                .rejected
                .into_iter()
                .map(|r| FrozenRejection {
                    candidate_key: r.candidate_key,
                    reason: r.reason,
                })
                .collect(),
            model: prepared.model,
        };
        for memory in prepared.project_drafts {
            frozen.project_drafts.push(freeze_memory(&memory, None)?);
        }
        for mut personal in prepared.user_drafts {
            match self.source_route(
                &runtime.cva,
                &claim.episode.conversation_id,
                &personal.source_ref,
            ) {
                Ok((principal, phy, authorizing_generations)) => {
                    personal.source_ref.principal_id = Some(principal.clone());
                    frozen.personal_drafts.push(FrozenPersonalDraft {
                        prepared_memory: freeze_memory(
                            &personal.memory,
                            Some(&personal.source_ref),
                        )?,
                        source_ref: source_value(&personal.source_ref),
                        mutation_id: personal.memory.draft.mutation_id.clone(),
                        destination_principal: principal,
                        destination_phy_owner_id: phy,
                        authorizing_generations,
                    });
                }
                Err(error) => frozen.rejected.push(FrozenRejection {
                    candidate_key: Some(personal.memory.draft.mutation_id),
                    reason: format!("personal candidate quarantined: {error}"),
                }),
            }
        }
        let episode_id = hex(&claim.episode.id.0);
        let projects = frozen
            .project_drafts
            .iter()
            .map(|memory| {
                thaw_memory(memory)
                    .map(|(m, _)| hex(&crate::memory_model::memory_id(&m.draft.mutation_id).0))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let refs = Self::external_refs(&frozen);
        let plan = EpisodePlan {
            episode_id: episode_id.clone(),
            attempt: claim.work.attempt_count,
            source_fingerprint: fingerprint(claim)?,
            expected_receipt_id: receipt_id(
                &episode_id,
                &frozen.model,
                &projects,
                &refs,
                frozen.rejected.len(),
            )?,
            prepared: frozen,
        };
        self.journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .append_episode_plan(plan.clone())
            .map_err(operation)?;
        Ok(plan)
    }

    fn external_refs(prepared: &FrozenPreparedApplication) -> Vec<OwnerMemoryRef> {
        prepared
            .personal_drafts
            .iter()
            .map(|draft| OwnerMemoryRef {
                owner_id: draft.destination_phy_owner_id.clone(),
                memory_id: hex(&crate::memory_model::memory_id(&draft.mutation_id).0),
            })
            .collect()
    }

    pub(super) fn thaw(
        &self,
        plan: &EpisodePlan,
    ) -> Result<PreparedApplication, ReliquaryRuntimeHostError> {
        let mut prepared = PreparedApplication {
            project_drafts: Vec::new(),
            user_drafts: Vec::new(),
            rejected: plan
                .prepared
                .rejected
                .iter()
                .map(|r| InsomniaRejection {
                    candidate_key: r.candidate_key.clone(),
                    reason: r.reason.clone(),
                })
                .collect(),
            model: plan.prepared.model.clone(),
        };
        for frozen in &plan.prepared.project_drafts {
            let (memory, source) = thaw_memory(frozen)?;
            if source.is_some() {
                return Err(operation(
                    "project plan contains external personal provenance",
                ));
            }
            prepared.project_drafts.push(memory);
        }
        for frozen in &plan.prepared.personal_drafts {
            let (memory, source) = thaw_memory(&frozen.prepared_memory)?;
            let source = source
                .ok_or_else(|| operation("personal frozen payload lacks source provenance"))?;
            if source_value(&source) != frozen.source_ref
                || memory.draft.mutation_id != frozen.mutation_id
                || source.principal_id.as_ref() != Some(&frozen.destination_principal)
            {
                return Err(operation("frozen personal route conflicts with payload"));
            }
            prepared.user_drafts.push(PreparedUserMemory {
                memory,
                source_ref: source,
            });
        }
        Ok(prepared)
    }

    /// Called with no REL store lock. Each PHY mutation is separately fenced/synced.
    pub(super) fn publish(
        &self,
        plan: &EpisodePlan,
        allow_write: bool,
    ) -> Result<UserPublication, ReliquaryRuntimeHostError> {
        let mut result = UserPublication {
            created: Vec::new(),
            existing: Vec::new(),
            refs: Vec::new(),
        };
        for frozen in &plan.prepared.personal_drafts {
            let _rel_lease = self.rel_fence.acquire_current()?;
            let _policy = self
                .policy
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let (memory, source) = thaw_memory(&frozen.prepared_memory)?;
            let source =
                source.ok_or_else(|| operation("personal plan lacks source provenance"))?;
            if source_value(&source) != frozen.source_ref
                || memory.draft.mutation_id != frozen.mutation_id
                || source.principal_id.as_ref() != Some(&frozen.destination_principal)
            {
                return Err(operation("personal frozen route/payload mismatch"));
            }
            let bindings = self
                .bindings
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            if bindings.get(&frozen.destination_principal) != Some(&frozen.destination_phy_owner_id)
            {
                return Err(operation(
                    "frozen personal destination cannot be reauthorized",
                ));
            }
            drop(bindings);
            // Frozen intent names the exact original User admissions. Missing
            // legacy authority stays Held; retries never consult active instances.
            {
                let journal = self
                    .journal
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                if frozen.authorizing_generations.is_empty() {
                    return Err(operation("frozen plan lacks captured personal authority"));
                }
                for id in &frozen.authorizing_generations {
                    let admission = journal
                        .attribution(id)
                        .ok_or_else(|| operation("frozen admission is missing"))?;
                    if admission.actor_class != ActorClass::User
                        || admission.admission.personal.is_none()
                        || admission.actor_principal.as_ref() != Some(&frozen.destination_principal)
                        || admission.admission.phy_owner_id.as_ref()
                            != Some(&frozen.destination_phy_owner_id)
                        || (allow_write && journal.authority_revoked(admission))
                    {
                        return Err(operation(
                            "frozen personal authority is missing, conflicting or revoked",
                        ));
                    }
                }
            }
            let (owner, fence, runtime) = {
                let phys = self
                    .phys
                    .read()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                let execution = phys
                    .get(&frozen.destination_phy_owner_id)
                    .ok_or_else(|| operation("frozen PHY target is offline"))?;
                (
                    Arc::clone(&execution.owner),
                    Arc::clone(&execution.fence),
                    Arc::clone(&execution.runtime),
                )
            };
            let _phy_lease = fence.acquire_current()?;
            let mut owner = owner
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            if owner.owner_id().as_ref() != Some(&frozen.destination_phy_owner_id) {
                return Err(operation("mounted target durable identity changed"));
            }
            let id = crate::memory_model::memory_id(&frozen.mutation_id);
            let mut created = false;
            if !owner.memories.contains_memory(id) {
                if !allow_write {
                    return Err(operation(
                        "completed REL receipt references missing PHY memory",
                    ));
                }
                let inference = memory.bound_temporal_inference();
                owner
                    .publish_memory_with_source_ref_and_temporal_inference(
                        None,
                        0,
                        memory.draft.clone(),
                        source,
                        inference,
                    )
                    .map_err(operation)?;
                created = true;
            }
            let original = owner.memory_revision(id, 1).map_err(operation)?;
            verify_memory_record(&frozen.prepared_memory, &original)?;
            if let Some(metadata) = memory.routing_metadata {
                match owner.memory_routing_metadata(id) {
                    Some(existing) if existing != &metadata => {
                        return Err(operation("same-ID personal routing metadata conflicts"));
                    }
                    None if allow_write => {
                        owner
                            .put_memory_routing_metadata(metadata)
                            .map_err(operation)?;
                    }
                    None => return Err(operation("completed PHY routing attachment is missing")),
                    _ => {}
                }
            } else if owner.memory_routing_metadata(id).is_some() {
                return Err(operation("unexpected personal routing metadata conflicts"));
            }
            owner.sync().map_err(operation)?;
            let verified = owner.memory_revision(id, 1).map_err(operation)?;
            verify_memory(&frozen.prepared_memory, &verified)?;
            drop(owner);
            if created {
                result.created.push(verified);
            } else {
                result.existing.push(verified);
            }
            result.refs.push(MemoryRef {
                owner_id: frozen.destination_phy_owner_id.clone(),
                memory_id: id,
            });
            runtime
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .wake()?;
        }
        Ok(result)
    }

    pub(super) fn verify_receipt(
        &self,
        runtime: &mut InteractionRuntime,
        plan: &EpisodePlan,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        let episode = parse_episode(&plan.episode_id)?;
        let attempts = runtime.cva.insomnia_attempts(episode);
        let Some(receipt) = attempts
            .iter()
            .find(|a| a.state == crate::InsomniaWorkState::Complete)
        else {
            return Ok(false);
        };
        let project = receipt
            .memory_ids
            .iter()
            .map(|id| hex(&id.0))
            .collect::<Vec<_>>();
        let refs = receipt
            .external_memory_refs
            .iter()
            .map(|r| OwnerMemoryRef {
                owner_id: r.owner_id.clone(),
                memory_id: hex(&r.memory_id.0),
            })
            .collect::<Vec<_>>();
        let identity = receipt_id(
            &plan.episode_id,
            &receipt.extractor_model,
            &project,
            &refs,
            receipt.rejected_count as usize,
        )?;
        if identity != plan.expected_receipt_id
            || receipt.extractor_version != crate::INSOMNIA_EXTRACTOR_CONTRACT_VERSION
            || refs != Self::external_refs(&plan.prepared)
        {
            return Err(operation("REL completion receipt differs from frozen plan"));
        }
        for frozen in &plan.prepared.project_drafts {
            let (memory, _) = thaw_memory(frozen)?;
            let id = crate::memory_model::memory_id(&memory.draft.mutation_id);
            let original = runtime.cva.memory_revision(id, 1).map_err(operation)?;
            verify_memory(frozen, &original)?;
        }
        Ok(true)
    }

    pub(super) fn observe(&self, plan: &EpisodePlan) -> Result<(), ReliquaryRuntimeHostError> {
        self.journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .append_receipt_observed(EpisodeReceiptObserved {
                episode_id: plan.episode_id.clone(),
                receipt_id: plan.expected_receipt_id.clone(),
                verified_external_refs: Self::external_refs(&plan.prepared),
            })
            .map_err(operation)?;
        Ok(())
    }
    pub(super) fn hold(
        &self,
        plan: &EpisodePlan,
        error: &ReliquaryRuntimeHostError,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.hold_operation(&plan.episode_id, error)
    }

    pub(super) fn hold_claim(
        &self,
        claim: &RuntimeInsomniaClaim,
        error: &ReliquaryRuntimeHostError,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.hold_operation(&hex(&claim.episode.id.0), error)
    }

    fn hold_operation(
        &self,
        operation_id: &str,
        error: &ReliquaryRuntimeHostError,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let mut reason = error.to_string();
        while reason.len() > 4096 {
            reason.pop();
        }
        let mut journal = self
            .journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if journal.records().iter().rev().any(|r| matches!(&r.record, OwnerOperationRecord::Held(h) if h.operation_id == operation_id && h.reason == reason)) { return Ok(()); }
        journal
            .append_held(Held {
                operation_id: operation_id.to_owned(),
                reason,
            })
            .map_err(operation)?;
        Ok(())
    }
    pub(super) fn unobserved(&self) -> Result<Vec<EpisodePlan>, ReliquaryRuntimeHostError> {
        let journal = self
            .journal
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        Ok(journal.records().iter().filter_map(|record| match &record.record {
            OwnerOperationRecord::EpisodePlan(plan) if !journal.records().iter().any(|r| matches!(&r.record, OwnerOperationRecord::EpisodeReceiptObserved(receipt) if receipt.episode_id == plan.episode_id)) => Some(plan.clone()),
            _ => None,
        }).collect())
    }
}
fn parse_episode(value: &str) -> Result<crate::EpisodeId, ReliquaryRuntimeHostError> {
    if value.len() != 64 {
        return Err(operation("invalid frozen episode identity"));
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(operation)?;
    }
    Ok(crate::EpisodeId(bytes))
}
