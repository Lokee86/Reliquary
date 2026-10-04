use super::{
    ReliquaryRuntimeHost, ReliquaryRuntimeHostError, gateway::mount::MountClaim, notify_work,
    operation, perception_flow, perception_owner, perception_queue,
};
use crate::Phylactery;
use std::sync::atomic::Ordering;

impl ReliquaryRuntimeHost {
    pub fn phylactery_profile(
        &self,
    ) -> Result<Option<crate::PhylacteryProfile>, ReliquaryRuntimeHostError> {
        self.phylactery
            .lock()
            .map(|slot| slot.as_ref().map(crate::Phylactery::profile))
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)
    }

    pub fn set_phylactery_profile(
        &self,
        display_name: Option<String>,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        let (changed, principal_id, profile) = {
            let mut slot = self
                .phylactery
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let phy = slot.as_mut().ok_or_else(|| {
                ReliquaryRuntimeHostError::Operation("No Phylactery is attached".into())
            })?;
            let changed = phy.set_profile(display_name).map_err(operation)?;
            if changed {
                phy.sync().map_err(operation)?;
            }
            (changed, phy.owner_id(), phy.profile())
        };

        if let Some(principal_id) = principal_id
            && let Ok(execution) = self.active_execution()
        {
            let mut runtime = execution
                .runtime
                .as_ref()
                .ok_or_else(|| {
                    ReliquaryRuntimeHostError::Operation("Reliquary runtime is unavailable".into())
                })?
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let rel_changed = runtime
                .cva
                .sync_principal_profile(&principal_id, &profile, perception_flow::now_ns())
                .map_err(operation)?;
            if rel_changed {
                runtime.cva.sync().map_err(operation)?;
            }
        }
        Ok(changed)
    }

    pub fn has_phylactery(&self) -> Result<bool, ReliquaryRuntimeHostError> {
        self.phylactery
            .lock()
            .map(|slot| slot.is_some())
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)
    }

    pub fn attach_phylactery(
        &self,
        mut phylactery: Phylactery,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let mut claim_slot = self
            .legacy_phy_claim
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if claim_slot.is_some() {
            return Err(ReliquaryRuntimeHostError::Operation(
                "Reliquary runtime already has a Phylactery attached".into(),
            ));
        }
        let claim = MountClaim::acquire_legacy_phy(&phylactery.container)?;
        let principal_profile = phylactery
            .owner_id()
            .map(|owner_id| (owner_id, phylactery.profile()));
        let perception_keys = perception_owner::startup_keys_user(&mut phylactery)?;
        {
            let mut slot = self
                .phylactery
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            if slot.is_some() {
                return Err(ReliquaryRuntimeHostError::Operation(
                    "Reliquary runtime already has a Phylactery attached".into(),
                ));
            }
            *slot = Some(phylactery);
        }
        *claim_slot = Some(claim);
        drop(claim_slot);

        if let Ok(execution) = self.active_execution() {
            execution.phylactery_enabled.store(true, Ordering::SeqCst);
            if let Some((principal_id, profile)) = principal_profile {
                let mut runtime = execution
                    .runtime
                    .as_ref()
                    .ok_or_else(|| {
                        ReliquaryRuntimeHostError::Operation(
                            "Reliquary runtime is unavailable".into(),
                        )
                    })?
                    .lock()
                    .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                if runtime
                    .cva
                    .sync_principal_profile(&principal_id, &profile, perception_flow::now_ns())
                    .map_err(operation)?
                {
                    runtime.cva.sync().map_err(operation)?;
                }
            }
            let mut queue = execution
                .perception_queue
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            queue.clear(perception_queue::PerceptionOwner::User);
            queue.push_all(perception_queue::PerceptionOwner::User, perception_keys);
            drop(queue);
            notify_work(&execution.signal)?;
        }
        Ok(())
    }

    pub fn detach_phylactery(&self) -> Result<Option<Phylactery>, ReliquaryRuntimeHostError> {
        let mut claim_slot = self
            .legacy_phy_claim
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let phylactery = self
            .phylactery
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .take();
        let detached_claim = phylactery.as_ref().and_then(|_| claim_slot.take());
        for execution in self.executions.values() {
            execution.phylactery_enabled.store(false, Ordering::SeqCst);
            execution
                .memory_profiles
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .user = None;
            execution
                .perception_queue
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .clear(perception_queue::PerceptionOwner::User);
            notify_work(&execution.signal)?;
        }
        drop(detached_claim);
        Ok(phylactery)
    }
}
