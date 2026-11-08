use super::*;
use reliquary_graph::{CommunityRename, CommunityRenameReceipt};

impl RelWriter {
    /// Commit one exact membership name or replay its durable accepted receipt.
    pub fn rename_management_community(
        &mut self,
        request: CommunityRename,
    ) -> Result<Option<CommunityRenameReceipt>, OwnerSessionError> {
        self.ensure_write_ready()?;
        let result = self.session.common.communities.rename(
            &mut self.container,
            &self.session.common.graph,
            request,
        );
        match result {
            Ok(receipt) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(Some(receipt))
            }
            Err(GraphError::UnknownCommunity) => Ok(None),
            Err(GraphError::StaleCommunity) => Err(OwnerSessionError::ManagementStale),
            Err(GraphError::CommunityRequestConflict) => Err(OwnerSessionError::ManagementConflict),
            Err(error) => {
                if matches!(error, GraphError::Container(_)) {
                    self.session.write_poisoned = true;
                }
                Err(error.into())
            }
        }
    }
}
