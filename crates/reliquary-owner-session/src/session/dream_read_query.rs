use super::*;
impl DreamReadIndex {
    pub fn query(
        &self,
        source: MemoryId,
        version: u64,
        time: Option<i64>,
        profile: CompatibilityProfileId,
        row: &[f32],
    ) -> DreamCandidatePool {
        let mut ids = BTreeSet::new();
        let mut examined = 0;
        if self.active.len() <= 128 {
            ids.extend(self.active.iter().copied());
            examined = self.active.len();
        } else {
            let mut semantic = BTreeSet::new();
            for table in 0..4 {
                let code = locality(row, table);
                for probe in std::iter::once(code).chain((0..12).map(|bit| code ^ (1 << bit))) {
                    if let Some(bucket) = self.semantic.get(&(profile.0, table, probe)) {
                        for id in bucket.iter().take(16) {
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
                for id in bucket.iter().take(4) {
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
                for (_, id) in self.times.range(..(time, source)).rev().take(12).chain(
                    self.times
                        .range((Excluded((time, source)), Unbounded))
                        .take(12),
                ) {
                    examined += 1;
                    ids.insert(*id);
                }
            }
            for (_, id) in self.recent.range(..(version, source)).rev().take(8) {
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
