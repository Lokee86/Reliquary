//! Authenticated publication of an evaluated Dream batch.
use super::*;
#[path = "instance_import_postprocessing.rs"]
mod import_postprocessing;
use reliquary_application::{DreamOrganizationDraft, DreamOwnerWriter, commit_dream_batch};

impl Instance {
    pub fn publish_dream_batch(
        &self,
        owner: OwnerId,
        publication_id: [u8; 32],
        draft: &DreamOrganizationDraft,
        updated_at_ns: i64,
    ) -> Result<bool, GatewayError> {
        self.ensure_credential_active()?;
        if draft.owner_id() != owner.as_bytes() {
            return Err(GatewayError::Retrieval(
                "Dream proposal belongs to another owner".into(),
            ));
        }
        let result = if self.rel_grants.contains(&owner) {
            self.with_rel_writer(owner, |writer| {
                commit_dream_batch(
                    &mut DreamOwnerWriter::Rel(writer),
                    owner.as_bytes(),
                    publication_id,
                    draft,
                    updated_at_ns,
                )
            })?
        } else {
            self.with_entity_phy_writer(owner, |writer| {
                commit_dream_batch(
                    &mut DreamOwnerWriter::Phy(writer),
                    owner.as_bytes(),
                    publication_id,
                    draft,
                    updated_at_ns,
                )
            })?
        };
        result.map_err(|error| OwnerSessionError::Record(error.to_string()).into())
    }

    pub(crate) fn due_freshness_transitions(
        &self,
        owner: OwnerId,
        limit: usize,
    ) -> Result<Vec<reliquary_application::FreshnessTransition>, GatewayError> {
        let (snapshot, policy) = match self.reliquary(owner) {
            Ok(rel) => (rel.retrieval_snapshot(None)?, rel.freshness_policy()),
            Err(GatewayError::WriterUnavailable) => {
                self.with_writable_reliquary(owner, |rel| {
                    rel.retrieval_snapshot(None)
                        .map(|snapshot| (snapshot, rel.freshness_policy()))
                })??
            }
            Err(error) => return Err(error),
        };
        reliquary_application::due_freshness_transitions(&snapshot, &policy, limit)
            .map_err(GatewayError::Retrieval)
    }
}
