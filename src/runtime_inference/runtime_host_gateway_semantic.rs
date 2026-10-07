//! Instance-authorized owner lanes. Private data never enters REL-only read APIs.
use super::*;
use crate::{RuntimeSemanticOwner, RuntimeSemanticOwnerKind};

#[path = "runtime_host_gateway_semantic_mutation.rs"]
mod mutation;
#[path = "runtime_host_gateway_semantic_provenance.rs"]
mod provenance;
pub use provenance::RuntimeKnowledgeProvenanceResolution;

impl GatewayRuntime {
    fn semantic_token(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<OwnerAccessToken, ReliquaryRuntimeHostError> {
        // Check the instance capability before consulting mount topology.
        self.authorize(
            instance,
            owner_id,
            if owner_id.starts_with("phy-") {
                OwnerGrant::UseOwnPhy
            } else {
                OwnerGrant::ViewRel
            },
        )
    }

    pub fn visible_semantic_owners(
        &self,
        instance: &InstanceRuntime,
    ) -> Result<Vec<RuntimeSemanticOwner>, ReliquaryRuntimeHostError> {
        let authorization = instance.authorization().map_err(operation)?;
        let mut owners = match self.active_rel_id(instance)? {
            Some(rel) => self
                .dependency_closure_for(instance, &rel)?
                .into_iter()
                .map(|owner_id| RuntimeSemanticOwner {
                    owner_id,
                    kind: RuntimeSemanticOwnerKind::Reliquary,
                })
                .collect::<Vec<_>>(),
            None => Vec::new(),
        };
        let grants = instance.grants_snapshot().map_err(operation)?;
        if let Some(phy) = instance.attached_phy_owner_id().filter(|phy| {
            grants
                .get(*phy)
                .is_some_and(|caps| caps.contains(&OwnerGrant::UseOwnPhy))
        }) {
            let token = self.authorize(instance, phy, OwnerGrant::UseOwnPhy)?;
            self.with_phy(&token, |_| Ok(()))?;
            owners.push(RuntimeSemanticOwner {
                owner_id: phy.into(),
                kind: RuntimeSemanticOwnerKind::Phylactery,
            });
        }
        let attachment = self.attachment_entry(instance)?;
        let _delivery = attachment.leases.acquire_current()?;
        if instance.authorization().map_err(operation)? != authorization {
            return Err(operation(
                "instance authorization changed before semantic delivery",
            ));
        }
        Ok(owners)
    }

    pub fn semantic_owner(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<RuntimeSemanticOwner, ReliquaryRuntimeHostError> {
        let token = self.semantic_token(instance, owner_id)?;
        match token.kind {
            OwnerKind::Rel => self.with_rel(&token, |_| {
                Ok(RuntimeSemanticOwner {
                    owner_id: owner_id.into(),
                    kind: RuntimeSemanticOwnerKind::Reliquary,
                })
            }),
            OwnerKind::Phy => self.with_phy(&token, |_| {
                Ok(RuntimeSemanticOwner {
                    owner_id: owner_id.into(),
                    kind: RuntimeSemanticOwnerKind::Phylactery,
                })
            }),
        }
    }

    pub fn knowledge_memory_for_owner(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        memory_id: crate::MemoryId,
    ) -> Result<crate::Memory, ReliquaryRuntimeHostError> {
        let token = self.semantic_token(instance, owner_id)?;
        match token.kind {
            OwnerKind::Rel => self.with_rel(&token, |runtime| {
                runtime.cva.memory(memory_id).map_err(operation)
            }),
            OwnerKind::Phy => {
                self.with_phy(&token, |owner| owner.memory(memory_id).map_err(operation))
            }
        }
    }

    pub fn read_knowledge_for_owner(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<crate::RuntimeKnowledgeState, ReliquaryRuntimeHostError> {
        let token = self.semantic_token(instance, owner_id)?;
        match token.kind {
            OwnerKind::Rel => self.read_reliquary_knowledge_for(instance, owner_id),
            OwnerKind::Phy => self.with_phy(&token, |owner| {
                let memories = owner
                    .memory_ids()
                    .into_iter()
                    .map(|id| owner.memory(id).map_err(operation))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((
                    memories,
                    owner.entities(),
                    owner.semantic_graph_relations(),
                    owner.community_snapshot(),
                    owner.community_semantic_names(),
                ))
            }),
        }
    }

    pub fn entity_stats_for_owner(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<crate::EntityStats, ReliquaryRuntimeHostError> {
        let token = self.semantic_token(instance, owner_id)?;
        match token.kind {
            OwnerKind::Rel => self.with_rel(&token, |runtime| Ok(runtime.cva.entity_stats())),
            OwnerKind::Phy => self.with_phy(&token, |owner| Ok(owner.entity_stats())),
        }
    }

    pub fn entities_for_owner(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<Vec<crate::Entity>, ReliquaryRuntimeHostError> {
        let token = self.semantic_token(instance, owner_id)?;
        match token.kind {
            OwnerKind::Rel => self.with_rel(&token, |runtime| Ok(runtime.cva.entities())),
            OwnerKind::Phy => self.with_phy(&token, |owner| Ok(owner.entities())),
        }
    }

    pub fn semantic_graph_relations_for_owner(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<Vec<crate::SemanticGraphRelation>, ReliquaryRuntimeHostError> {
        let token = self.semantic_token(instance, owner_id)?;
        match token.kind {
            OwnerKind::Rel => {
                self.with_rel(&token, |runtime| Ok(runtime.cva.semantic_graph_relations()))
            }
            OwnerKind::Phy => self.with_phy(&token, |owner| Ok(owner.semantic_graph_relations())),
        }
    }

    pub fn memory_routing_metadata_for_owner(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        memory_id: crate::MemoryId,
    ) -> Result<Option<crate::MemoryRoutingMetadata>, ReliquaryRuntimeHostError> {
        let token = self.semantic_token(instance, owner_id)?;
        match token.kind {
            OwnerKind::Rel => self.with_rel(&token, |runtime| {
                Ok(runtime.cva.memory_routing_metadata(memory_id).cloned())
            }),
            OwnerKind::Phy => self.with_phy(&token, |owner| {
                Ok(owner.memory_routing_metadata(memory_id).cloned())
            }),
        }
    }

    pub fn phylactery_profile(
        &self,
        instance: &InstanceRuntime,
    ) -> Result<crate::PhylacteryProfile, ReliquaryRuntimeHostError> {
        let phy = instance
            .attached_phy_owner_id()
            .ok_or_else(|| operation("instance has no bound PHY"))?;
        let token = self.authorize(instance, phy, OwnerGrant::UseOwnPhy)?;
        self.with_phy(&token, |owner| Ok(owner.profile()))
    }

    /// Mutate only this verified instance's exact PHY; REL projection is separate.
    pub fn set_phylactery_profile(
        &self,
        instance: &InstanceRuntime,
        display_name: Option<String>,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        let phy = instance
            .attached_phy_owner_id()
            .ok_or_else(|| operation("instance has no bound PHY"))?;
        let token = self.authorize(instance, phy, OwnerGrant::UseOwnPhy)?;
        let changed = self.with_phy(&token, |owner| {
            let changed = owner.set_profile(display_name).map_err(operation)?;
            if changed {
                owner.sync().map_err(operation)?;
            }
            Ok(changed)
        })?;
        if changed {
            self.wake_phy(phy)?;
        }
        Ok(changed)
    }

    /// Source-backed explicit principal projection. Never consults selected REL.
    pub fn refresh_principal_projection_for(
        &self,
        instance: &InstanceRuntime,
        rel_id: &str,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        let rel = self.authorize(instance, rel_id, OwnerGrant::SubmitConversation)?;
        let Some(phy) = instance.attached_phy_owner_id() else {
            return Ok(false);
        };
        let token = self.authorize(instance, phy, OwnerGrant::UseOwnPhy)?;
        let profile = self.with_phy(&token, |owner| Ok(owner.profile()))?;
        let changed = self.commit_principal_projection(instance, &rel, &token, &profile)?;
        if changed {
            self.wake_rel(rel_id)?;
        }
        Ok(changed)
    }
    fn commit_principal_projection(
        &self,
        instance: &InstanceRuntime,
        rel: &OwnerAccessToken,
        phy: &OwnerAccessToken,
        profile: &crate::PhylacteryProfile,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        let attachment = self.attachment_entry(instance)?;
        let _attachment_lease = attachment.leases.acquire_current()?;
        instance
            .authorize_owner_generation(
                &phy.owner_id,
                OwnerGrant::UseOwnPhy,
                phy.authorization,
                phy.owner_grant_generation,
            )
            .map_err(operation)?;
        let (epoch, fence) = {
            let phys = self
                .phy_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let execution = phys
                .get(&phy.owner_id)
                .ok_or_else(|| operation("profile source PHY is not mounted"))?;
            (
                execution.mount_epoch.load(Ordering::SeqCst),
                Arc::clone(&execution.fence),
            )
        };
        if epoch != phy.mount_epoch {
            return Err(operation("profile source belongs to a previous PHY mount"));
        }
        // Keep the private source live through the REL commit, without taking
        // its semantic store lock. Grant refresh drains the attachment lease.
        let _source_lease = fence.acquire(phy.fence_generation)?;
        self.with_rel_mutation(
            rel,
            super::mutation_events::RelMutationFamily::Semantic,
            |runtime| {
                instance
                    .authorize_owner_generation(
                        &phy.owner_id,
                        OwnerGrant::UseOwnPhy,
                        phy.authorization,
                        phy.owner_grant_generation,
                    )
                    .map_err(operation)?;
                let changed = runtime
                    .cva
                    .sync_principal_profile(
                        &phy.owner_id,
                        profile,
                        crate::insomnia::runtime_step::now_ns(),
                    )
                    .map_err(operation)?;
                Ok((
                    changed,
                    false,
                    vec![super::events::RelInvalidation::PrincipalEntity(
                        crate::entity_principal::principal_entity_id(&phy.owner_id),
                    )],
                ))
            },
        )
    }
}

#[cfg(test)]
#[path = "runtime_host_gateway_semantic_tests.rs"]
mod tests;
