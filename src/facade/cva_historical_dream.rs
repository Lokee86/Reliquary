use crate::{Cva, CvaError, InitialDreamPass, MemoryId};
use std::collections::BTreeMap;
impl Cva {
    /// Returns retained completed first-cycle evidence without changing scores or
    /// resident journal state. Missing entries (including transformed markers)
    /// must not be interpreted as verified no-link outcomes.
    pub fn historical_completed_initial_dream_passes(
        &self,
    ) -> Result<BTreeMap<MemoryId, InitialDreamPass>, CvaError> {
        if self.owner_uuid().is_none() {
            return Err(CvaError::Freshness(
                "historical Dream proof requires a durable REL owner".into(),
            ));
        }
        crate::freshness_dream_journal::DreamFreshnessJournal::retained_completed_passes(
            &self.container,
            self.owner_uuid(),
        )
    }

    /// Uses Dream's existing representative authority on the pinned completed
    /// graph. Historical age is checked by the producer, not physical birth order.
    pub fn historical_dream_representatives(
        &mut self,
        source_id: MemoryId,
        roots: &std::collections::BTreeSet<MemoryId>,
    ) -> Result<BTreeMap<MemoryId, MemoryId>, CvaError> {
        self.memory(source_id)?;
        let relations = self.graph_relations();
        let mut result = BTreeMap::new();
        for &root in roots {
            self.memory(root)?;
            let duplicate = relations.iter().any(|relation| {
                relation.kind == crate::GraphRelationKind::DuplicateOf
                    && (relation.source == root || relation.target == root)
            });
            let representative = if duplicate {
                crate::dream_canonical::existing_duplicate_representative(
                    self,
                    root,
                    u64::MAX,
                    source_id,
                    &relations,
                )
                .map_err(|error| CvaError::Freshness(error.to_string()))?
                .ok_or_else(|| {
                    CvaError::Freshness(
                        "historical duplicate root has no verified representative".into(),
                    )
                })?
            } else {
                root
            };
            result.insert(root, representative);
        }
        Ok(result)
    }
}
