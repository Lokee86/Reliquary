use crate::{Cva, HistoryGenerationId, Phylactery, RewriteGenesis};

impl Cva {
    pub fn history_generation(&self) -> HistoryGenerationId {
        self.rewrite_lineage.generation()
    }
    pub fn rewrite_genesis(&self) -> Option<&RewriteGenesis> {
        self.rewrite_lineage.rewrite_genesis()
    }
}

impl Phylactery {
    pub fn history_generation(&self) -> HistoryGenerationId {
        self.rewrite_lineage.generation()
    }
    pub fn rewrite_genesis(&self) -> Option<&RewriteGenesis> {
        self.rewrite_lineage.rewrite_genesis()
    }
}
