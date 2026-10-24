use crate::PhylacteryError;
use std::path::Path;

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PhylacteryError> {
        let mut owner = Self::open_unvalidated(path)?;
        owner.validate_prune_evidence()?;
        owner.validate_open_owner()?;
        Ok(owner)
    }

    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, PhylacteryError> {
        let mut owner = Self::open_read_only_unvalidated(path)?;
        owner.validate_prune_evidence()?;
        owner.validate_open_owner()?;
        Ok(owner)
    }
}
