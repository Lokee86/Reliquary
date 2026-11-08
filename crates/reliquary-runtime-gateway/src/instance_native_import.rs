use super::*;
use reliquary_application::native_import_source::{
    NativeImportPlan, NativeImportStageError, NativeImportStageResult, NativeImportStatus,
    native_import_status,
};

impl Instance {
    pub fn stage_native_import(
        &self,
        owner: OwnerId,
        plan: NativeImportPlan,
    ) -> Result<NativeImportStageResult, GatewayError> {
        self.with_rel_writer(owner, |writer| {
            plan.stage(writer, self.principal(), || {
                self.ensure_credential_active().is_ok()
            })
        })?
        .map_err(|error| match error {
            NativeImportStageError::Conflict => GatewayError::NativeImportConflict,
            NativeImportStageError::Revoked => GatewayError::CredentialRevoked,
            NativeImportStageError::Owner(error) => GatewayError::OwnerSession(error),
        })
    }

    pub fn native_import_status(
        &self,
        owner: OwnerId,
        import_id: &str,
    ) -> Result<Option<NativeImportStatus>, GatewayError> {
        self.with_reliquary(owner, |session| native_import_status(session, import_id))
    }
}
