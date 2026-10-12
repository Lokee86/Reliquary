use super::{ReliquaryRuntimeHost, ReliquaryRuntimeHostError, operation};
use crate::dream_cooldown::unix_now_ns;
use crate::{InsomniaStats, MemoryStats, MemoryVectorStats};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeBackgroundStatus {
    pub insomnia: InsomniaStats,
    pub reliquary_memories: MemoryStats,
    pub reliquary_vectors: MemoryVectorStats,
    pub phylactery_memories: Option<MemoryStats>,
    pub phylactery_vectors: Option<MemoryVectorStats>,
    pub reliquary_vector_pending: usize,
    pub reliquary_semantic_pending: usize,
    pub phylactery_vector_pending: usize,
    pub reliquary_dream_pending: usize,
    pub phylactery_dream_pending: usize,
    pub reliquary_perception_pending: usize,
    pub phylactery_perception_pending: usize,
}

impl RuntimeBackgroundStatus {
    pub fn quiescent(self) -> bool {
        self.insomnia.pending == 0
            && self.insomnia.processing == 0
            && self.insomnia.failed == 0
            && self.insomnia.terminal == 0
            && self.reliquary_vector_pending == 0
            && self.reliquary_semantic_pending == 0
            && self.phylactery_vector_pending == 0
            && self.reliquary_dream_pending == 0
            && self.phylactery_dream_pending == 0
            && self.reliquary_perception_pending == 0
            && self.phylactery_perception_pending == 0
    }
}

impl ReliquaryRuntimeHost {
    pub fn background_status(&self) -> Result<RuntimeBackgroundStatus, ReliquaryRuntimeHostError> {
        let profiles = *self
            .active_execution()?
            .memory_profiles
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let (
            insomnia,
            reliquary_semantic_pending,
            reliquary_memories,
            reliquary_vectors,
            reliquary_vector_pending,
            reliquary_dream_pending,
        ) = {
            let runtime = self
                .active_execution()?
                .runtime
                .as_ref()
                .cloned()
                .ok_or_else(|| {
                    ReliquaryRuntimeHostError::Operation("Reliquary runtime is unavailable".into())
                })?;
            let mut runtime = runtime
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let ids = runtime.cva.memory_ids();
            let (vector_pending, dream_pending) =
                owner_pending(&mut runtime.cva, &ids, profiles.project)?;
            (
                runtime.cva.insomnia_stats(),
                runtime
                    .cva
                    .semantic_freshness_pending_turns()
                    .map_err(operation)?,
                runtime.cva.memory_stats(),
                runtime.cva.memory_vector_stats(),
                vector_pending,
                dream_pending,
            )
        };

        let (
            phylactery_memories,
            phylactery_vectors,
            phylactery_vector_pending,
            phylactery_dream_pending,
        ) = {
            let mut slot = self
                .active_execution()?
                .phylactery
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            match slot.as_mut() {
                Some(phy) => {
                    let ids = phy.memory_ids();
                    let (vector_pending, dream_pending) = owner_pending(phy, &ids, profiles.user)?;
                    (
                        Some(phy.memory_stats()),
                        Some(phy.memory_vector_stats()),
                        vector_pending,
                        dream_pending,
                    )
                }
                None => (None, None, 0, 0),
            }
        };

        let (reliquary_perception_pending, phylactery_perception_pending) = self
            .active_execution()?
            .perception_queue
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .counts();

        Ok(RuntimeBackgroundStatus {
            insomnia,
            reliquary_semantic_pending,
            reliquary_memories,
            reliquary_vectors,
            phylactery_memories,
            phylactery_vectors,
            reliquary_vector_pending,
            phylactery_vector_pending,
            reliquary_dream_pending,
            phylactery_dream_pending,
            reliquary_perception_pending,
            phylactery_perception_pending,
        })
    }
}

trait MemoryOwner {
    fn memory(&mut self, id: crate::MemoryId) -> Result<crate::Memory, String>;
    fn memory_body_id(&mut self, id: crate::MemoryId) -> Result<crate::MemoryBodyId, String>;
    fn memory_vector_location(
        &self,
        profile: crate::CompatibilityProfileId,
        body: crate::MemoryBodyId,
    ) -> Option<crate::MemoryVectorLocation>;
    fn dream_eligible_epoch(
        &mut self,
        id: crate::MemoryId,
        now_ns: i64,
    ) -> Result<Option<u64>, String>;
}

impl MemoryOwner for crate::Cva {
    fn memory(&mut self, id: crate::MemoryId) -> Result<crate::Memory, String> {
        crate::Cva::memory(self, id).map_err(|error| error.to_string())
    }

    fn memory_body_id(&mut self, id: crate::MemoryId) -> Result<crate::MemoryBodyId, String> {
        crate::Cva::memory_body_id(self, id).map_err(|error| error.to_string())
    }

    fn memory_vector_location(
        &self,
        profile: crate::CompatibilityProfileId,
        body: crate::MemoryBodyId,
    ) -> Option<crate::MemoryVectorLocation> {
        crate::Cva::memory_vector_location(self, profile, body)
    }

    fn dream_eligible_epoch(
        &mut self,
        id: crate::MemoryId,
        now_ns: i64,
    ) -> Result<Option<u64>, String> {
        crate::Cva::dream_eligible_epoch(self, id, now_ns).map_err(|error| error.to_string())
    }
}

impl MemoryOwner for crate::Phylactery {
    fn memory(&mut self, id: crate::MemoryId) -> Result<crate::Memory, String> {
        crate::Phylactery::memory(self, id).map_err(|error| error.to_string())
    }

    fn memory_body_id(&mut self, id: crate::MemoryId) -> Result<crate::MemoryBodyId, String> {
        crate::Phylactery::memory_body_id(self, id).map_err(|error| error.to_string())
    }

    fn memory_vector_location(
        &self,
        profile: crate::CompatibilityProfileId,
        body: crate::MemoryBodyId,
    ) -> Option<crate::MemoryVectorLocation> {
        crate::Phylactery::memory_vector_location(self, profile, body)
    }

    fn dream_eligible_epoch(
        &mut self,
        id: crate::MemoryId,
        now_ns: i64,
    ) -> Result<Option<u64>, String> {
        crate::Phylactery::dream_eligible_epoch(self, id, now_ns).map_err(|error| error.to_string())
    }
}

fn owner_pending<O: MemoryOwner>(
    owner: &mut O,
    ids: &[crate::MemoryId],
    profile: Option<crate::CompatibilityProfileId>,
) -> Result<(usize, usize), ReliquaryRuntimeHostError> {
    let mut vector_pending = 0;
    let mut dream_pending = 0;
    let now_ns = unix_now_ns();
    for id in ids {
        let memory = owner.memory(*id).map_err(operation)?;
        if memory.archived {
            continue;
        }
        let has_vector = match profile {
            Some(profile) => {
                let body = owner.memory_body_id(*id).map_err(operation)?;
                owner.memory_vector_location(profile, body).is_some()
            }
            None => false,
        };
        if !has_vector {
            vector_pending += 1;
        } else if owner
            .dream_eligible_epoch(*id, now_ns)
            .map_err(operation)?
            .is_some()
        {
            dream_pending += 1;
        }
    }
    Ok((vector_pending, dream_pending))
}
