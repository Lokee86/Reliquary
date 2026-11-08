//! Trusted host operations over reference-only external source state.

use super::{Gateway, Instance, OwnerGrant, OwnerId, ProductError, runtime};

pub use reliquary_application::{
    ExternalChangeBatch, ExternalChangeKind, ExternalConversationChange, HydratedExternalChunk,
    HydratedExternalMessage,
};
pub use reliquary_insomnia::{
    InsomniaCandidate, InsomniaOwnership, RoutingMention, SourceAttribution, TextField,
};
pub use reliquary_owner_session::records::{
    ExternalMemoryProvenance, ExternalMemoryReceipt, ExternalReferenceSnapshotManifest,
    ExternalReferenceSnapshotReceipt,
    external_index::model::{
        EXTERNAL_SOURCE_CONTRACT, ExternalChunkRef, ExternalMessageDescriptor, ExternalMessageRef,
        ExternalSourceError, MAX_EXTERNAL_BATCH, MAX_EXTERNAL_INDEX_BYTES, MAX_EXTERNAL_MANIFEST,
        MAX_EXTERNAL_MESSAGE_BYTES, MAX_EXTERNAL_RESULTS,
    },
};
pub use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
pub use runtime::ExternalHostSource;

/// Disposable lexical hints scoped to the Instance that built the index.
pub struct AuthorizedExternalLexicalIndex {
    owner: OwnerId,
    inner: runtime::AuthorizedExternalLexicalIndex,
}

impl AuthorizedExternalLexicalIndex {
    pub fn owner_id(&self) -> OwnerId {
        self.owner
    }
}

/// Evaluated content and exact host references for inspection before publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalMemoryProposal {
    pub candidate: InsomniaCandidate,
    pub authority: ExternalChunkRef,
    pub content: Option<ExternalChunkRef>,
    pub grounding: Option<ExternalChunkRef>,
    pub source_time_ns: i64,
}

impl Gateway {
    /// The Gateway holder supplies the adapter attached to this owner. The
    /// request Instance must also be authorized by both the owner and the host.
    pub fn external_source_manifest(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
    ) -> Result<ExternalReferenceSnapshotManifest, ProductError> {
        instance.require_rel_read(owner)?;
        self.inner
            .external_source_manifest(&instance.inner, owner.into(), source)
            .map_err(Into::into)
    }

    pub fn record_external_source_manifest(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
    ) -> Result<ExternalReferenceSnapshotReceipt, ProductError> {
        instance.require(OwnerGrant::OperateRel(owner))?;
        self.inner
            .record_external_source_manifest(&instance.inner, owner.into(), source)
            .map_err(Into::into)
    }

    pub fn recorded_external_source_manifest(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
    ) -> Result<Option<ExternalReferenceSnapshotReceipt>, ProductError> {
        instance.require_rel_read(owner)?;
        self.inner
            .recorded_external_source_manifest(&instance.inner, owner.into(), source)
            .map_err(Into::into)
    }

    pub fn advance_external_source_manifest(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
    ) -> Result<ExternalReferenceSnapshotReceipt, ProductError> {
        instance.require(OwnerGrant::OperateRel(owner))?;
        self.inner
            .advance_external_source_manifest(&instance.inner, owner.into(), source)
            .map_err(Into::into)
    }

    pub fn rebuild_external_lexical_index(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
    ) -> Result<AuthorizedExternalLexicalIndex, ProductError> {
        instance.require_rel_read(owner)?;
        let inner =
            self.inner
                .rebuild_external_lexical_index(&instance.inner, owner.into(), source)?;
        Ok(AuthorizedExternalLexicalIndex { owner, inner })
    }

    pub fn search_external_lexical_index(
        &self,
        instance: &Instance,
        source: &dyn ExternalHostSource,
        index: &AuthorizedExternalLexicalIndex,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ExternalChunkRef>, ProductError> {
        instance.require_rel_read(index.owner)?;
        self.inner
            .search_external_lexical_index(&instance.inner, source, &index.inner, query, limit)
            .map_err(Into::into)
    }

    pub fn hydrate_external_source(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
        expected: &[ExternalMessageDescriptor],
    ) -> Result<Vec<HydratedExternalMessage>, ProductError> {
        instance.require_rel_read(owner)?;
        self.inner
            .hydrate_external_source(&instance.inner, owner.into(), source, expected)
            .map_err(Into::into)
    }

    pub fn hydrate_external_chunk(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
        expected: &ExternalMessageDescriptor,
        reference: ExternalChunkRef,
    ) -> Result<HydratedExternalChunk, ProductError> {
        instance.require_rel_read(owner)?;
        self.inner
            .hydrate_external_chunk(&instance.inner, owner.into(), source, expected, reference)
            .map_err(Into::into)
    }

    pub fn prepare_external_memory(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
        expected: &[ExternalMessageDescriptor],
        endpoint: &dyn GeneralEndpoint,
    ) -> Result<Vec<ExternalMemoryProposal>, ProductError> {
        instance.require_rel_read(owner)?;
        let proposals = self.inner.prepare_external_memory(
            &instance.inner,
            owner.into(),
            source,
            expected,
            Endpoint(endpoint),
        )?;
        Ok(proposals
            .into_iter()
            .map(|proposal| ExternalMemoryProposal {
                candidate: proposal.candidate,
                authority: proposal.authority,
                content: proposal.content,
                grounding: proposal.grounding,
                source_time_ns: proposal.source_time_ns,
            })
            .collect())
    }

    /// Evaluate and publish an exact host window against the recorded cut.
    /// Existing receipts recover by window identity before invoking inference.
    pub fn publish_external_memory(
        &self,
        instance: &Instance,
        owner: OwnerId,
        source: &dyn ExternalHostSource,
        expected: &[ExternalMessageDescriptor],
        endpoint: &dyn GeneralEndpoint,
    ) -> Result<ExternalMemoryReceipt, ProductError> {
        instance.require(OwnerGrant::OperateRel(owner))?;
        self.inner
            .publish_external_memory(
                &instance.inner,
                owner.into(),
                source,
                expected,
                Endpoint(endpoint),
            )
            .map_err(Into::into)
    }
}

struct Endpoint<'a>(&'a dyn GeneralEndpoint);

impl GeneralEndpoint for Endpoint<'_> {
    fn model(&self) -> &str {
        self.0.model()
    }

    fn complete_json(
        &self,
        system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        schema: &serde_json::Value,
    ) -> Result<serde_json::Value, GeneralEndpointError> {
        self.0
            .complete_json(system_prompt, user_payload, schema_name, schema)
    }
}
