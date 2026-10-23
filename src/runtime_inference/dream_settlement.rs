use crate::{MemoryId, PropagationEvent};
use std::collections::BTreeMap;

/// Frozen inputs for propagation. Runtime hosts release their owner mutex before
/// evaluating this value; only preparation/commit touch the Container.
pub(crate) struct InitialSettlementWork {
    pub(crate) pass_id: String,
    pub(crate) active_admission: bool,
    pub(crate) proof: crate::FreshnessEventProof,
    pub(crate) event: Option<PropagationEvent>,
    pub(crate) adjacency: crate::freshness::MemoryAdjacency,
    pub(crate) memberships: crate::freshness::CommunityMembership,
    pub(crate) policy: crate::freshness::FreshnessPolicy,
}
pub(crate) struct InitialSettlementResult {
    pub(crate) pass_id: String,
    pub(crate) active_admission: bool,
    pub(crate) proof: crate::FreshnessEventProof,
    pub(crate) effects: BTreeMap<MemoryId, i16>,
}
impl InitialSettlementWork {
    pub(crate) fn evaluate(
        self,
        workers: usize,
    ) -> Result<InitialSettlementResult, crate::CvaError> {
        let _trace = crate::freshness_trace::scope("initial_dream_evaluate");
        let effects = crate::freshness_trace::measure("dream.propagation", || match self.event {
            Some(event) => crate::freshness::propagate_with_policy(
                &event,
                &self.adjacency,
                &self.memberships,
                workers.max(1),
                &self.policy,
            )
            .map_err(|error| crate::CvaError::Freshness(error.to_string())),
            None => Ok(BTreeMap::new()),
        })?;
        Ok(InitialSettlementResult {
            pass_id: self.pass_id,
            active_admission: self.active_admission,
            proof: self.proof,
            effects,
        })
    }
}
