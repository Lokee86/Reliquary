use super::{FreshnessStore, accepted_use_events};
use crate::projection::FreshnessEventCut;

impl FreshnessStore {
    /// Return the complete semantic boundary through one REL turn, including
    /// accepted-use events that are represented by an atomic grouped receipt.
    pub fn cut_through_turn(&self, rel_turn: u64) -> FreshnessEventCut {
        let event_id_inclusive = self
            .events
            .values()
            .filter(|event| event.accepted_turn == rel_turn)
            .map(|event| event.event_id.clone())
            .chain(
                self.uses
                    .values()
                    .filter(|(receipt, _)| receipt.accepted_turn == rel_turn)
                    .flat_map(|(receipt, proof)| {
                        accepted_use_events(
                            self.owner_uuid.unwrap_or([0; 16]),
                            receipt,
                            *proof,
                            &self.policy,
                        )
                        .into_iter()
                        .map(|event| event.event_id)
                    }),
            )
            .max();
        FreshnessEventCut {
            rel_turn,
            event_id_inclusive,
        }
    }

    /// Validate that a selected event boundary names a real semantic event and
    /// does not split the synthetic events of one atomic accepted-use receipt.
    pub fn is_authoritative_cut(&self, cut: &FreshnessEventCut) -> bool {
        let Some(limit) = cut.event_id_inclusive.as_ref() else {
            return true;
        };
        let explicit = self
            .events
            .get(limit)
            .is_some_and(|event| event.accepted_turn == cut.rel_turn);
        let mut known = explicit;
        for (receipt, proof) in self.uses.values() {
            if receipt.accepted_turn != cut.rel_turn {
                continue;
            }
            let mut ids = accepted_use_events(
                self.owner_uuid.unwrap_or([0; 16]),
                receipt,
                *proof,
                &self.policy,
            )
            .into_iter()
            .map(|event| event.event_id)
            .collect::<Vec<_>>();
            ids.sort();
            if ids.iter().any(|id| id == limit) {
                known = true;
            }
            if ids.first().is_some_and(|first| first <= limit)
                && ids.last().is_some_and(|last| limit < last)
            {
                return false;
            }
        }
        known
    }
}
