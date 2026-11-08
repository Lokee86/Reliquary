use super::*;
mod feed;
mod project_attachment;

use reliquary_application::{
    ExternalChangeBatch, ExternalLexicalIndex, HydratedExternalChunk, HydratedExternalMessage,
    validate_external_hydration_request, verify_external_hydration,
};
use reliquary_owner_session::records::external_index::model::{
    EXTERNAL_SOURCE_CONTRACT, ExternalChunkRef, ExternalMessageDescriptor, ExternalSourceError,
    MAX_EXTERNAL_BATCH, identifier,
};
use reliquary_owner_session::records::{
    ExternalReferenceSnapshotManifest, ExternalReferenceSnapshotReceipt,
};

/// Trusted host boundary. A REL grant alone does not grant access to host text.
pub trait ExternalHostSource: Send + Sync {
    fn namespace(&self) -> &str;
    fn contract_version(&self) -> u32;
    fn authorize(&self, principal: &str) -> Result<(), ExternalSourceError>;
    fn snapshot_manifest(
        &self,
        principal: &str,
    ) -> Result<ExternalReferenceSnapshotManifest, ExternalSourceError>;
    fn changes_after(
        &self,
        principal: &str,
        cursor: u64,
        limit: usize,
    ) -> Result<ExternalChangeBatch, ExternalSourceError>;
    fn hydrate(
        &self,
        principal: &str,
        expected: &[ExternalMessageDescriptor],
    ) -> Result<Vec<HydratedExternalMessage>, ExternalSourceError>;
}

/// A disposable index bound to the Instance and host source that built it.
pub struct AuthorizedExternalLexicalIndex {
    instance_id: Uuid,
    owner: OwnerId,
    namespace: String,
    index: ExternalLexicalIndex,
}

impl Gateway {
    /// Read the trusted host's reference-only cut for an owner-scoped rebuild.
    pub fn external_source_manifest(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
    ) -> Result<ExternalReferenceSnapshotManifest, GatewayError> {
        let namespace = self.check_external_host(instance, owner, source)?;
        let manifest = source
            .snapshot_manifest(instance.principal())
            .map_err(external_error)?;
        if manifest.namespace != namespace {
            return Err(external_error(ExternalSourceError::InvalidContract));
        }
        manifest.validate().map_err(external_error)?;
        self.recheck_external_host(instance, owner, source, &namespace)?;
        Ok(manifest)
    }

    /// Inspect the last accepted reference-only cut for this owner and principal.
    pub fn recorded_external_source_manifest(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
    ) -> Result<Option<ExternalReferenceSnapshotReceipt>, GatewayError> {
        let namespace = self.check_external_host(instance, owner, source)?;
        let recorded = instance.with_reliquary(owner, |session| {
            session
                .external_reference_snapshot(&namespace, instance.principal())
                .cloned()
        })?;
        self.recheck_external_host(instance, owner, source, &namespace)?;
        Ok(recorded)
    }

    /// Persist the host's authorized descriptor cut without copying host text.
    pub fn record_external_source_manifest(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
    ) -> Result<ExternalReferenceSnapshotReceipt, GatewayError> {
        let manifest = self.external_source_manifest(instance, owner, source)?;
        Ok(instance.with_rel_writer(owner, |writer| {
            writer.record_external_reference_snapshot(instance.principal(), manifest)
        })??)
    }

    /// Rebuild disposable lexical hints from a current, authorized host cut.
    pub fn rebuild_external_lexical_index(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
    ) -> Result<AuthorizedExternalLexicalIndex, GatewayError> {
        let manifest = self.external_source_manifest(instance, owner, source)?;
        let mut index = ExternalLexicalIndex::new(&manifest.namespace).map_err(external_error)?;
        let active: Vec<_> = manifest
            .descriptors
            .into_iter()
            .filter(|descriptor| descriptor.active)
            .collect();
        for expected in active.chunks(MAX_EXTERNAL_BATCH) {
            let hydrated = self.hydrate_external_source(instance, owner, source, expected)?;
            index
                .add_batch(expected, &hydrated)
                .map_err(external_error)?;
        }
        self.recheck_external_host(instance, owner, source, &manifest.namespace)?;
        Ok(AuthorizedExternalLexicalIndex {
            instance_id: instance.instance_id,
            owner,
            namespace: manifest.namespace,
            index,
        })
    }

