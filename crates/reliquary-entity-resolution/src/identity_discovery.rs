//! Page cross-owner candidate pairs from equal, owner-local discovery keys.

use reliquary_perception_store::EntityId;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound::{Excluded, Unbounded};

/// A frequent signal cannot nominate a pair on its own. Another, more selective
/// name or identifier can still nominate the same Entities.
const MAX_ENTITIES_PER_KEY: usize = 32;

#[derive(Default)]
pub struct IdentityDiscoveryIndex {
    by_entity: BTreeMap<EntityId, Vec<[u8; 32]>>,
    by_key: BTreeMap<[u8; 32], BTreeSet<EntityId>>,
}

impl IdentityDiscoveryIndex {
    /// Build one owner snapshot from consecutive, nonoverlapping Entity pages.
    pub fn extend(&mut self, rows: Vec<(EntityId, Vec<[u8; 32]>)>) {
        for (entity, keys) in rows {
            for key in &keys {
                self.by_key.entry(*key).or_default().insert(entity);
            }
            self.by_entity.insert(entity, keys);
        }
    }

    /// A row also advances the cursor when none of its keys nominates a peer.
    /// Call only with complete indexes so frequency is measured across each owner.
    pub fn candidate_rows(
        &self,
        peer: &Self,
        after: Option<EntityId>,
        limit: usize,
    ) -> Vec<(EntityId, Vec<EntityId>)> {
        self.by_entity
            .range((after.map_or(Unbounded, Excluded), Unbounded))
            .take(limit)
            .map(|(&entity, keys)| {
                let mut matches = BTreeSet::new();
                for key in keys {
                    if self.by_key[key].len() <= MAX_ENTITIES_PER_KEY
                        && let Some(targets) = peer.by_key.get(key)
                        && targets.len() <= MAX_ENTITIES_PER_KEY
                    {
                        matches.extend(targets.iter().copied());
                    }
                }
                (entity, matches.into_iter().collect())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_past_empty_rows_deduplicates_matches_and_skips_common_keys() {
        let id = |n| EntityId([n; 32]);
        let mut left = IdentityDiscoveryIndex::default();
        left.extend(vec![
            (id(1), vec![]),
            (id(2), vec![[1; 32], [2; 32], [3; 32]]),
        ]);
        let mut peer = IdentityDiscoveryIndex::default();
        let rows = (0..=MAX_ENTITIES_PER_KEY as u8).map(|n| {
            let mut keys = vec![[3; 32]];
            if n == 2 {
                keys.extend([[1; 32], [2; 32]]);
            }
            (id(n), keys)
        });
        peer.extend(rows.collect());

        assert_eq!(left.candidate_rows(&peer, None, 1), [(id(1), vec![])]);
        assert_eq!(
            left.candidate_rows(&peer, Some(id(1)), 1),
            [(id(2), vec![id(2)])]
        );
        assert_eq!(peer.candidate_rows(&left, None, 1), [(id(0), vec![])]);
    }
}
