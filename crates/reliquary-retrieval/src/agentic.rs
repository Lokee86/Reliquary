use reliquary_memory_core::MemoryId;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_AGENTIC_CANDIDATES: usize = 10;
pub const MAX_AGENTIC_QUERY_BYTES: usize = 512;

/// One turn's tentative reads and externally confirmed selections.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RetrievalTurn {
    query: Option<String>,
    searched: BTreeSet<MemoryId>,
    selected: BTreeSet<MemoryId>,
}

impl RetrievalTurn {
    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }
    pub fn searched(&self) -> &BTreeSet<MemoryId> {
        &self.searched
    }
    pub fn selected(&self) -> &BTreeSet<MemoryId> {
        &self.selected
    }

    pub fn begin_query(&mut self, query: &str) -> Result<(), &'static str> {
        self.query = Some(validate_query(query)?.to_owned());
        self.searched.clear();
        Ok(())
    }

    pub fn refine(&mut self, query: &str) -> Result<(), &'static str> {
        if self.query.is_none() {
            return Err("no active retrieval query");
        }
        self.query = Some(validate_query(query)?.to_owned());
        Ok(())
    }

    pub fn record_evaluated(&mut self, ids: impl IntoIterator<Item = MemoryId>) {
        self.searched.extend(ids);
    }

    pub fn confirm_selected(
        &mut self,
        ids: impl IntoIterator<Item = MemoryId>,
    ) -> Result<(), &'static str> {
        let ids: BTreeSet<_> = ids.into_iter().collect();
        if !ids.is_subset(&self.searched) {
            return Err("confirmed selection was not evaluated in this turn");
        }
        self.selected.extend(ids);
        Ok(())
    }
}

pub fn validate_query(query: &str) -> Result<&str, &'static str> {
    let query = query.trim();
    if query.is_empty() || query.len() > MAX_AGENTIC_QUERY_BYTES {
        return Err("retrieval query must contain 1 through 512 bytes");
    }
    Ok(query)
}

/// Preserve the raw union before excluding selected IDs and limiting offers.
pub fn freshness_candidates(
    scores: impl IntoIterator<Item = (MemoryId, i16)>,
    selected: &BTreeSet<MemoryId>,
) -> (BTreeSet<MemoryId>, Vec<MemoryId>) {
    let mut unique = BTreeMap::<MemoryId, i16>::new();
    for (id, score) in scores {
        unique
            .entry(id)
            .and_modify(|prior| *prior = (*prior).max(score))
            .or_insert(score);
    }
    let raw = unique.keys().copied().collect();
    let mut remaining: Vec<_> = unique
        .into_iter()
        .filter(|(id, _)| !selected.contains(id))
        .collect();
    remaining.sort_by(|(a_id, a), (b_id, b)| b.cmp(a).then_with(|| a_id.cmp(b_id)));
    remaining.truncate(MAX_AGENTIC_CANDIDATES);
    (raw, remaining.into_iter().map(|(id, _)| id).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_union_is_graded_before_limit_and_confirmed_ids_are_excluded() {
        let mut state = RetrievalTurn::default();
        state.begin_query("query").unwrap();
        let scores: Vec<_> = (0..40).map(|i| (MemoryId([i; 32]), i16::from(i))).collect();
        let (raw, offered) = freshness_candidates(scores.clone(), state.selected());
        assert_eq!(raw.len(), 40);
        assert_eq!(offered[0], MemoryId([39; 32]));
        assert!(state.selected().is_empty());
        state.record_evaluated(offered.clone());
        state.confirm_selected([offered[0]]).unwrap();
        state.refine("revised query").unwrap();
        assert_eq!(state.selected().len(), 1);
        state.begin_query("another query").unwrap();
        assert!(state.searched().is_empty());
        assert_eq!(state.selected().len(), 1);
        let (_, next) = freshness_candidates(scores, state.selected());
        assert_eq!(next[0], MemoryId([38; 32]));
        assert!(state.confirm_selected([MemoryId([99; 32])]).is_err());
    }
}