    /// Search only hints captured under this Instance's host authorization.
    pub fn search_external_lexical_index(
        &self,
        instance: &Instance,
        source: &dyn ExternalHostSource,
        index: &AuthorizedExternalLexicalIndex,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ExternalChunkRef>, GatewayError> {
        if index.instance_id != instance.instance_id {
            return Err(external_error(ExternalSourceError::Unauthorized));
        }
        let namespace = self.check_external_host(instance, index.owner, source)?;
        if namespace != index.namespace {
            return Err(external_error(ExternalSourceError::Unauthorized));
        }
        let hits = index.index.search(query, limit).map_err(external_error)?;
        self.recheck_external_host(instance, index.owner, source, &namespace)?;
        Ok(hits)
    }

    /// Hydrate exact host descriptors with both REL and host authorization.
    /// The caller supplies the adapter attached to this owner by the trusted host.
    pub fn hydrate_external_source(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
        expected: &[ExternalMessageDescriptor],
    ) -> Result<Vec<HydratedExternalMessage>, GatewayError> {
        let namespace = self.check_external_host(instance, owner, source)?;
        validate_external_hydration_request(&namespace, expected).map_err(external_error)?;
        let hydrated = source
            .hydrate(instance.principal(), expected)
            .map_err(external_error)?;
        verify_external_hydration(&namespace, expected, &hydrated).map_err(external_error)?;
        self.recheck_external_host(instance, owner, source, &namespace)?;
        Ok(hydrated)
    }

    /// Reauthorize a search hint and return only its verified UTF-8 range.
    pub fn hydrate_external_chunk(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
        expected: &ExternalMessageDescriptor,
        reference: ExternalChunkRef,
    ) -> Result<HydratedExternalChunk, GatewayError> {
        if expected.reference != reference.message {
            return Err(external_error(ExternalSourceError::StaleEvidence));
        }
        self.hydrate_external_source(instance, owner, source, std::slice::from_ref(expected))?
            .pop()
            .ok_or_else(|| external_error(ExternalSourceError::StaleEvidence))?
            .into_chunk(reference)
            .map_err(external_error)
    }

    fn check_external_host(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
    ) -> Result<String, GatewayError> {
        if !Arc::ptr_eq(&self.owners, &instance.owners) {
            return Err(external_error(ExternalSourceError::Unauthorized));
        }
        instance.check_external_source_owner(owner)?;
        if source.contract_version() != EXTERNAL_SOURCE_CONTRACT {
            return Err(external_error(ExternalSourceError::InvalidContract));
        }
        let namespace = source.namespace();
        identifier(namespace).map_err(external_error)?;
        let namespace = namespace.to_owned();
        source
            .authorize(instance.principal())
            .map_err(external_error)?;
        instance.check_external_source_owner(owner)?;
        Ok(namespace)
    }

    fn recheck_external_host(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
        namespace: &str,
    ) -> Result<(), GatewayError> {
        if source.namespace() != namespace || source.contract_version() != EXTERNAL_SOURCE_CONTRACT
        {
            return Err(external_error(ExternalSourceError::StaleEvidence));
        }
        source
            .authorize(instance.principal())
            .map_err(external_error)?;
        instance.check_external_source_owner(owner)
    }
}

impl Instance {
    fn check_external_source_owner(&self, owner: OwnerId) -> Result<(), GatewayError> {
        self.authorize_queue_owner(owner)?;
        let owners = self
            .owners
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        match owners.get(&owner) {
            Some(mounted) if mounted.is_rel() => Ok(()),
            Some(_) => Err(GatewayError::WrongOwnerKind(owner)),
            None => Err(GatewayError::UnknownOwner(owner)),
        }
    }
}

fn external_error(error: ExternalSourceError) -> GatewayError {
    GatewayError::ExternalSource(error)
}

#[cfg(test)]
mod tests;

impl Gateway {
    /// Evaluate the exact host window outside owner locks and recheck access
    /// before returning its reference-bound proposals.
    pub fn prepare_external_memory<E: reliquary_switchboard::GeneralEndpoint>(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
        expected: &[ExternalMessageDescriptor],
        endpoint: E,
    ) -> Result<Vec<reliquary_application::ExternalMemoryProposal>, GatewayError> {
        let namespace = self.check_external_host(instance, owner, source)?;
        let hydrated = self.hydrate_external_source(instance, owner, source, expected)?;
        let proposals = reliquary_application::prepare_external_memory(
            endpoint, &namespace, expected, &hydrated,
        );
        self.recheck_external_host(instance, owner, source, &namespace)?;
        Ok(proposals.map_err(external_error)?)
    }
}

mod publication;
