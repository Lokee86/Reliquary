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
