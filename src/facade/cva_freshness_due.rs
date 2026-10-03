//! Cva adapters for the bounded, rebuildable Freshness due dispatcher.
use crate::freshness_due::{FreshnessDue, FreshnessReversal};
use crate::{Cva, CvaError, MemoryId};
use std::collections::{BTreeSet, VecDeque};

const MAX_DUE_PER_ACTIVITY: usize = 64;
const MAX_CROSSINGS_PER_MEMORY: usize = 2;

/// At-least-once derived lifecycle notification. Reopen rebuilds pending crossings
/// from durable score records; consumers should deduplicate by memory/boundary/state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreshnessTransitionNotice {
    Crossing(FreshnessDue),
    Reversal(FreshnessReversal),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FreshnessNotificationState {
    crossings: VecDeque<FreshnessDue>,
    reversal: Option<FreshnessReversal>,
}

impl FreshnessNotificationState {
    fn is_empty(&self) -> bool {
        self.crossings.is_empty() && self.reversal.is_none()
    }
}

impl Cva {
    /// Refreshes only affected Memory schedules after durable admission or event commit.
    /// Due crossings are processed first so an event cannot erase a boundary already reached.
    pub(crate) fn freshness_refresh_due(
        &mut self,
        ids: impl IntoIterator<Item = MemoryId>,
        accepted_turn: u64,
    ) -> Result<(), CvaError> {
        let schedule_turn = self.rel_turn_count().max(accepted_turn);
        let affected: BTreeSet<_> = ids.into_iter().collect();
        // Capture exact old boundaries for event-affected IDs before their new score
        // postimages replace the dispatcher snapshot. This is proportional to the event.
        for id in &affected {
            let due = self
                .freshness_due
                .take_due_for(*id, schedule_turn)
                .map_err(|error| CvaError::Freshness(error.to_string()))?;
            for crossing in due {
                self.buffer_freshness_crossing(crossing);
            }
        }
        self.process_freshness_due(schedule_turn);
        let policy = self.freshness.policy();
        for id in affected {
            let Some(record) = self.freshness_record(id) else {
                continue;
            };
            let refreshed = self
                .freshness_due
                .refresh_with_transition(id, &record, schedule_turn, &policy)
                .map_err(|error| CvaError::Freshness(error.to_string()))?;
            if let Some(reversal) = refreshed.reversal {
                self.freshness_notifications.entry(id).or_default().reversal = Some(reversal);
            }
        }
        Ok(())
    }

    /// Drains at most limit notices in deterministic Memory-ID order.
    /// A pending reversal follows that Memory's older boundary notices.
    pub fn drain_freshness_notifications(
        &mut self,
        limit: usize,
    ) -> Vec<FreshnessTransitionNotice> {
        self.process_freshness_due(self.rel_turn_count());
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

    /// Called immediately after accepted REL activity. Reads remain lazy and pure;
    /// this advances only the volatile notification index and preserves backlog.
    pub(crate) fn freshness_on_accepted_activity(&mut self, accepted_turn: u64) {
        self.process_freshness_due(accepted_turn);
    }

    fn process_freshness_due(&mut self, accepted_turn: u64) {
        for _ in 0..MAX_DUE_PER_ACTIVITY {
            let Some(due) = self
                .freshness_due
                .drain_due(accepted_turn, 1)
                .into_iter()
                .next()
            else {
                break;
            };
            if self.freshness_due.advance(due).is_err() {
                let _ = self.freshness_due.defer(due);
                break;
            }
            self.buffer_freshness_crossing(due);
        }
    }

    fn buffer_freshness_crossing(&mut self, due: FreshnessDue) {
        let state = self.freshness_notifications.entry(due.memory).or_default();
        if state.crossings.len() < MAX_CROSSINGS_PER_MEMORY {
            state.crossings.push_back(due);
        } else {
            // Keep the earliest outstanding edge plus the latest canonical edge.
            // Derived notices are at-least-once/coalescing hints, not durable history.
            if let Some(latest) = state.crossings.back_mut() {
                *latest = due;
            }
        }
    }
}
