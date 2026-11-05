use super::*;
use reliquary_application::external_window_id;
use reliquary_owner_session::records::{ExternalMemoryDraft, ExternalMemoryReceipt};
use reliquary_switchboard::GeneralEndpoint;

impl Gateway {
    /// Derive and publish one exact host window. The descriptor cut is
    /// checked again under the REL lease before any durable write.
    pub fn publish_external_memory<E: GeneralEndpoint>(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
        expected: &[ExternalMessageDescriptor],
        endpoint: E,
    ) -> Result<ExternalMemoryReceipt, GatewayError> {
        let namespace = self.check_external_host(instance, owner, source)?;
        validate_external_hydration_request(&namespace, expected).map_err(external_error)?;
        let principal = instance.principal();
        let window = external_window_id(expected).map_err(external_error)?;
        let snapshot = instance
            .with_writable_reliquary(owner, |session| {
                session
                    .external_reference_snapshot(&namespace, principal)
                    .cloned()
            })?
            .ok_or_else(|| external_error(ExternalSourceError::RebuildRequired))?;
        if expected
            .iter()
            .any(|descriptor| !snapshot.manifest().descriptors.contains(descriptor))
        {
            return Err(external_error(ExternalSourceError::StaleEvidence));
        }
        let hydrated = self.hydrate_external_source(instance, owner, source, expected)?;
        let recovered = instance.with_rel_writer(owner, |writer| {
            if writer
                .read_session()
                .external_reference_snapshot(&namespace, principal)
                != Some(&snapshot)
            {
                return Err(external_error(ExternalSourceError::Conflict));
            }
            Ok(writer.resume_external_memory_publication(principal, window)?)
        })??;
        if let Some(receipt) = recovered {
            return Ok(receipt);
        }
        let proposals = reliquary_application::prepare_external_memory(
            endpoint, &namespace, expected, &hydrated,
        )
        .map_err(external_error)?;
        let entries = proposals
            .into_iter()
            .enumerate()
            .map(|(ordinal, proposal)| {
                let (draft, authority, content, grounding) =
                    proposal.into_project_draft(principal, window, ordinal)?;
                Ok(ExternalMemoryDraft {
                    draft,
                    authority_source: authority,
                    content_sources: content.into_iter().collect(),
                    grounding_sources: grounding.into_iter().collect(),
                })
            })
            .collect::<Result<Vec<_>, ExternalSourceError>>()
            .map_err(external_error)?;
        self.hydrate_external_source(instance, owner, source, expected)?;
        instance.with_rel_writer(owner, |writer| {
            if writer
                .read_session()
                .external_reference_snapshot(&namespace, principal)
                != Some(&snapshot)
            {
                return Err(external_error(ExternalSourceError::Conflict));
            }
            Ok(writer.publish_external_memory(principal, window, entries)?)
        })?
    }
}
