use crate::{Cva, HistoryGenerationId, Phylactery, RewriteGenesis};

impl Cva {
    pub fn history_generation(&self) -> HistoryGenerationId {
        self.runtime_history_generation()
    }
    pub fn rewrite_genesis(&self) -> Option<&RewriteGenesis> {
        self.runtime_rewrite_genesis()
    }
}

impl Phylactery {
    pub fn history_generation(&self) -> HistoryGenerationId {
        self.runtime_history_generation()
    }
    pub fn rewrite_genesis(&self) -> Option<&RewriteGenesis> {
        self.runtime_rewrite_genesis()
    }
}
