//! Read-only Ego grades over the canonical Freshness owner; no reinforcement.
use crate::freshness_storage::FreshnessStore;
use crate::{FreshnessState, MemoryId};
use std::collections::BTreeSet;
pub(crate) struct EgoGradeRead<'a> {
    store: &'a FreshnessStore,
}
impl<'a> EgoGradeRead<'a> {
    pub(crate) fn new(store: &'a FreshnessStore) -> Self {
        Self { store }
    }
    pub(crate) fn grades(
        &self,
        ids: &[MemoryId],
        as_of: u64,
        head: u64,
    ) -> Result<Vec<(MemoryId, i16, FreshnessState)>, String> {
        if as_of > head {
            return Err("Ego Freshness cut is beyond the REL activity head".into());
        }
        let policy = self.store.policy();
        let mut seen = BTreeSet::new();
        let mut output = Vec::new();
        for id in ids {
            if !seen.insert(*id) {
                continue;
            }
            let record = if as_of == head {
                self.store.current_rewrite_record(*id, as_of)
            } else {
                self.store.record_at(*id, as_of)?
            };
            let Some(record) = record else { continue };
            let score = record
                .score_at_with_policy(as_of, &policy)
                .map_err(|e| e.to_string())?;
            let state = record
                .state_at_with_policy(as_of, &policy)
                .map_err(|e| e.to_string())?;
            output.push((*id, score, state));
        }
        Ok(output)
    }
}
