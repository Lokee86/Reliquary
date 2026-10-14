//! A frozen one-shot native REL import and its atomic Freshness publication.
use crate::freshness::historical_scorer::{HistoricalScoreEvent, score_historical_records};
use crate::freshness_storage::ImportCheckpoint;
use crate::{
    Cva, CvaError, MemoryId,
    freshness::{FreshnessPolicy, FreshnessRecord},
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
#[path = "historical_embedding.rs"]
mod embedding;
#[path = "historical_embedding_model.rs"]
mod embedding_model;
#[path = "historical_embedding_plan.rs"]
mod embedding_plan;
#[path = "historical_manifest_encoding.rs"]
mod encoding;
#[path = "historical_events.rs"]
mod events;
pub use embedding_model::{
    HistoricalEmbeddingCoverage, HistoricalEmbeddingDisposition, HistoricalEmbeddingFailure,
    HistoricalEmbeddingLimits, HistoricalEmbeddingReport,
};
#[path = "historical_import_input.rs"]
mod input;
#[path = "historical_manifest.rs"]
mod manifest;
#[path = "historical_import_transform.rs"]
mod transform;

#[derive(Clone, Debug)]
struct FrozenHistoricalImport {
    events: Option<events::NativeHistoricalEvents>,
    propagation: Option<manifest::PropagationInputs>,
    checkpoint: ImportCheckpoint,
    initial: BTreeMap<MemoryId, FreshnessRecord>,
}
#[derive(Clone, Debug)]
struct PreparedHistoricalImport {
    checkpoint: ImportCheckpoint,
}
impl FrozenHistoricalImport {
    /// Only the owner-derived batch can enter production scoring.
    fn prepare_native(&self) -> Result<PreparedHistoricalImport, CvaError> {
        let Some(batch) = &self.events else {
            return Ok(PreparedHistoricalImport {
                checkpoint: self.checkpoint.clone(),
            });
        };
        let inputs = self
            .propagation
            .as_ref()
            .ok_or_else(|| fail("missing frozen propagation proof"))?;
        if batch
            .ordered
            .iter()
            .any(|(_, event)| event.position > self.checkpoint.cut)
        {
            return Err(fail(
                "historical source event exceeds frozen checkpoint cut",
            ));
        }
        let events: Vec<_> = batch
            .ordered
            .iter()
            .map(|(_, event)| event.clone())
            .collect();
        let outcomes = crate::freshness::historical_propagation::propagate_historical_events(
            &events,
            &inputs.adjacency,
            &inputs.memberships,
            &inputs.authorized,
            &inputs.availability,
            1,
            &FreshnessPolicy::default_v1(),
        )
        .map_err(|e| fail(&e.to_string()))?;
        let scores: Vec<_> = batch
            .ordered
            .iter()
            .zip(outcomes)
            .flat_map(|((id, event), outcome)| {
                outcome
                    .candidates
                    .into_iter()
                    .map(move |(recipient, amount)| HistoricalScoreEvent {
                        recipient,
                        position: event.position,
                        event_id: *id,
                        amount,
                    })
            })
            .collect();
        self.prepare(&scores)
    }
    fn import_id(&self) -> [u8; 32] {
        self.checkpoint.import_id
    }
    fn cut(&self) -> u64 {
        self.checkpoint.cut
    }
    fn initial_records(&self) -> &BTreeMap<MemoryId, FreshnessRecord> {
        &self.initial
    }
    /// Scores only positive historical contributions for supported admitted recipients.
    /// Internal coordinator evidence must be covered by the frozen canonical manifest.
    fn prepare(
        &self,
        events: &[HistoricalScoreEvent],
    ) -> Result<PreparedHistoricalImport, CvaError> {
        let mut positions = BTreeMap::new();
        for event in events {
            if event.amount <= 0
                || event.amount > 50
                || self
                    .initial
                    .get(&event.recipient)
                    .is_none_or(|r| r.admitted_at_turn.is_none())
            {
                return Err(CvaError::Freshness(
                    "unsupported historical contribution/recipient".into(),
                ));
            }
            if positions
                .insert(event.event_id, event.position)
                .is_some_and(|old| old != event.position)
            {
                return Err(CvaError::Freshness(
                    "one historical event has conflicting source positions".into(),
                ));
            }
        }
        let mut checkpoint = self.checkpoint.clone();
        checkpoint.records = score_historical_records(
            &self.initial,
            events,
            checkpoint.cut,
            &FreshnessPolicy::default_v1(),
        )
        .map_err(|e| CvaError::Freshness(e.to_string()))?;
        checkpoint.validate().map_err(CvaError::Freshness)?;
        Ok(PreparedHistoricalImport { checkpoint })
    }
}
impl PreparedHistoricalImport {
    fn records(&self) -> &BTreeMap<MemoryId, FreshnessRecord> {
        &self.checkpoint.records
    }
}
fn fail(message: &str) -> CvaError {
    CvaError::Freshness(message.into())
}
fn file_digest(path: &std::path::Path) -> Result<[u8; 32], CvaError> {
    let mut file = std::fs::File::open(path).map_err(|e| fail(&e.to_string()))?;
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 65536];
    loop {
        let size = file.read(&mut bytes).map_err(|e| fail(&e.to_string()))?;
        if size == 0 {
            break;
        }
        hash.update(&bytes[..size]);
    }
    Ok(hash.finalize().into())
}
impl Cva {
    /// Private checkpoint primitive. The owner coordinator derives the
    /// verified manifest and frozen owner cut; this is not a caller-facing API.
    fn freeze_historical_import(&mut self) -> Result<FrozenHistoricalImport, CvaError> {
        self.freeze_historical_import_with_fill(false)
    }
    fn freeze_historical_import_with_fill(
        &mut self,
        requested_complete: bool,
    ) -> Result<FrozenHistoricalImport, CvaError> {
        if let Some(checkpoint) = self.freshness.import_checkpoint() {
            let checkpoint = checkpoint.clone();
            let mut initial = BTreeMap::new();
            for (id, record) in &checkpoint.records {
                let mut birth = FreshnessRecord::created();
                if let Some(at) = record.admitted_at_turn {
                    birth.admit(at).map_err(|e| fail(&e.to_string()))?;
                }
                initial.insert(*id, birth);
            }
            return Ok(FrozenHistoricalImport {
                events: None,
                propagation: None,
                checkpoint,
                initial,
            });
        }
        let owner = self.historical_reconstruction_owner()?;
        let manifest = manifest::build_with_fill(self, requested_complete)?;
        let events = events::build(self, &manifest)?;
        let cohort: BTreeSet<_> = self.memory_ids().into_iter().collect();
        self.container.sync()?;
        let checkpoint = ImportCheckpoint {
            owner,
            import_id: manifest.import_id,
            corpus_digest: manifest.corpus_digest,
            manifest_digest: manifest.digest,
            input_digest: manifest.input_digest,
            cohort,
            cut: self.rel_turn_count(),
            global_version: self.latest_global_version(),
            input_bytes: manifest.input_bytes,
            archive_version: self.archive_version(),
            graph_version: self.graph_version(),
            memory_graph_version: self.memory_graph_version(),
            records: BTreeMap::new(),
            superseded: manifest.superseded,
        };
        Ok(FrozenHistoricalImport {
            events: Some(events),
            propagation: Some(manifest.propagation),
            checkpoint,
            initial: manifest.initial,
        })
    }
    /// Atomically replace provisional scoring. Exact accepted retries resolve before
    /// input drift checks and never erase legitimate post-checkpoint live events.
    fn commit_historical_import(
        &mut self,
        prepared: &PreparedHistoricalImport,
    ) -> Result<bool, CvaError> {
        let checkpoint = &prepared.checkpoint;
        if self.owner_uuid() != Some(checkpoint.owner) {
            return Err(fail("import checkpoint owner mismatch"));
        }
        if let Some(old) = self.freshness.import_checkpoint() {
            return if old == checkpoint {
                Ok(false)
            } else {
                Err(fail("ImportManifestConflict"))
            };
        }
        if self.latest_global_version() != checkpoint.global_version
            || self.archive_version() != checkpoint.archive_version
            || self.rel_turn_count() != checkpoint.cut
            || self.graph_version() != checkpoint.graph_version
            || self.memory_graph_version() != checkpoint.memory_graph_version
            || std::fs::metadata(self.container.path())
                .map_err(|e| fail(&e.to_string()))?
                .len()
                != checkpoint.input_bytes
            || file_digest(self.container.path())? != checkpoint.input_digest
        {
            return Err(fail("ImportInputDrift: discard and refreeze the attempt"));
        }
        let payload = checkpoint.encode();
        let mut staged = self.freshness.clone();
        staged
            .ingest_import_checkpoint(&payload)
            .map_err(CvaError::Freshness)?;
        let mut due = crate::FreshnessDueDispatcher::default();
        due.rebuild(
            staged.records().iter().map(|(&id, &r)| (id, r)),
            checkpoint.cut,
            &self.freshness_policy(),
        )
        .map_err(|e| fail(&e.to_string()))?;
        staged
            .persist_policy_if_needed(&mut self.container, checkpoint.owner)
            .map_err(CvaError::Freshness)?;
        self.container.append(&payload)?;
        self.container.sync()?;
        staged
            .ingest_appended(&payload)
            .map_err(CvaError::Freshness)?;
        self.freshness = staged;
        self.freshness_due = due;
        self.freshness_notifications.clear();
        Ok(true)
    }
}

