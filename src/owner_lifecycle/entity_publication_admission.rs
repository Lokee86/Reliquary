use crate::{EntityDraft, EntityError};

/// REL/PHY principal exclusion at the native Entity publication boundary.
pub(crate) fn validate_entity_publication(
    draft: &EntityDraft,
    reject_principal: bool,
) -> Result<(), EntityError> {
    if reject_principal
        && draft.kind.trim() == crate::entity_principal_identity::PRINCIPAL_ENTITY_KIND
    {
        return Err(EntityError::InvalidField("Phylactery principal Entity"));
    }
    Ok(())
}
