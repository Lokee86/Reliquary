//! Accepted Freshness projection strictly before an explicit physical cut.
use crate::memory_store::MemoryStore;
use crate::owner_lifecycle::HistorySource;
use crate::{MemoryId, freshness::FreshnessRecord};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) enum HistoricalFreshnessBaseError {
    Freshness(String),
    Source(crate::owner_lifecycle::HistoryReadFailure),
    Memory(crate::MemoryError),
}
impl From<String> for HistoricalFreshnessBaseError {
    fn from(value: String) -> Self {
        Self::Freshness(value)
    }
}
impl From<&str> for HistoricalFreshnessBaseError {
    fn from(value: &str) -> Self {
        Self::Freshness(value.into())
    }
}
impl From<crate::owner_lifecycle::HistoryReadFailure> for HistoricalFreshnessBaseError {
    fn from(value: crate::owner_lifecycle::HistoryReadFailure) -> Self {
        Self::Source(value)
    }
}

pub(crate) struct HistoricalFreshnessBaseCut {
    pub(crate) owner_uuid: [u8; 16],
    pub(crate) prefix_end: u64,
    pub(crate) activity_cut: u64,
    pub(crate) global_cut: u64,
}

pub(crate) use crate::owner_lifecycle::import_birth_facts::ExternalBirthFacts;

pub(crate) fn historical_freshness_base(
    source: &HistorySource<'_>,
    memories: &MemoryStore,
    cut: HistoricalFreshnessBaseCut,
    mut external_birth_facts: BTreeMap<u64, Result<ExternalBirthFacts, crate::MemoryError>>,
) -> Result<BTreeMap<MemoryId, FreshnessRecord>, HistoricalFreshnessBaseError> {
    let owner = cut.owner_uuid;
    if source.owner_uuid() != Some(owner) {
        return Err("import base owner mismatch".into());
    }
    let mut prefix = source.begin_freshness_replay(owner)?;
    let mut dream_prefix = crate::freshness_dream_journal::DirectBirthPrefix::new(owner);
    let mut represented = std::collections::BTreeSet::new();
    let scan = source;
    {
        let mut ingest_frame = |frame: crate::owner_lifecycle::HistoryFrame| -> Result<(), HistoricalFreshnessBaseError> {
            let end = frame.end;
            let payload = frame.payload.as_slice();
            {
                // Rebuild publication intents only from this physical prefix. This
                // mirrors reopen ordering and excludes intents created after start.
                dream_prefix
                    .ingest(payload)
                    .map_err(|error| error.to_string())?;
                for (id, mutation, turn, admitted) in dream_prefix.pending_initializations() {
                    if represented.contains(&id)
                        || turn > cut.activity_cut
                        || memories.first_mutation_id(id) != Some(mutation.as_str())
                        || memories
                            .first_global_version(id)
                            .is_none_or(|global| global > cut.global_cut)
                    {
                        continue;
                    }
                    prefix
                        .derive_birth(owner, id, turn, admitted)
                        .map_err(|error| error.to_string())?;
                    represented.insert(id);
                }

                // Grouped and external receipts carry their own birth proofs.
                if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
                    .map_err(|e| e.to_string())?
                    && let Some(turn) = completion.freshness_birth_turn
                {
                    for id in completion.freshness_birth_memory_ids {
                        let record = completion
                            .records
                            .iter()
                            .find(|record| record.id == id && record.revision == 1)
                            .ok_or_else(|| {
                                "grouped birth proof lacks first Memory record".to_owned()
                            })?;
                        if record.global_version > cut.global_cut
                            || turn > cut.activity_cut
                            || memories.first_mutation_id(id) != Some(record.mutation_id.as_str())
                        {
                            return Err("pre-start grouped birth proof conflicts".into());
                        }
                        prefix
                            .derive_birth(owner, id, turn, false)
                            .map_err(|error| error.to_string())?;
                        represented.insert(id);
                    }
                }
                if let Some((start, count, _)) = frame.external_version_range()
                    .map_err(HistoricalFreshnessBaseError::Memory)? {
                    let facts = external_birth_facts.remove(&end)
                        .ok_or("missing accepted external publication birth facts")?
                        .map_err(HistoricalFreshnessBaseError::Memory)?;
                    if facts.records.len() != count as usize || facts.records.iter().enumerate()
                        .any(|(index, (_, global, _))| start.checked_add(index as u64) != Some(*global)) {
                        return Err("external publication birth facts conflict with accepted clock range".into());
                    }
                    if let Some(turn) = facts.birth_turn {
                        for (id, global_version, mutation_id) in facts.records {
                            if global_version > cut.global_cut
                                || turn > cut.activity_cut
                                || memories.first_mutation_id(id) != Some(mutation_id.as_str())
                            {
                                return Err("pre-start external birth proof conflicts".into());
                            }
                            prefix
                                .derive_birth(owner, id, turn, false)
                                .map_err(|error| error.to_string())?;
                            represented.insert(id);
                        }
                    }
                }
                prefix
                    .ingest_scanned(payload, end)
                    .map_err(HistoricalFreshnessBaseError::Freshness)
            }
        };
        let mut cursor = scan.prefix_cursor(cut.prefix_end)?;
        (|| {
            while let Some(frame) = cursor.next_frame()? {
                ingest_frame(frame)?;
            }
            Ok::<(), HistoricalFreshnessBaseError>(())
        })()
    }?;

    if !external_birth_facts.is_empty() {
        return Err("external publication birth facts exceed accepted prefix".into());
    }
    source.finish_freshness_replay(&mut prefix, cut.prefix_end)?;
    prefix
        .records_at(cut.activity_cut)
        .map_err(HistoricalFreshnessBaseError::Freshness)
}
