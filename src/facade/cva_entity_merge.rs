use crate::{
    Cva, CvaError, EntityId, EntityMergeOutcome, MemoryEntityMentionKey, Phylactery,
    PhylacteryError,
};

macro_rules! impl_entity_merge {
    ($owner:ty, $error:ty) => {
        impl $owner {
            pub fn merge_entities(
                &mut self,
                survivor_id: EntityId,
                retired_id: EntityId,
                now_ns: i64,
            ) -> Result<EntityMergeOutcome, $error> {
                crate::perception_entity_merge_runtime::merge_rel(
                    self,
                    survivor_id,
                    retired_id,
                    now_ns,
                )
            }

            pub fn retarget_entity_resolution(
                &mut self,
                key: MemoryEntityMentionKey,
                from: EntityId,
                to: EntityId,
                now_ns: i64,
            ) -> Result<bool, $error> {
                crate::perception_entity_merge_runtime::retarget_rel(self, key, from, to, now_ns)
            }
        }
    };
}

impl_entity_merge!(Cva, CvaError);
impl Phylactery {
    pub fn merge_entities(
        &mut self,
        survivor_id: EntityId,
        retired_id: EntityId,
        now_ns: i64,
    ) -> Result<EntityMergeOutcome, PhylacteryError> {
        crate::perception_entity_merge_runtime::merge_phy(self, survivor_id, retired_id, now_ns)
    }

    pub fn retarget_entity_resolution(
        &mut self,
        key: MemoryEntityMentionKey,
        from: EntityId,
        to: EntityId,
        now_ns: i64,
    ) -> Result<bool, PhylacteryError> {
        crate::perception_entity_merge_runtime::retarget_phy(self, key, from, to, now_ns)
    }
}
