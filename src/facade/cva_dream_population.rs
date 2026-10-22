use crate::freshness_dream_journal::DirectBirthPrefix;
use crate::owner_lifecycle::{
    AcceptedExternalBirthCohort, AcceptedExternalBirthMember, AcceptedInsomniaBirthCohort,
    AcceptedInsomniaBirthMember, DreamPopulationPrefix, direct_birth_ids,
    validate_external_birth_cohort, validate_insomnia_birth_cohort,
};
use crate::{Archive, Container, Cva, CvaError, MemoryId};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
#[path = "external_birth_prefix_tests.rs"]
mod external_birth_prefix_tests;
#[cfg(test)]
#[path = "cva_freshness_insomnia_prefix_tests.rs"]
mod insomnia_prefix_tests;
#[cfg(test)]
#[path = "cva_dream_population_tests.rs"]
mod tests;

impl Cva {
    pub(crate) fn external_birth_cohort_at_memory_prefix(
        &self,
        payload: &[u8],
        memory_cut: u64,
    ) -> Result<Option<AcceptedExternalBirthCohort>, CvaError> {
        if memory_cut > self.memories.memory_version() {
            return Err(crate::MemoryError::InvalidVersion.into());
        }
        let Some(publication) = crate::memory_external_publication::decode(payload)? else {
            return Ok(None);
        };
        let Some(mut cohort) =
            validate_external_birth_cohort(&self.archive, &self.memories, &publication)?
        else {
            return Ok(None);
        };
        let first = self
            .memories
            .first_publication_prefix(memory_cut)?
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        cohort.members.retain(|member| {
            member.memory_version <= memory_cut
                && first.get(&member.id) == Some(&member.global_version)
        });
        Ok(Some(cohort))
    }

    pub(crate) fn insomnia_birth_cohort_at_memory_prefix(
        &self,
        payload: &[u8],
        memory_cut: u64,
    ) -> Result<Option<AcceptedInsomniaBirthCohort>, CvaError> {
        let first = self
            .memories
            .first_publication_prefix(memory_cut)?
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let Some(completion) = crate::insomnia::completion::decode_completion(payload)
            .map_err(|message| CvaError::Freshness(message.into()))?
        else {
            return Ok(None);
        };
        let Some(mut cohort) =
            validate_insomnia_birth_cohort(&self.archive, &self.memories, &completion)?
        else {
            return Ok(None);
        };
        cohort.members.retain(|member| {
            member.memory_version <= memory_cut
                && first.get(&member.id) == Some(&member.global_version)
        });
        Ok(Some(cohort))
    }

    pub(crate) fn direct_birth_ids_at_memory_prefix(
        &self,
        prefix: &DirectBirthPrefix,
        memory_cut: u64,
    ) -> Result<BTreeSet<MemoryId>, CvaError> {
        if prefix.owner_uuid().is_none() || prefix.owner_uuid() != self.owner_uuid() {
            return Err(CvaError::Freshness(
                "Dream direct-birth prefix belongs to another owner".into(),
            ));
        }
        let first = self
            .memories
            .first_publication_prefix(memory_cut)?
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        Ok(direct_birth_ids(prefix, &self.memories, &first))
    }

    /// Reconstruct publication births at a physical Begin boundary using the
    /// live container's bounded scanner; this wrapper performs no EOF scan.
    pub(crate) fn dream_population_at_prefix(
        &mut self,
        prefix_end: u64,
        memory_cut: u64,
        source_id: MemoryId,
    ) -> Result<BTreeSet<MemoryId>, CvaError> {
        let owner = self.owner_uuid().ok_or_else(|| {
            CvaError::Freshness("Dream population requires a versioned owner".into())
        })?;
        let state = DreamPopulationPrefix::scan_through(
            &mut self.container,
            &self.archive,
            &self.memories,
            owner,
            prefix_end,
        )?;
        state.materialize(&self.memories, Some(owner), memory_cut, source_id)
    }
}

#[cfg(test)]
#[path = "dream_population_authority_tests.rs"]
mod authority_tests;

#[cfg(test)]
#[path = "dream_population_proof_tests.rs"]
mod proof_tests;
