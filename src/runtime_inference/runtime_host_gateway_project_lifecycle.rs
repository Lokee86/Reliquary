use super::{GatewayRuntime, OwnerKind, mount::MountClaim, mount::MountedFileIdentity};
use crate::ReliquaryRuntimeHostError;
use std::path::Path;
use std::sync::MutexGuard;

impl GatewayRuntime {
    pub(crate) fn reserve_new_rel_path(
        &self,
        rel_path: &Path,
    ) -> Result<MutexGuard<'_, ()>, ReliquaryRuntimeHostError> {
        let guard = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if rel_path.exists() {
            return Err(super::operation("new Project REL target already exists"));
        }
        Ok(guard)
    }
    pub(crate) fn admit_unmounted_path(
        &self,
        path: &Path,
    ) -> Result<MountClaim, super::ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| super::ReliquaryRuntimeHostError::LockPoisoned)?;
        let identity = MountedFileIdentity::inspect(path, OwnerKind::Rel)?;
        if self.existing_file_mount(&identity)? {
            return Err(super::operation(
                "project lifecycle requires an unmounted REL",
            ));
        }
        MountClaim::acquire(&identity)
    }

    pub(crate) fn reconciliation_embedding_endpoint(
        &self,
    ) -> Result<
        Option<std::sync::Arc<dyn crate::EmbeddingEndpoint + Send + Sync>>,
        ReliquaryRuntimeHostError,
    > {
        Ok(self
            .routes
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .embedding())
    }
}
