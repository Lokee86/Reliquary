//! Public offline maintenance bindings for exact historical-copy invariants.
use crate::{Cva, CvaError};
impl Cva {
    pub(crate) fn validate_historical_import_copy(&self, copy: &Self) -> Result<(), CvaError> {
        self.owner.validate_historical_import_copy(&copy.owner)
    }
    pub(crate) fn validate_historical_principal_repack(
        &self,
        principal: &str,
    ) -> Result<(), CvaError> {
        self.owner.validate_historical_principal_repack(principal)
    }
}
