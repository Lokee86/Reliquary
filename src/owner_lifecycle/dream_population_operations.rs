use crate::MemoryId;
use crate::freshness_dream_journal::DirectBirthPrefix;
use crate::owner_lifecycle::{
    AcceptedExternalBirthCohort, AcceptedExternalBirthMember, AcceptedInsomniaBirthCohort,
    AcceptedInsomniaBirthMember, DreamPopulationPrefix, direct_birth_ids,
    validate_external_birth_cohort, validate_insomnia_birth_cohort,
};
use crate::owner_lifecycle::{OwnerError, ReliquaryOwner};
use std::collections::{BTreeMap, BTreeSet};

impl ReliquaryOwner {
    pub(crate) fn external_birth_cohort_at_memory_prefix(
        &self,
        payload: &[u8],
        memory_cut: u64,
    ) -> Result<Option<AcceptedExternalBirthCohort>, OwnerError> {
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
    ) -> Result<Option<AcceptedInsomniaBirthCohort>, OwnerError> {
        let first = self
            .memories
            .first_publication_prefix(memory_cut)?
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let Some(completion) = crate::insomnia::completion::decode_completion(payload)
            .map_err(|message| OwnerError::Freshness(message.into()))?
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
    ) -> Result<BTreeSet<MemoryId>, OwnerError> {
        if prefix.owner_uuid().is_none() || prefix.owner_uuid() != self.owner_uuid() {
            return Err(OwnerError::Freshness(
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
    ) -> Result<BTreeSet<MemoryId>, OwnerError> {
        let owner = self.owner_uuid().ok_or_else(|| {
            OwnerError::Freshness("Dream population requires a versioned owner".into())
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
