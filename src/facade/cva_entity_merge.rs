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
                crate::entity_merge::merge_entities_parts::<$error>(
                    &mut self.container,
                    &self.memories,
                    &mut self.entities,
                    &mut self.entity_resolutions,
                    &mut self.graph,
                    &mut self.relationships,
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
                crate::entity_merge::retarget_entity_resolution_parts::<$error>(
                    &mut self.container,
                    &self.memories,
                    &self.entities,
                    &mut self.entity_resolutions,
                    &mut self.graph,
                    key,
                    from,
                    to,
                    now_ns,
                )
            }
        }
    };
}

impl_entity_merge!(Cva, CvaError);
impl_entity_merge!(Phylactery, PhylacteryError);
