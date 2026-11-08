use super::*;
use reliquary_application::native_import_workflow::{
    NativeImportBatch, StagedNativeImport, settle_native_import, stage_native_import,
};
use reliquary_owner_session::records::ImportFreshnessSettlement;

impl Instance {
    /// Stage a source-bound copied snapshot through the existing native import
    /// workflow. This does not create live Gateway turns or convert legacy files.
    pub fn stage_native_import(
        &self,
        owner: OwnerId,
        batch: NativeImportBatch,
    ) -> Result<StagedNativeImport, GatewayError> {
        Ok(self.with_rel_writer(owner, |writer| stage_native_import(writer, batch))??)
    }

    /// Freeze the accepted import's historical Freshness settlement.
    pub fn settle_native_import(
        &self,
        owner: OwnerId,
        import_id: &str,
    ) -> Result<ImportFreshnessSettlement, GatewayError> {
        Ok(self.with_rel_writer(owner, |writer| settle_native_import(writer, import_id))??)
    }
}
