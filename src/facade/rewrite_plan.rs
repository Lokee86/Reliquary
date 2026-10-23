//! Supported public rewrite planners delegate to immutable Maintenance inputs.
use crate::rewrite::{RewriteError, RewritePlan, RewriteRequest};
use crate::{Cva, Phylactery};

impl Cva {
    pub fn plan_rewrite(&mut self, request: &RewriteRequest) -> Result<RewritePlan, RewriteError> {
        crate::maintenance::rewrite::ReliquaryRewritePlanSource {
            container: &self.container,
            memories: &self.memories,
            compatibility_profiles: &self.compatibility_profiles,
            memory_vectors: &self.memory_vectors,
            rewrite_lineage: &self.rewrite_lineage,
            entities: &self.entities,
            relationships: &self.relationships,
            graph: &self.graph,
            archive: &self.archive,
            freshness: &self.freshness,
        }
        .plan_rewrite(request)
    }
}

impl Phylactery {
    pub fn plan_rewrite(&mut self, request: &RewriteRequest) -> Result<RewritePlan, RewriteError> {
        crate::maintenance::rewrite::PhylacteryRewritePlanSource {
            container: &self.container,
            memories: &self.memories,
            compatibility_profiles: &self.compatibility_profiles,
            memory_vectors: &self.memory_vectors,
            rewrite_lineage: &self.rewrite_lineage,
            entities: &self.entities,
            relationships: &self.relationships,
            graph: &self.graph,
        }
        .plan_rewrite(request)
    }
}
