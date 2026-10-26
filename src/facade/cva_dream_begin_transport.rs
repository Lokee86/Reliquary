//! Exact Dream Begin authority across physical-prefix and destination changes.
use crate::dream_begin_codec::{self, BeginPopulation, BeginRecord};
use crate::{Cva, CvaError, InitialDreamPass};
use reliquary_container::ObjectRef;

impl Cva {
    /// Resolve a source Begin at its own physical boundary, never at EOF.
    pub(crate) fn materialized_dream_begin(
        &mut self,
        object: ObjectRef,
        payload: &[u8],
    ) -> Result<Option<InitialDreamPass>, CvaError> {
        let owner = self
            .owner_uuid()
            .ok_or_else(|| CvaError::Freshness("Dream Begin transport requires an owner".into()))?;
        if !payload.starts_with(b"CVDREAM2") {
            return Ok(None);
        }

        let record = dream_begin_codec::decode(payload).map_err(CvaError::Freshness)?;
        if record.owner_uuid != owner {
            return Err(CvaError::Freshness(
                "Dream Begin source owner/format mismatch".into(),
            ));
        }
        let ids = match record.population {
            BeginPopulation::ExplicitSet(ids) => ids,
            BeginPopulation::CompactCut { memory_cut, digest } => {
                let ids =
                    self.dream_population_at_prefix(object.offset(), memory_cut, record.source_id)?;
                if crate::dream_population_digest::digest(&ids) != digest {
                    return Err(CvaError::Freshness(
                        "Dream Begin transport population digest differs".into(),
                    ));
                }
                ids
            }
        };
        Ok(Some(InitialDreamPass {
            pass_id: record.pass_id,
            source_id: record.source_id,
            first_revision: 1,
            turn: record.turn,
            accepted_settlement_turn: None,
            preexisting_ids: ids,
            graph_version_before: record.graph_version_before,
            roots: Default::default(),
        }))
    }

    /// Compact authority is legal only after proving exact set equality at the
    /// destination prefix. Manual subsets and changed birth order stay literal.
    pub(crate) fn current_dream_begin_payload(
        &mut self,
        pass: &InitialDreamPass,
    ) -> Result<Vec<u8>, CvaError> {
        if pass.first_revision != 1
            || !pass.roots.is_empty()
            || pass.accepted_settlement_turn.is_some()
        {
            return Err(CvaError::Freshness(
                "Dream Begin transport requires an initial pass".into(),
            ));
        }
        let owner = self.owner_uuid().ok_or_else(|| {
            CvaError::Freshness("Dream Begin destination requires an owner".into())
        })?;
        let mut population = BeginPopulation::ExplicitSet(pass.preexisting_ids.clone());
        if !pass.preexisting_ids.is_empty()
            && self.memories.first_global_version(pass.source_id).is_some()
        {
            let end = self.container.physical_length()?;
            let cut = self.memory_version();
            let ids = self.dream_population_at_prefix(end, cut, pass.source_id)?;
            if ids == pass.preexisting_ids {
                population = BeginPopulation::CompactCut {
                    memory_cut: cut,
                    digest: crate::dream_population_digest::digest(&ids),
                };
            }
        }
        dream_begin_codec::encode(&BeginRecord {
            owner_uuid: owner,
            pass_id: pass.pass_id.clone(),
            source_id: pass.source_id,
            turn: pass.turn,
            graph_version_before: pass.graph_version_before,
            population,
        })
        .map_err(CvaError::Freshness)
    }
}