impl Cva {
    pub(crate) fn validate_historical_import_checkpoint(&mut self) -> Result<(), CvaError> {
        if let Some(cp) = self.freshness.import_checkpoint().cloned() {
            input::validate_checkpoint(self, &cp)?;
        }
        Ok(())
    }
}

#[path = "historical_reconstruction.rs"]
mod reconstruction;
pub use reconstruction::{
    HistoricalReconstructionError, HistoricalReconstructionOptions, HistoricalReconstructionReport,
};

#[cfg(test)]
#[path = "historical_import_tests.rs"]
mod tests;

impl Cva {
    fn historical_reconstruction_owner(&self) -> Result<[u8; 16], CvaError> {
        let owner = self
            .owner_uuid()
            .ok_or_else(|| fail("historical import requires a durable REL owner"))?;
        if self.rel_turn_count() == 0
            || self.freshness_policy() != FreshnessPolicy::default_v1()
            || self.freshness_has_pending_dream_passes()
        {
            return Err(fail(
                "unsupported import owner/policy or unfinished generation",
            ));
        }
        let stats = self.insomnia_stats();
        if stats.pending + stats.processing + stats.failed + stats.terminal != 0 {
            return Err(fail("import Insomnia generation is not complete"));
        }
        Ok(owner)
    }
}
