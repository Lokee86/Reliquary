//! Shared entry points for semantic fragment facts and Freshness acceptance.

#[cfg(test)]
use crate::Node;
#[cfg(test)]
pub(crate) use crate::owner_lifecycle::semantic_source::SemanticSourceVector;
pub(crate) use crate::owner_lifecycle::semantic_source::{
    AcceptedSemanticSource, SemanticSourceReadiness,
};
pub(crate) use crate::public_semantic_operations::SemanticCandidateSearch;
#[cfg(test)]
pub(crate) use crate::public_semantic_operations::freshness_policy_digest;
#[cfg(test)]
pub(crate) use crate::public_semantic_operations::{
    SemanticTargetSnapshot, plan_eligible_semantic_targets,
};
use crate::{Cva, CvaError, MemoryId};
#[cfg(test)]
use sha2::{Digest, Sha256};

impl Cva {
    #[cfg(test)]
    pub(crate) fn apply_accepted_semantic_reinforcement(
        &mut self,
        node: &Node,
        source_memory_cohort: &std::collections::BTreeSet<MemoryId>,
    ) -> Result<usize, CvaError> {
        self.runtime_apply_accepted_semantic_reinforcement(node, source_memory_cohort)
    }

    pub(crate) fn apply_prepared_semantic_reinforcement(
        &mut self,
        source: AcceptedSemanticSource,
        source_memory_cohort: &std::collections::BTreeSet<MemoryId>,
    ) -> Result<usize, CvaError> {
        self.runtime_apply_prepared_semantic_reinforcement(source, source_memory_cohort)
    }

    pub(crate) fn search_eligible_semantic_targets(
        &mut self,
        source: &AcceptedSemanticSource,
        source_memory_cohort: &std::collections::BTreeSet<MemoryId>,
    ) -> Result<SemanticCandidateSearch, CvaError> {
        self.runtime_search_eligible_semantic_targets(source, source_memory_cohort)
    }
}

#[cfg(test)]
fn semantic_target_digest(effects: &std::collections::BTreeMap<MemoryId, i16>) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"reliquary:semantic-target-evidence:v1\0");
    for (memory_id, bonus) in effects {
        digest.update(memory_id.0);
        digest.update(bonus.to_le_bytes());
    }
    digest.finalize().into()
}

#[cfg(test)]
#[path = "cva_semantic_source_tests.rs"]
mod tests;
