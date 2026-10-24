//! Cva adapters for the bounded, rebuildable Freshness due dispatcher.
use crate::Cva;
use crate::freshness_due::{FreshnessDue, FreshnessReversal};

/// At-least-once derived lifecycle notification. Reopen rebuilds pending crossings
/// from durable score records; consumers should deduplicate by memory/boundary/state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreshnessTransitionNotice {
    Crossing(FreshnessDue),
    Reversal(FreshnessReversal),
}

impl Cva {
    /// Drains at most limit notices in deterministic Memory-ID order.
    /// A pending reversal follows that Memory's older boundary notices.
    pub fn drain_freshness_notifications(
        &mut self,
        limit: usize,
    ) -> Vec<FreshnessTransitionNotice> {
        self.freshness_on_accepted_activity(self.owner.archive.rel_turn_count());
        let mut result = Vec::with_capacity(limit.min(64));
        while result.len() < limit {
            let Some((id, mut state)) = self.freshness_notifications.pop_first() else {
                break;
            };
            if let Some(crossing) = state.crossings.pop_front() {
                result.push(FreshnessTransitionNotice::Crossing(crossing));
            } else if let Some(reversal) = state.reversal.take() {
                result.push(FreshnessTransitionNotice::Reversal(reversal));
            }
            if !state.is_empty() {
                self.freshness_notifications.insert(id, state);
            }
        }
        result
    }
}
