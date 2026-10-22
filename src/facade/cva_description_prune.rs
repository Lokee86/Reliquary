//! Facade adapters for read-only retained-description proofs.

use crate::owner_lifecycle::{HistorySource, LifecycleTarget};
use crate::{Cva, CvaError};

impl Cva {
    pub(crate) fn description_retention(
        &self,
        target: &LifecycleTarget,
    ) -> Result<crate::owner_maintenance::HistoryRetentionDescriptor, CvaError> {
        crate::history::reliquary_description_retention::description_retention(
            &self.entities,
            &self.relationships,
            target,
        )
    }

    pub(crate) fn validate_pruned_descriptions(&self) -> Result<(), CvaError> {
        let source = HistorySource::new(&self.container);
        crate::history::reliquary_description_retention::validate_description_retention(
            source,
            &self.entities,
            &self.relationships,
            &self.owner_maintenance,
        )
    }
}
