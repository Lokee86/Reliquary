use super::*;
use reliquary_application::native_import_source::{
    HermesCopiedSourceTurn, HermesCopyScope, NativeImportPlan, NativeImportStageError,
    NativeImportStageResult, NativeImportStatus, native_import_status, read_hermes_archive_range,
    read_hermes_copy_source, tombstone_hermes_copy,
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
        .map_err(import_error)
    }

    pub fn read_hermes_archive_range(
        &self,
        owner: OwnerId,
        conversation_id: &str,
        start_node_id: &str,
        end_node_id: &str,
        source_archive_version: u64,
    ) -> Result<Option<Vec<HermesCopiedSourceTurn>>, GatewayError> {
        let result = self.with_reliquary(owner, |session| {
            read_hermes_archive_range(
                session,
                conversation_id,
                start_node_id,
                end_node_id,
                source_archive_version,
                self.principal(),
            )
        })??;
        self.ensure_credential_active()?;
        Ok(result)
    }

    pub fn read_hermes_copy_source(
        &self,
        owner: OwnerId,
        scope: &HermesCopyScope,
        source_sha256: [u8; 32],
        start_message_uid: &str,
        end_message_uid: &str,
    ) -> Result<Option<Vec<HermesCopiedSourceTurn>>, GatewayError> {
        let result = self.with_reliquary(owner, |session| {
            read_hermes_copy_source(
                session,
                scope,
                source_sha256,
                start_message_uid,
                end_message_uid,
                self.principal(),
            )
        })??;
        self.ensure_credential_active()?;
        Ok(result)
    }

    pub fn tombstone_hermes_copy(
        &self,
        owner: OwnerId,
        scope: &HermesCopyScope,
        source_sha256: [u8; 32],
    ) -> Result<bool, GatewayError> {
        self.with_rel_writer(owner, |writer| {
            tombstone_hermes_copy(writer, scope, source_sha256, self.principal(), || {
                self.ensure_credential_active().is_ok()
            })
        })?
        .map_err(import_error)
    }

    pub fn native_import_status(
        &self,
        owner: OwnerId,
        import_id: &str,
    ) -> Result<Option<NativeImportStatus>, GatewayError> {
        self.with_reliquary(owner, |session| native_import_status(session, import_id))
    }
}

fn import_error(error: NativeImportStageError) -> GatewayError {
    match error {
        NativeImportStageError::Conflict => GatewayError::NativeImportConflict,
        NativeImportStageError::Revoked => GatewayError::CredentialRevoked,
        NativeImportStageError::Owner(error) => GatewayError::OwnerSession(error),
    }
}
