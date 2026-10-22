//! Import scope translation into the read-only History boundary.
use crate::history::ExternalBirthFacts;
use crate::owner_lifecycle::HistorySource;
use crate::{Cva, CvaError, MemoryId, freshness::FreshnessRecord};
use std::collections::BTreeMap;

/// Decode the allowed external-source publisher evidence at the facade edge.
/// History receives only bounded birth facts, never the external codec or its
/// envelope. Decode errors are kept at frame ends and replayed in scan order.
fn external_birth_facts(
    source: &HistorySource<'_>,
    prefix_end: u64,
) -> BTreeMap<u64, Result<ExternalBirthFacts, crate::MemoryError>> {
    let mut facts = BTreeMap::new();
    let Ok(mut cursor) = source.prefix_cursor(prefix_end) else {
        // The same stable borrowed source is rescanned by History, which emits
        // this physical/source failure in its original place in replay order.
        return facts;
    };
    loop {
        let frame = match cursor.next_frame() {
            Ok(Some(frame)) => frame,
            Ok(None) | Err(_) => {
                // Do not report a pre-scan failure here. History repeats this
                // bounded scan and preserves ordering with earlier evidence.
                break;
            }
        };
        match crate::memory_external_publication::decode(&frame.payload) {
            Ok(Some(publication)) => {
                facts.insert(
                    frame.end,
                    Ok(ExternalBirthFacts {
                        birth_turn: publication.birth_turn,
                        records: publication
                            .records
                            .into_iter()
                            .map(|record| (record.id, record.global_version, record.mutation_id))
                            .collect(),
                    }),
                );
            }
            Ok(None) => {}
            Err(error) => {
                facts.insert(frame.end, Err(error));
            }
        }
    }
    facts
}

impl Cva {
    pub(crate) fn historical_import_base(
        &self,
        scope: &crate::imports::ImportEpochScope,
    ) -> Result<BTreeMap<MemoryId, FreshnessRecord>, CvaError> {
        let source = crate::owner_lifecycle::HistorySource::new(&self.container)
            .map_err(|error| CvaError::Container(error.0))?;
        let external_birth_facts = external_birth_facts(&source, scope.start_offset);
        crate::history::historical_freshness_base(
            &source,
            &self.memories,
            crate::history::HistoricalFreshnessBaseCut {
                owner_uuid: scope.start.owner_uuid,
                prefix_end: scope.start_offset,
                activity_cut: scope.base_cut,
                global_cut: scope.start.pre.global_version,
            },
            external_birth_facts,
        )
        .map_err(|error| match error {
            crate::history::HistoricalFreshnessBaseError::Freshness(message) => {
                CvaError::Freshness(message)
            }
            crate::history::HistoricalFreshnessBaseError::Memory(error) => error.into(),
            crate::history::HistoricalFreshnessBaseError::Source(error) => {
                CvaError::Container(error.0)
            }
        })
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
            outputs: BTreeMap::new(),
        };
        let bytes_before = std::fs::read(&path).unwrap();
        let records_before = cva.freshness.records().clone();
        let receipts_before = cva.freshness.replay_entries().unwrap();
        let base = cva.historical_import_base(&scope).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes_before);
        assert_eq!(cva.freshness.records(), &records_before);
        assert_eq!(cva.freshness.replay_entries().unwrap(), receipts_before);
        assert_eq!(base.get(&memory), Some(&expected));
        assert_ne!(
            base.get(&memory),
            cva.freshness.records_at(cut).unwrap().get(&memory)
        );

        drop(cva);
        std::fs::remove_file(path).unwrap();
    }
}
