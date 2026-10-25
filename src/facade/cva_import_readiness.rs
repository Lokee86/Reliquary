use crate::imports::ImportReadinessReport;
use crate::{Cva, CvaError};

impl Cva {
    /// Inspect owner-derived readiness for the exact currently open native import.
    /// Import owns the readiness policy and lifecycle view assembly.
    pub fn native_import_readiness(
        &mut self,
        import_id: &str,
    ) -> Result<ImportReadinessReport, CvaError> {
        self.runtime_native_import_readiness(import_id)
            .map_err(CvaError::from)
    }
}
