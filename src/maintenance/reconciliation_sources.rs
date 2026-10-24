//! Source leases held across reconciliation without exposing physical methods.
use crate::owner_lifecycle::ReliquaryOwner;
use crate::{Container, ContainerError, CvaReconcileError};
use std::{fs, path::Path};

pub(crate) struct ReconciliationSources {
    left: Container,
    right: Container,
}
impl ReconciliationSources {
    pub(crate) fn open(left: &Path, right: &Path) -> Result<Self, CvaReconcileError> {
        let left = Container::open_read_only_for_scan(left)?;
        let right = Container::open_read_only_for_scan(right)?;
        left.validate_path_identity()?;
        right.validate_path_identity()?;
        Ok(Self { left, right })
    }
    pub(crate) fn validate_output(
        &self,
        output: &ReliquaryOwner,
        path: &Path,
    ) -> Result<(), CvaReconcileError> {
        let sources = self
            .left
            .validate_path_identity()
            .and_then(|()| self.right.validate_path_identity());
        let candidate = output.container.validate_path_identity();
        if sources.is_err() || candidate.is_err() {
            if candidate.is_ok() {
                let _ = fs::remove_file(path);
            }
            return Err(ContainerError::UnstableSnapshot.into());
        }
        Ok(())
    }
}
