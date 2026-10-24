use crate::{Cva, FreshnessState, MemoryId, PropagationEvent};
use std::collections::BTreeSet;

/// Trusted host attestation that exact Memory content entered an external context.
/// The controller constructs this from its verified accepted-content manifest;
/// untrusted client assertions, search results, internal processing, and low-level
/// reads must never construct or authorize a receipt. This value is not itself
/// cryptographic proof or an authenticated client API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedMemoryUseReceipt {
    pub owner_uuid: [u8; 16],
    pub accepted_turn: u64,
    pub use_id: String,
    pub memories: Vec<MemoryId>,
}

/// Graded REL-local signal for Ego context selection. Consumers combine it with
/// coverage and hard-keep lanes; it does not filter the Memory Web.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EgoMemoryFreshness {
    pub memory_id: MemoryId,
    pub score: i16,
    pub state: FreshnessState,
}

/// An owner-local routine-context candidate and its already-computed token cost.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EgoFreshnessCandidate {
    pub memory_id: MemoryId,
    pub context_cost: usize,
}

impl Cva {
    /// Apply existing access reinforcement for confirmed external Memory consumption.
    ///
    /// This is the sole trusted embedding-host write boundary, not a client receipt
    /// endpoint. The host must verify actual content acceptance, construct the exact
    /// included IDs, and retain a stable use identity and accepted REL turn for retry.
    /// Candidate delivery, metadata inspection, internal Ego/Dream/Insomnia work,
    /// failed delivery, and cancellation never authorize this call. Owner/turn and
    /// replay validation below cannot establish the host's consumption evidence.
    /// Identical retries apply nothing; conflicting identity reuse fails.
    pub fn record_accepted_memory_use(
        &mut self,
        receipt: AcceptedMemoryUseReceipt,
        workers: usize,
    ) -> Result<usize, crate::CvaError> {
        crate::freshness_operations_runtime::record_accepted_memory_use(
            self,
            receipt.owner_uuid,
            receipt.use_id,
            receipt.accepted_turn,
            receipt.memories,
            workers,
        )
    }
    /// Return current/historical signed Freshness grades without access reinforcement,
    /// activity advancement, or receipt creation. Lazy decay is a derived read.
    /// Does not exclude low-scoring, archived, or hard-kept Memories.
    pub fn ego_memory_freshness(
        &self,
        memory_ids: &[MemoryId],
        as_of_turn: u64,
    ) -> Result<Vec<EgoMemoryFreshness>, crate::CvaError> {
        crate::freshness_operations_runtime::ego_memory_grades(self, memory_ids, as_of_turn).map(
            |grades| {
                grades
                    .into_iter()
                    .map(|(memory_id, score, state)| EgoMemoryFreshness {
                        memory_id,
                        score,
                        state,
                    })
                    .collect()
            },
        )
    }

    /// Choose routine context under a token budget. Hard-keeps survive even if
    /// they exceed budget; all candidates are retained when the full Web fits.
    pub fn select_ego_routine_context(
        &self,
        candidates: &[EgoFreshnessCandidate],
        budget: usize,
        hard_keep: &BTreeSet<MemoryId>,
        as_of_turn: u64,
    ) -> Result<Vec<MemoryId>, crate::CvaError> {
        let candidates = candidates
            .iter()
            .map(|candidate| (candidate.memory_id, candidate.context_cost))
            .collect::<Vec<_>>();
        crate::freshness_operations_runtime::select_ego_routine_context(
            self,
            &candidates,
            budget,
            hard_keep,
            as_of_turn,
        )
    }
}
