//! Concrete PHY transitions share the gateway's admission and lifecycle authority.
use super::*;

impl GatewayRuntime {
    pub fn quiesce_phy(&self, owner_id: &str) -> Result<(), ReliquaryRuntimeHostError> {
        let _transition = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        self.quiesce_phy_inner(owner_id)
    }

    fn quiesce_phy_inner(&self, owner_id: &str) -> Result<(), ReliquaryRuntimeHostError> {
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        let (fence, runtime) = {
            let owners = self
                .phy_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let owner = owners
                .get(owner_id)
                .ok_or_else(|| operation("PHY owner is not mounted"))?;
            (Arc::clone(&owner.fence), Arc::clone(&owner.runtime))
        };
        fence.close_and_drain()?;
        runtime
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .stop_workers()
    }

    pub fn quiesce_phy_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let token = self.authorize(instance, owner_id, OwnerGrant::UseOwnPhy)?;
        let _transition = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let attachment = self.attachment_entry(instance)?;
        let _delivery = attachment.leases.acquire_current()?;
        instance
            .authorize_owner_generation(
                owner_id,
                OwnerGrant::UseOwnPhy,
                token.authorization,
                token.owner_grant_generation,
            )
            .map_err(operation)?;
        {
            let owners = self
                .phy_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let owner = owners
                .get(owner_id)
                .ok_or_else(|| operation("PHY owner is not mounted"))?;
            if owner.mount_epoch.load(Ordering::SeqCst) != token.mount_epoch
                || owner.fence.snapshot()?.1 != token.fence_generation
            {
                return Err(operation("PHY lifecycle token is stale"));
            }
        }
        self.quiesce_phy_inner(owner_id)
    }

    pub fn reopen_phy(&self, owner_id: &str) -> Result<u64, ReliquaryRuntimeHostError> {
        let _transition = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        self.reopen_phy_inner(owner_id)
    }

    pub fn reopen_phy_for(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<u64, ReliquaryRuntimeHostError> {
        let _transition = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let attachment = self.attachment_entry(instance)?;
        let _delivery = attachment.leases.acquire_current()?;
        let authorization = instance.authorization().map_err(operation)?;
        instance
            .authorize(owner_id, OwnerGrant::UseOwnPhy, authorization)
            .map_err(operation)?;
        self.reopen_phy_inner(owner_id)
    }

    fn reopen_phy_inner(&self, owner_id: &str) -> Result<u64, ReliquaryRuntimeHostError> {
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        let (fence, runtime, owner) = {
            let owners = self
                .phy_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let execution = owners
                .get(owner_id)
                .ok_or_else(|| operation("PHY owner is not mounted"))?;
            (
                Arc::clone(&execution.fence),
                Arc::clone(&execution.runtime),
                Arc::clone(&execution.owner),
            )
        };
        fence.validate_reopen()?;
        let mut runtime = runtime
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if !runtime.workers_stopped() {
            return Err(operation(
                "PHY workers must be stopped before reconciliation",
            ));
        }
        {
            let owner = owner
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            if owner.owner_id().as_deref() != Some(owner_id) {
                return Err(operation(
                    "PHY durable identity changed during reconciliation",
                ));
            }
            owner.sync().map_err(operation)?;
        }
        runtime.reconcile()?;
        let epoch = self.allocate_mount_epoch()?;
        self.phy_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .get(owner_id)
            .ok_or_else(|| operation("PHY owner is not mounted"))?
            .mount_epoch
            .store(epoch, Ordering::SeqCst);
        // Workers acquire the same fence: open before spawning, after durable preflight.
        let generation = fence.reopen()?;
        if let Err(error) = runtime.restart_workers() {
            let _ = fence.close_and_drain();
            let _ = runtime.stop_workers();
            return Err(error);
        }
        Ok(generation)
    }

    pub(super) fn wake_phy(&self, owner_id: &str) -> Result<(), ReliquaryRuntimeHostError> {
        let runtime = {
            let owners = self
                .phy_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            Arc::clone(
                &owners
                    .get(owner_id)
                    .ok_or_else(|| operation("PHY owner is not mounted"))?
                    .runtime,
            )
        };
        runtime
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .wake()
    }
}
