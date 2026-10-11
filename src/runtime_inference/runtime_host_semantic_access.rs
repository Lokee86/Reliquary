use super::{ReliquaryRuntimeHost, ReliquaryRuntimeHostError};

pub use crate::runtime_shared::{RuntimeSemanticOwner, RuntimeSemanticOwnerKind};

impl ReliquaryRuntimeHost {
    pub fn visible_semantic_owners(
        &self,
    ) -> Result<Vec<RuntimeSemanticOwner>, ReliquaryRuntimeHostError> {
        let active = self.active_rel_id().ok_or_else(|| {
            ReliquaryRuntimeHostError::Operation("Reliquary runtime has no active REL".into())
        })?;
        let mut owners = self
            .dependency_closure(&active)?
            .into_iter()
            .map(|owner_id| RuntimeSemanticOwner {
                owner_id,
                kind: RuntimeSemanticOwnerKind::Reliquary,
            })
            .collect::<Vec<_>>();
        if let Some(owner_id) = self.phylactery_owner_id()? {
            owners.push(RuntimeSemanticOwner {
                owner_id,
                kind: RuntimeSemanticOwnerKind::Phylactery,
            });
        }
        Ok(owners)
    }

    pub fn semantic_owner(
        &self,
        owner_id: &str,
    ) -> Result<RuntimeSemanticOwner, ReliquaryRuntimeHostError> {
        if self.execution_for(owner_id).is_ok() {
            return Ok(RuntimeSemanticOwner {
                owner_id: owner_id.to_owned(),
                kind: RuntimeSemanticOwnerKind::Reliquary,
            });
        }
        if self.phylactery_owner_id()?.as_deref() == Some(owner_id) {
            return Ok(RuntimeSemanticOwner {
                owner_id: owner_id.to_owned(),
                kind: RuntimeSemanticOwnerKind::Phylactery,
            });
        }
        Err(ReliquaryRuntimeHostError::Operation(format!(
            "semantic owner {owner_id} is not mounted"
        )))
    }

    pub fn visible_semantic_owner(
        &self,
        owner_id: &str,
    ) -> Result<RuntimeSemanticOwner, ReliquaryRuntimeHostError> {
        self.visible_semantic_owners()?
            .into_iter()
            .find(|owner| owner.owner_id == owner_id)
            .ok_or_else(|| {
                ReliquaryRuntimeHostError::Operation(format!(
                    "semantic owner {owner_id} is not visible from the active REL"
                ))
            })
    }
}
