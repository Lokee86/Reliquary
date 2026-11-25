use super::*;
impl DreamReadIndex {
    /// Reuse owner-local index buckets for a bounded evidence lookup.
    pub(in crate::session) fn lexical_candidates(
        &self,
        query: &str,
        limit: usize,
        eligible: impl Fn(MemoryId) -> bool,
    ) -> Vec<MemoryId> {
        let mut terms = super::tokens(query)
            .into_iter()
            .filter_map(|term| self.terms.get(&term).map(|ids| (term, ids)))
            .collect::<Vec<_>>();
        terms.sort_by(|(a, a_ids), (b, b_ids)| (a_ids.len(), a).cmp(&(b_ids.len(), b)));
        let mut scores = BTreeMap::<MemoryId, usize>::new();
        for (_, ids) in terms.into_iter().take(16) {
            for id in ids.iter().filter(|id| eligible(**id)).take(32) {
                *scores.entry(*id).or_default() += 1;
            }
        }
        let mut matches = scores.into_iter().collect::<Vec<_>>();
        matches.sort_unstable_by_key(|&(id, score)| (std::cmp::Reverse(score), id));
        matches.into_iter().take(limit).map(|(id, _)| id).collect()
    }

    pub fn query(
        &self,
        source: MemoryId,
        version: u64,
        time: Option<i64>,
        profile: CompatibilityProfileId,
        row: &[f32],
        eligible: impl Fn(MemoryId) -> bool,
    ) -> DreamCandidatePool {
        let mut ids = BTreeSet::new();
        let mut examined = 0;
        if self.active.len() <= 128 {
            ids.extend(self.active.iter().copied().filter(|&id| eligible(id)));
            examined = self.active.len();
        } else {
            let mut semantic = BTreeSet::new();
            for table in 0..4 {
                let code = locality(row, table);
                for probe in std::iter::once(code).chain((0..12).map(|bit| code ^ (1 << bit))) {
                    if let Some(bucket) = self.semantic.get(&(profile.0, table, probe)) {
                        for id in bucket.iter().filter(|id| eligible(**id)).take(16) {
                            examined += 1;
                            if *id != source {
                                semantic.insert(*id);
                            }
                            if semantic.len() == 64 {
                                break;
                            }
                        }
                    }
                    if semantic.len() == 64 {
                        break;
                    }
                }
                if semantic.len() == 64 {
                    break;
                }
            }
            ids.extend(semantic);
            let mut lexical = BTreeSet::new();
            let mut terms: Vec<_> = self
                .source_terms
                .get(&source)
                .into_iter()
                .flatten()
                .cloned()
                .filter_map(|term| self.terms.get(&term).map(|set| (set.len(), term, set)))
                .collect();
            terms.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
            for (_, _, bucket) in terms.into_iter().take(16) {
                for id in bucket.iter().filter(|id| eligible(**id)).take(4) {
                    examined += 1;
                    if *id != source {
                        lexical.insert(*id);
                    }
                    if lexical.len() == 32 {
                        break;
                    }
                }
                if lexical.len() == 32 {
                    break;
                }
            }
            ids.extend(lexical);
            if let Some(time) = time {
                use std::ops::Bound::{Excluded, Unbounded};
                for (_, id) in self
                    .times
                    .range(..(time, source))
                    .rev()
                    .filter(|(_, id)| eligible(*id))
                    .take(12)
                    .chain(
                        self.times
                            .range((Excluded((time, source)), Unbounded))
                            .filter(|(_, id)| eligible(*id))
                            .take(12),
                    )
                {
                    examined += 1;
                    ids.insert(*id);
                }
            }
            for (_, id) in self
                .recent
                .range(..(version, source))
                .rev()
                .filter(|(_, id)| eligible(*id))
                .take(8)
            {
                examined += 1;
                ids.insert(*id);
            }
        }
        ids.remove(&source);
        DreamCandidatePool {
            memory_version: self.version,
            ids: ids.into_iter().collect(),
            index_entries_examined: examined,
        }
    }
}
