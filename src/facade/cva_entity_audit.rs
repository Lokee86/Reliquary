use crate::entity_audit::EntityAuditContext;
use crate::{Cva, EntityAuditReport, Phylactery};

macro_rules! impl_owner_audit {
    ($owner:ty) => {
        impl $owner {
            pub fn audit_entities(&self) -> EntityAuditReport {
                let context = EntityAuditContext {
                    entities: &self.entities,
                    memories: &self.memories,
                    graph: &self.graph,
                    resolutions: &self.entity_resolutions,
                };
                crate::entity_audit::audit_owner(&context)
            }
        }
    };
}

impl_owner_audit!(Cva);
impl_owner_audit!(Phylactery);
