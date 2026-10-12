//! Exact-owner background controls. No active selection, store handle exports or queue.
use super::super::*;
use crate::{MemoryStats, MemoryVectorStats, RuntimeBackgroundStatus};

fn empty_status() -> RuntimeBackgroundStatus {
    RuntimeBackgroundStatus {
        insomnia: crate::InsomniaStats {
            total: 0,
            pending: 0,
            processing: 0,
            complete: 0,
            failed: 0,
            terminal: 0,
            attempts: 0,
        },
        reliquary_memories: MemoryStats {
            memories: 0,
            revisions: 0,
            bodies: 0,
            memory_version: 0,
        },
        reliquary_vectors: MemoryVectorStats {
            objects: 0,
            rows: 0,
            bindings: 0,
        },
        phylactery_memories: None,
        phylactery_vectors: None,
        reliquary_vector_pending: 0,
        phylactery_vector_pending: 0,
        reliquary_dream_pending: 0,
        phylactery_dream_pending: 0,
        reliquary_perception_pending: 0,
        phylactery_perception_pending: 0,
    }
}

impl GatewayRuntime {
    /// Trusted machine-local route replacement, shared by all mounted owners.
    /// Endpoint calls are never made while configuration or owner locks are held.
    pub fn set_routes(
        &self,
        routes: ReliquaryRuntimeRoutes,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        *self
            .routes
            .write()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)? = routes;
        let rels = self
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .values()
            .map(|rel| Arc::clone(&rel.owner))
            .collect::<Vec<_>>();
        let phys = self
            .phy_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .values()
            .map(|phy| Arc::clone(&phy.runtime))
            .collect::<Vec<_>>();
        for rel in rels {
            rel.lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .routes_changed()?;
        }
        for phy in phys {
            phy.lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .wake()?;
        }
        Ok(())
    }

    /// Exact REL counters. This deliberately includes no personal-owner state.
    pub fn background_status_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<RuntimeBackgroundStatus, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
        let (profile, pending) = {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let execution = rels
                .get(owner_id)
                .ok_or_else(|| operation("REL is not mounted"))?
                .owner
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let profile = *execution
                .memory_profile
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let pending = execution
                .perception_queue
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .counts();
            (profile, pending)
        };
        self.with_rel(&token, |runtime| {
            let cva = &mut runtime.cva;
            let mut status = empty_status();
            status.insomnia = cva.insomnia_stats();
            status.reliquary_memories = cva.memory_stats();
            status.reliquary_vectors = cva.memory_vector_stats();
            status.reliquary_perception_pending = pending;
            let now = crate::dream_cooldown::unix_now_ns();
            for id in cva.memory_ids() {
                if cva.memory(id).map_err(operation)?.archived {
                    continue;
                }
                let has_vector = match profile {
                    Some(profile) => {
                        let body = cva.memory_body_id(id).map_err(operation)?;
                        cva.memory_vector_location(profile, body).is_some()
                    }
                    None => false,
                };
                if !has_vector {
                    status.reliquary_vector_pending += 1;
                } else if cva
                    .dream_eligible_epoch(id, now)
                    .map_err(operation)?
                    .is_some()
                {
                    status.reliquary_dream_pending += 1;
                }
            }
            Ok(status)
        })
    }

    /// Private PHY counters require this attachment's exact UseOwnPhy grant.
    pub fn phy_background_status_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<RuntimeBackgroundStatus, ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::UseOwnPhy)?;
        let (profile, pending) = {
            let phys = self
                .phy_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let runtime = phys
                .get(owner_id)
                .ok_or_else(|| operation("PHY is not mounted"))?
                .runtime
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            (runtime.memory_profile()?, runtime.perception_pending()?)
        };
        self.with_phy(&token, |phy| {
            let mut status = empty_status();
            status.phylactery_memories = Some(phy.memory_stats());
            status.phylactery_vectors = Some(phy.memory_vector_stats());
            status.phylactery_perception_pending = pending;
            let now = crate::dream_cooldown::unix_now_ns();
            for id in phy.memory_ids() {
                if phy.memory(id).map_err(operation)?.archived {
                    continue;
                }
                let has_vector = match profile {
                    Some(profile) => {
                        let body = phy.memory_body_id(id).map_err(operation)?;
                        phy.memory_vector_location(profile, body).is_some()
                    }
                    None => false,
                };
                if !has_vector {
                    status.phylactery_vector_pending += 1;
                } else if phy
                    .dream_eligible_epoch(id, now)
                    .map_err(operation)?
                    .is_some()
                {
                    status.phylactery_dream_pending += 1;
                }
            }
            Ok(status)
        })
    }

    pub fn wake_rel_background_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageRel)?;
        let signal = {
            let owners = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            Arc::clone(
                &owners
                    .get(owner_id)
                    .ok_or_else(|| operation("REL is not mounted"))?
                    .owner
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .signal,
            )
        };
        self.with_rel(&token, |_| super::super::super::notify_work(&signal))
    }

    pub fn wake_phy_background_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::UseOwnPhy)?;
        let shared = {
            let owners = self
                .phy_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            owners
                .get(owner_id)
                .ok_or_else(|| operation("PHY is not mounted"))?
                .runtime
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .control_handle()
        };
        self.with_phy(&token, |_| shared.wake())
    }

    /// Controls REL Dream/Perception/vector dispatch. Insomnia has its separate switch.
    pub fn set_rel_background_enabled_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        enabled: bool,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::ManageRel)?;
        let shared = {
            let owners = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            owners
                .get(owner_id)
                .ok_or_else(|| operation("REL is not mounted"))?
                .owner
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .control_handle()
        };
        self.with_rel(&token, |_| {
            shared.background_enabled.store(enabled, Ordering::SeqCst);
            super::super::super::notify_work(&shared.signal)
        })
    }

    pub fn set_phy_background_enabled_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        enabled: bool,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::UseOwnPhy)?;
        let shared = {
            let owners = self
                .phy_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            owners
                .get(owner_id)
                .ok_or_else(|| operation("PHY is not mounted"))?
                .runtime
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .control_handle()
        };
        self.with_phy(&token, |_| {
            shared.background_enabled.store(enabled, Ordering::SeqCst);
            shared.wake()
        })
    }

    /// Live thread count and the owner-local background dispatch switch.
    /// Quiesced owners reject instance operations until reopened.
    pub fn background_worker_state_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<(usize, bool), ReliquaryRuntimeHostError> {
        if owner_id.starts_with("rel-") {
            let token = self.authorize(instance, owner_id, OwnerGrant::ViewRel)?;
            let state = {
                let owners = self
                    .rel_executions
                    .read()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                owners
                    .get(owner_id)
                    .ok_or_else(|| operation("REL is not mounted"))?
                    .owner
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .worker_state()
            };
            self.with_rel(&token, |_| Ok(state))
        } else {
            let token = self.authorize(instance, owner_id, OwnerGrant::UseOwnPhy)?;
            let state = {
                let owners = self
                    .phy_executions
                    .read()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                owners
                    .get(owner_id)
                    .ok_or_else(|| operation("PHY is not mounted"))?
                    .runtime
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                    .worker_state()
            };
            self.with_phy(&token, |_| Ok(state))
        }
    }
}

#[cfg(test)]
#[path = "runtime_host_gateway_background_tests.rs"]
mod tests;
