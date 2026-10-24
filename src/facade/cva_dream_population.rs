//! Historical prefix test probes over the canonical lower owner operation.
#[cfg(test)]
use crate::freshness_dream_journal::DirectBirthPrefix;
#[cfg(test)]
use crate::owner_lifecycle::{AcceptedExternalBirthCohort, AcceptedInsomniaBirthCohort};
#[cfg(test)]
use crate::{Archive, Container, Cva, CvaError, MemoryId};
#[cfg(test)]
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
impl Cva {
    pub(crate) fn external_birth_cohort_at_memory_prefix(
        &self,
        payload: &[u8],
        cut: u64,
    ) -> Result<Option<AcceptedExternalBirthCohort>, CvaError> {
        self.owner
            .external_birth_cohort_at_memory_prefix(payload, cut)
    }
    pub(crate) fn insomnia_birth_cohort_at_memory_prefix(
        &self,
        payload: &[u8],
        cut: u64,
    ) -> Result<Option<AcceptedInsomniaBirthCohort>, CvaError> {
        self.owner
            .insomnia_birth_cohort_at_memory_prefix(payload, cut)
    }
    pub(crate) fn direct_birth_ids_at_memory_prefix(
        &self,
        prefix: &DirectBirthPrefix,
        cut: u64,
    ) -> Result<BTreeSet<MemoryId>, CvaError> {
        self.owner.direct_birth_ids_at_memory_prefix(prefix, cut)
    }
    pub(crate) fn dream_population_at_prefix(
        &mut self,
        end: u64,
        cut: u64,
        source: MemoryId,
    ) -> Result<BTreeSet<MemoryId>, CvaError> {
        self.native_dream_population_at_prefix(end, cut, source)
    }
}
#[cfg(test)]
#[path = "dream_population_authority_tests.rs"]
mod authority_tests;
#[cfg(test)]
#[path = "external_birth_prefix_tests.rs"]
mod external_birth_prefix_tests;
#[cfg(test)]
#[path = "cva_freshness_insomnia_prefix_tests.rs"]
mod insomnia_prefix_tests;
#[cfg(test)]
#[path = "cva_dream_population_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "dream_population_proof_tests.rs"]
mod proof_tests;
