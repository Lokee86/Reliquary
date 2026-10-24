use crate::{Cva, EntityAuditReport, Phylactery};

macro_rules! impl_owner_audit {
    ($owner:ty) => {
        impl $owner {
            pub fn audit_entities(&self) -> EntityAuditReport {
                crate::perception_entity_resolution_runtime::audit_rel(self)
            }
        }
    };
}

impl_owner_audit!(Cva);
impl Phylactery {
    pub fn audit_entities(&self) -> EntityAuditReport {
        crate::perception_entity_resolution_runtime::audit_phy(self)
    }
}
