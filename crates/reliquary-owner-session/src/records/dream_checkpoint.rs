//! Disposable startup projection of owner-verified Dream evidence.
use super::population_history::PopulationHistory;
use super::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::borrow::Cow;

#[derive(Serialize, Deserialize)]
struct Checkpoint<'a> {
    owner_uuid: [u8; 16],
    admissions: Vec<Admission>,
    journal: freshness_journal::JournalCheckpoint<'a>,
    initial_settlements: Cow<'a, BTreeMap<String, InitialDreamSettlement>>,
    settled_sources: Cow<'a, [MemoryId]>,
    settlement_positions: Cow<'a, BTreeMap<MemoryId, u64>>,
    frontiers: Cow<'a, BTreeMap<String, frontier::StoredFrontier>>,
}

#[derive(Serialize, Deserialize)]
struct Admission {
    pass_id: String,
    source_id: MemoryId,
    turn: u64,
    graph_version_before: u64,
    memory_cut: u64,
    added: Vec<MemoryId>,
    removed: Vec<MemoryId>,
    begin: Option<BeginKind>,
}

impl Serialize for DreamEvidenceStore {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut ordered: Vec<_> = self.admissions.values().collect();
        ordered.sort_by_cached_key(|admission| {
            // Multiple frontiers can share a Memory cut while their proven
            // populations follow the sequence of births before that cut.
            let population_cut = self
                .begins
                .get(&admission.pass_id)
                .and_then(|begin| {
                    begin
                        .native_birth()
                        .or_else(|| begin.inferred_explicit_birth())
                })
                .map_or(admission.memory_cut, |version| version.saturating_sub(1));
            (population_cut, admission.pass_id.as_str())
        });
        let empty = Population::from_set(BTreeSet::new());
        let mut previous = &empty;
        let admissions = ordered
            .into_iter()
            .map(|admission| {
                let current = &admission.preexisting_ids;
                let (added, removed) = current.changes_since(previous);
                let entry = Admission {
                    pass_id: admission.pass_id.clone(),
                    source_id: admission.source_id,
                    turn: admission.turn,
                    graph_version_before: admission.graph_version_before,
                    memory_cut: admission.memory_cut,
                    added,
                    removed,
                    begin: self.begins.get(&admission.pass_id).map(|begin| begin.kind),
                };
                previous = current;
                entry
            })
            .collect();
        Checkpoint {
            owner_uuid: self.owner_uuid,
            admissions,
            journal: self.journal.checkpoint(&self.admissions),
            initial_settlements: Cow::Borrowed(&self.initial_settlements),
            settled_sources: Cow::Borrowed(&self.settled_sources),
            settlement_positions: Cow::Borrowed(&self.settlement_positions),
            frontiers: Cow::Borrowed(&self.frontiers),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DreamEvidenceStore {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut checkpoint = Checkpoint::deserialize(deserializer)?;
        let population_history =
            PopulationHistory::from_deltas(checkpoint.admissions.iter_mut().map(|entry| {
                (
                    std::mem::take(&mut entry.added),
                    std::mem::take(&mut entry.removed),
                )
            }))
            .map_err(de::Error::custom)?;
        let mut admissions = BTreeMap::new();
        let mut begins = BTreeMap::new();
        let mut begun_sources = BTreeMap::new();
        for (index, entry) in checkpoint.admissions.into_iter().enumerate() {
            let cohort = population_history.snapshot(index + 1);
            if matches!(
                entry.begin,
                Some(BeginKind::InferredExplicit { first_memory_version })
                    if !(1..=entry.memory_cut).contains(&first_memory_version)
            ) || (entry.begin.is_some() && cohort.contains(&entry.source_id))
            {
                return Err(de::Error::custom(
                    "invalid Dream checkpoint population delta",
                ));
            }
            let admission = StoredAdmission {
                owner_uuid: checkpoint.owner_uuid,
                pass_id: entry.pass_id.clone(),
                source_id: entry.source_id,
                turn: entry.turn,
                graph_version_before: entry.graph_version_before,
                preexisting_ids: cohort,
                memory_cut: entry.memory_cut,
            };
            if let Some(kind) = entry.begin {
                if begun_sources
                    .insert(entry.source_id, entry.pass_id.clone())
                    .is_some()
                {
                    return Err(de::Error::custom("duplicate Dream checkpoint Begin source"));
                }
                begins.insert(entry.pass_id.clone(), StoredBegin::new(&admission, kind));
            }
            if admissions.insert(entry.pass_id, admission).is_some() {
                return Err(de::Error::custom("duplicate Dream checkpoint admission"));
            }
        }
        let journal = checkpoint
            .journal
            .restore(&admissions)
            .map_err(de::Error::custom)?;
        Ok(Self {
            owner_uuid: checkpoint.owner_uuid,
            admissions,
            begins,
            begun_sources,
            journal,
            initial_settlements: checkpoint.initial_settlements.into_owned(),
            settled_sources: checkpoint.settled_sources.into_owned(),
            settlement_positions: checkpoint.settlement_positions.into_owned(),
            frontiers: checkpoint.frontiers.into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::freshness_journal::InitialDreamPass;
    use super::*;
    use crate::DreamMemoryCut;
    use crate::records::dream_frontier::FrontierRecord;
    use reliquary_memory_core::MemoryBodyId;

    fn memory_id(index: u64) -> MemoryId {
        MemoryId(Sha256::digest(index.to_le_bytes()).into())
    }

    #[test]
    fn checkpoint_keeps_changing_dream_cohorts_compact_and_exact() {
        const PASSES: u64 = 960;
        let owner = [7; 16];
        let mut store = DreamEvidenceStore {
            owner_uuid: owner,
            admissions: BTreeMap::new(),
            begins: BTreeMap::new(),
            begun_sources: BTreeMap::new(),
            journal: DreamFreshnessJournal::new(Some(owner)),
            initial_settlements: BTreeMap::new(),
            settled_sources: Vec::new(),
            settlement_positions: BTreeMap::new(),
            frontiers: BTreeMap::new(),
        };
        let mut cohort: BTreeSet<_> = (0..600).map(memory_id).collect();
        let mut last_settlement = None;
        for i in 0..PASSES {
            // Reverse lexical order relative to the physical cuts.
            let pass_id = format!("pass-{:04}", PASSES - i);
            let source_id = memory_id(600 + i);
            let body_id = MemoryBodyId([0; 32]);
            let cut = 600 + i;
            let turn = i + 1;
            let native_birth = i % 4 == 1;
            let inferred_explicit = i == 0;
            let memory_cut = if native_birth || inferred_explicit {
                600 + PASSES
            } else {
                cut
            };
            let begin = BeginRecord {
                owner_uuid: owner,
                pass_id: pass_id.clone(),
                source_id,
                turn,
                graph_version_before: i,
                population: if native_birth {
                    BeginPopulation::NativeBirth {
                        first_memory_version: cut + 1,
                        digest: population_digest(&cohort),
                    }
                } else if i % 2 == 0 {
                    BeginPopulation::ExplicitSet(cohort.clone())
                } else {
                    BeginPopulation::CompactCut {
                        memory_cut,
                        digest: population_digest(&cohort),
                    }
                },
            };
            let admission = DreamBeginAdmission {
                owner_uuid: owner,
                pass_id: pass_id.clone(),
                source_id,
                turn,
                graph_version_before: i,
                preexisting_ids: cohort.clone(),
                memory_cut,
            };
            let mut frontier = FrontierRecord {
                owner,
                pass_id: pass_id.clone(),
                begin: Some(dream_begin_codec::encode(&begin).unwrap()),
                source: DreamMemoryCut {
                    id: source_id,
                    revision: 1,
                    body_id,
                }
                .into(),
                candidates: Vec::new(),
                graph_version: i,
                memory_version: memory_cut,
            };
            if inferred_explicit {
                store.admissions.insert(pass_id.clone(), admission.into());
                frontier.begin = None;
                store
                    .ingest_inferred_explicit_frontier(InferredExplicitFrontier {
                        position: ObjectRef {
                            offset: 0,
                            length: 0,
                        },
                        frontier,
                        turn,
                        first_memory_version: cut + 1,
                    })
                    .unwrap();
            } else {
                store
                    .replay_initial_frontier(frontier, begin, admission)
                    .unwrap();
            }
            let settlement = InitialDreamSettlement {
                owner,
                pass_id,
                source: source_id,
                revision: 1,
                body: body_id,
                graph_version: i,
                accepted_turn: turn,
                community_generation: None,
                roots: BTreeSet::from([memory_id(599)]),
                credit_roots: BTreeSet::new(),
                admit: false,
                freshness_effects: Vec::new(),
                memory_delta: None,
            };
            if turn == PASSES {
                last_settlement = Some(settlement);
            } else {
                store.apply_initial_settlement(settlement).unwrap();
            }
            cohort.insert(source_id);
            if i % 3 == 0 {
                cohort.remove(&memory_id(i / 3));
            }
        }
        // An independently materialized journal entry has no admission to share.
        let legacy = InitialDreamPass {
            pass_id: "legacy".into(),
            source_id: memory_id(9999),
            first_revision: 1,
            turn: 2,
            accepted_settlement_turn: None,
            preexisting_ids: BTreeSet::from([memory_id(1)]),
            graph_version_before: 1,
            roots: BTreeSet::new(),
        };
        store
            .journal
            .ingest_materialized_begin(owner, legacy.clone())
            .unwrap();

        let bytes = rmp_serde::to_vec_named(&store).unwrap();
        assert!(
            bytes.len() < 1024 * 1024,
            "{} checkpoint bytes",
            bytes.len()
        );
        let mut restored: DreamEvidenceStore = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(rmp_serde::to_vec_named(&restored).unwrap(), bytes);
        assert!(
            restored
                .admissions
                .values()
                .all(|entry| !entry.preexisting_ids.is_materialized())
        );
        assert!(
            restored
                .begins
                .values()
                .all(|begin| begin.view.get().is_none())
        );
        for pass_id in ["pass-0960", "pass-0959", "pass-0001"] {
            assert_eq!(restored.begin(pass_id), store.begin(pass_id));
        }
        assert_eq!(
            restored
                .begun("pass-0960")
                .unwrap()
                .inferred_explicit_birth(),
            Some(601)
        );
        assert!(matches!(
            restored.begin("pass-0960").unwrap().population,
            BeginPopulation::ExplicitSet(_)
        ));
        let native = restored.begun("pass-0959").unwrap();
        assert_eq!(native.native_birth(), Some(602));
        assert!(!native.is_compact());
        assert_eq!(restored.journal.pending("legacy"), Some(legacy));
        assert_eq!(
            restored.journal.pending("pass-0001"),
            store.journal.pending("pass-0001")
        );
        assert_eq!(restored.initial_settlements, store.initial_settlements);
        assert_eq!(restored.settled_sources, store.settled_sources);
        for pass_id in store.begins.keys() {
            assert_eq!(
                restored.initial_frontier(pass_id),
                store.initial_frontier(pass_id)
            );
            assert!(restored.frontier_has_bound_begin(pass_id));
        }
        restored
            .apply_initial_settlement(last_settlement.unwrap())
            .unwrap();
        assert_eq!(restored.settlement_count(), PASSES);
        assert!(
            restored
                .admissions
                .values()
                .all(|entry| !entry.preexisting_ids.is_materialized())
        );
    }
}
