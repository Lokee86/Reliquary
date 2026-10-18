//! Canonical pre-import Freshness projection at the physical ImportStart cut.
use crate::{Cva, CvaError, MemoryId, freshness::FreshnessRecord};
use std::collections::BTreeMap;

impl Cva {
    /// Rebuilds the accepted Freshness projection from bytes physically before
    /// this epoch's ImportStart marker. The activity cut alone cannot distinguish
    /// same-cut receipts on either side of the marker.
    pub(crate) fn historical_import_base(
        &self,
        scope: &crate::imports::ImportEpochScope,
    ) -> Result<BTreeMap<MemoryId, FreshnessRecord>, CvaError> {
        let owner = scope.start.owner_uuid;
        if self.owner_uuid() != Some(owner) {
            return Err(CvaError::Freshness("import base owner mismatch".into()));
        }
        let mut prefix = crate::freshness_storage::FreshnessStore::new(Some(owner));
        prefix.bind_history_path(self.container.path(), 0);
        prefix.begin_rebuild();
        let mut dream_prefix = crate::freshness_dream_journal::DreamFreshnessJournal::default();
        let mut represented = std::collections::BTreeSet::new();
        let mut scan = crate::Container::open_read_only_for_scan(self.container.path())?;
        scan.visit_payloads_through::<CvaError>(scope.start_offset, |object, payload| {
            let location = object.legacy_bytes();
            let end = u64::from_le_bytes(location[..8].try_into().unwrap())
                + 8
                + u64::from_le_bytes(location[8..].try_into().unwrap());

            // Rebuild publication intents only from this physical prefix. This
            // mirrors reopen ordering and excludes intents created after start.
            dream_prefix.ingest(payload).map_err(CvaError::Freshness)?;
            for (id, mutation, turn, admitted) in dream_prefix.pending_initializations() {
                if represented.contains(&id)
                    || turn > scope.base_cut
                    || self.memories.first_mutation_id(id) != Some(mutation.as_str())
                    || self
                        .memories
                        .first_global_version(id)
                        .is_none_or(|global| global > scope.start.pre.global_version)
                {
                    continue;
                }
                prefix
                    .derive_birth(owner, id, turn, admitted)
                    .map_err(CvaError::Freshness)?;
                represented.insert(id);
            }

            // Grouped and external receipts carry their own birth proofs.
            if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
                .map_err(|e| CvaError::Freshness(e.into()))?
                && let Some(turn) = completion.freshness_birth_turn
            {
                for id in completion.freshness_birth_memory_ids {
                    let record = completion
                        .records
                        .iter()
                        .find(|record| record.id == id && record.revision == 1)
                        .ok_or_else(|| {
                            CvaError::Freshness(
                                "grouped birth proof lacks first Memory record".into(),
                            )
                        })?;
                    if record.global_version > scope.start.pre.global_version
                        || turn > scope.base_cut
                        || self.memories.first_mutation_id(id) != Some(record.mutation_id.as_str())
                    {
                        return Err(CvaError::Freshness(
                            "pre-start grouped birth proof conflicts".into(),
                        ));
                    }
                    prefix
                        .derive_birth(owner, id, turn, false)
                        .map_err(CvaError::Freshness)?;
                    represented.insert(id);
                }
            }
            if let Some(publication) = crate::external_source::publication::decode(payload)?
                && let Some(turn) = publication.birth_turn
            {
                for record in publication.records {
                    if record.global_version > scope.start.pre.global_version
                        || turn > scope.base_cut
                        || self.memories.first_mutation_id(record.id)
                            != Some(record.mutation_id.as_str())
                    {
                        return Err(CvaError::Freshness(
                            "pre-start external birth proof conflicts".into(),
                        ));
                    }
                    prefix
                        .derive_birth(owner, record.id, turn, false)
                        .map_err(CvaError::Freshness)?;
                    represented.insert(record.id);
                }
            }
            prefix
                .ingest_scanned(payload, end)
                .map_err(CvaError::Freshness)
        })?;

        prefix.bind_history_path(self.container.path(), scope.start_offset);
        prefix.finish_rebuild().map_err(CvaError::Freshness)?;
        prefix
            .records_at(scope.base_cut)
            .map_err(CvaError::Freshness)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EpisodeOrigin, FreshnessEventProof,
        imports::{ImportEpochScope, ImportStart, ImportWatermarks},
        insomnia::completion_tests::{process, setup_origin},
    };
    use std::collections::{BTreeMap, BTreeSet};

    fn accepted_use(cva: &mut Cva, memory: MemoryId, id: &str, turn: u64) {
        let owner = cva.owner_uuid().unwrap();
        let payload = cva
            .freshness
            .prepare_accepted_use_batch(
                owner,
                id.to_owned(),
                turn,
                vec![memory],
                FreshnessEventProof {
                    graph_version: cva.graph_version(),
                    community_generation: None,
                    community_graph_version: None,
                },
                vec![BTreeMap::from([(memory, 25)])],
            )
            .unwrap();
        cva.container.append(&payload).unwrap();
        cva.container.sync().unwrap();
        cva.freshness.ingest_appended(&payload).unwrap();
    }

    #[test]
    fn physical_start_prefix_keeps_pre_start_same_cut_receipt_only() {
        let (path, mut cva, _) =
            setup_origin("historical-import-base-same-cut.rel", EpisodeOrigin::Live);
        cva.set_freshness_policy_before_enrollment(crate::freshness::FreshnessPolicy {
            decay_turns_per_point: 1,
            ..crate::freshness::FreshnessPolicy::default_v1()
        })
        .unwrap();
        let memory = process(&mut cva, 1, 110).created[0].id;
        cva.freshness_admit(memory, cva.rel_turn_count()).unwrap();
        // Decay creates room for two distinguishable same-cut accepted uses.
        for index in 0..40 {
            cva.append_node(
                format!("decay-{index}"),
                "decay".into(),
                None,
                "user".into(),
                200 + index,
                "Accepted activity before import",
            )
            .unwrap();
        }
        let cut = cva.rel_turn_count();
        accepted_use(&mut cva, memory, "a-before-start", cut);
        let expected = cva.freshness.record_at(memory, cut).unwrap().unwrap();
        let start_offset = std::fs::metadata(&path).unwrap().len();
        cva.container.append(b"ImportStart test boundary").unwrap();
        cva.container.sync().unwrap();
        accepted_use(&mut cva, memory, "z-after-start", cut);

        let owner = cva.owner_uuid().unwrap();
        let scope = ImportEpochScope {
            start: ImportStart {
                owner_uuid: owner,
                import_id: "test-import".into(),
                import_key: "test-import".into(),
                source_namespace: "test".into(),
                source_format: "test".into(),
                started_at_ns: 0,
                pre: ImportWatermarks {
                    global_version: cva.latest_global_version(),
                    ..ImportWatermarks::default()
                },
            },
            start_offset,
            marker_end: start_offset,
            start_digest: [1; 32],
            base_cut: cut,
            source_nodes: BTreeSet::new(),
            episodes: BTreeSet::new(),
            cohort: BTreeSet::new(),
        };
        let base = cva.historical_import_base(&scope).unwrap();
        assert_eq!(base.get(&memory), Some(&expected));
        assert_ne!(
            base.get(&memory),
            cva.freshness.records_at(cut).unwrap().get(&memory)
        );

        drop(cva);
        std::fs::remove_file(path).unwrap();
    }
}
