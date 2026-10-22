use crate::external_index::model::{
    EXTERNAL_SOURCE_CONTRACT, ExternalChangeBatch, ExternalMessageDescriptor,
    ExternalSnapshotManifest, ExternalSourceError, HydratedExternalMessage,
};
/// Implementations must authorize this verified principal for *every* feed,
/// manifest and hydration call. A REL grant never grants access to host text.
/// Calls run outside semantic locks and admission leases. Errors contain no bodies.
pub trait ExternalConversationSource: Send + Sync {
    fn source_namespace(&self) -> &str;
    fn authorize(&self, principal_id: &str) -> Result<(), ExternalSourceError>;
    fn contract_version(&self) -> u32 {
        EXTERNAL_SOURCE_CONTRACT
    }
    fn changes_after(
        &self,
        principal_id: &str,
        cursor: u64,
        limit: usize,
    ) -> Result<ExternalChangeBatch, ExternalSourceError>;
    fn snapshot_manifest(
        &self,
        principal_id: &str,
    ) -> Result<ExternalSnapshotManifest, ExternalSourceError>;
    fn hydrate(
        &self,
        principal_id: &str,
        messages: &[ExternalMessageDescriptor],
    ) -> Result<Vec<HydratedExternalMessage>, ExternalSourceError>;
}
