use super::*;
use reliquary_application::apply_external_change_batch;

impl Gateway {
    /// Advance one bounded host change page against the exact recorded cut.
    pub fn advance_external_source_manifest(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
    ) -> Result<ExternalReferenceSnapshotReceipt, GatewayError> {
        let namespace = self.check_external_host(instance, owner, source)?;
        let principal = instance.principal();
        let base = instance
            .with_writable_reliquary(owner, |session| {
                session
                    .external_reference_snapshot(&namespace, principal)
                    .cloned()
            })?
            .ok_or_else(|| external_error(ExternalSourceError::RebuildRequired))?;
        let batch = source
            .changes_after(principal, base.manifest().watermark, MAX_EXTERNAL_BATCH)
            .map_err(external_error)?;
        let applied =
            apply_external_change_batch(base.manifest(), &batch).map_err(external_error)?;
        self.recheck_external_host(instance, owner, source, &namespace)?;
        instance.with_rel_writer(owner, |writer| {
            if writer
                .read_session()
                .external_reference_snapshot(&namespace, principal)
                != Some(&base)
            {
                return Err(external_error(ExternalSourceError::Conflict));
            }
            Ok(writer.record_external_reference_snapshot(principal, applied)?)
        })?
    }
}
