//! A frozen one-shot native REL import and its atomic Freshness publication.
use crate::freshness::historical_scorer::{HistoricalScoreEvent, score_historical_records};
use crate::freshness_storage::{
    HistoricalImportFreshnessScope, HistoricalImportFreshnessSettlement,
};
use crate::{
    Cva, CvaError, MemoryId,
    freshness::{FreshnessPolicy, FreshnessRecord},
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;
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
#[path = "historical_import_base.rs"]
mod historical_base;
#[path = "historical_import_input.rs"]
mod input;
#[path = "historical_manifest.rs"]
mod manifest;
#[path = "historical_import_transform.rs"]
mod transform;

#[derive(Clone, Debug)]
struct FrozenHistoricalImport {
    retained: Vec<HistoricalScoreEvent>,
    epoch: bool,
    events: Option<events::NativeHistoricalEvents>,
    propagation: Option<manifest::PropagationInputs>,
    settlement: HistoricalImportFreshnessSettlement,
    initial: BTreeMap<MemoryId, FreshnessRecord>,
}
#[derive(Clone, Debug)]
struct PreparedHistoricalImport {
    epoch: bool,
    settlement: HistoricalImportFreshnessSettlement,
}
impl FrozenHistoricalImport {
    /// Only the owner-derived batch can enter production scoring.
    fn prepare_native(&self) -> Result<PreparedHistoricalImport, CvaError> {
        let Some(_) = &self.events else {
            return Ok(PreparedHistoricalImport {
                epoch: self.epoch,
                settlement: self.settlement.clone(),
            });
        };
        let mut scores = self.native_scores()?;
        scores.extend(self.retained.clone());
        self.prepare(&scores)
    }
    fn native_scores(&self) -> Result<Vec<HistoricalScoreEvent>, CvaError> {
        let Some(batch) = &self.events else {
            return Ok(Vec::new());
        };
        let inputs = self
            .propagation
            .as_ref()
            .ok_or_else(|| fail("missing frozen propagation proof"))?;
        if batch
            .ordered
            .iter()
            .any(|(_, event)| event.position > self.settlement.settlement_cut)
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
        Ok(scores)
    }
    fn import_id(&self) -> [u8; 32] {
        self.settlement.import_id
    }
    fn cut(&self) -> u64 {
        self.settlement.settlement_cut
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
        let mut settlement = self.settlement.clone();
        settlement.records = if self.epoch && self.initial.is_empty() && events.is_empty() {
            BTreeMap::new()
        } else {
            score_historical_records(
                &self.initial,
                events,
                settlement.settlement_cut,
                &FreshnessPolicy::default_v1(),
            )
            .map_err(|e| CvaError::Freshness(e.to_string()))?
        };
        settlement.validate().map_err(CvaError::Freshness)?;
        Ok(PreparedHistoricalImport {
            epoch: self.epoch,
            settlement,
        })
    }
}
impl PreparedHistoricalImport {
    fn records(&self) -> &BTreeMap<MemoryId, FreshnessRecord> {
        &self.settlement.records
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
// All ordinary backing records are immutable except Archive conversation
// compaction allocations. Bind physical coordinates as well as payloads so
// relocation cannot silently preserve an invalid epoch boundary.
fn immutable_prefix_digest(path: &std::path::Path, through: u64) -> Result<[u8; 32], CvaError> {
    let mut scan = crate::Container::open_read_only_for_scan(path)?;
    let mut hash = Sha256::new();
    hash.update(b"reliquary:import-immutable-prefix:v1");
    scan.visit_payloads_through::<CvaError>(through, |object, payload| {
        if !crate::conversation_compaction_codec::is_compaction_payload(payload) {
            hash.update(object.legacy_bytes());
            hash.update(payload);
        }
        Ok(())
    })?;
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
        // An earlier import's settlement is not a retry of the open epoch.
        // Resolve the durable epoch identity before considering accepted evidence.
        let accepted = if let Some(start) = self.open_import_epoch() {
            self.freshness
                .import_settlements()
                .iter()
                .find(|settlement| {
                    settlement
                        .import_id
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                        == start.import_id
                })
        } else {
            self.freshness.latest_import_settlement()
        };
        if let Some(settlement) = accepted {
            let settlement = settlement.clone();
            if self.open_import_epoch().is_some()
                && self.import_epoch_scope()?.start_digest != settlement.start_evidence_digest
            {
                return Err(fail("import settlement start evidence mismatch"));
            }
            let mut initial = BTreeMap::new();
            for (id, record) in &settlement.records {
                let mut birth = FreshnessRecord::created();
                if let Some(at) = record.admitted_at_turn {
                    birth.admit(at).map_err(|e| fail(&e.to_string()))?;
                }
                initial.insert(*id, birth);
            }
            return Ok(FrozenHistoricalImport {
                retained: Vec::new(),
                epoch: false,
                events: None,
                propagation: None,
                settlement,
                initial,
            });
        }
        let manifest = manifest::build_with_fill(self, requested_complete)?;
        let owner = self
            .owner_uuid()
            .ok_or_else(|| fail("historical import requires a durable REL owner"))?;
        let events = events::build(self, &manifest)?;
        let cohort = manifest.cohort.clone();
        self.container.sync()?;
        let settlement = HistoricalImportFreshnessSettlement::from_import_scope(
            HistoricalImportFreshnessScope {
                owner,
                import_id: manifest.import_id,
                start_evidence_digest: manifest.start_evidence_digest,
                base_cut: manifest.base_cut,
                settlement_cut: self.rel_turn_count(),
            },
            manifest.corpus_digest,
            manifest.digest,
            manifest.input_digest,
            cohort,
            self.latest_global_version(),
            manifest.input_bytes,
            self.archive_version(),
            self.graph_version(),
            self.memory_graph_version(),
            BTreeMap::new(),
            manifest.superseded,
        );
        let mut frozen = FrozenHistoricalImport {
            retained: Vec::new(),
            epoch: manifest.epoch,
            events: Some(events),
            propagation: Some(manifest.propagation),
            settlement,
            initial: manifest.initial,
        };
        if frozen.epoch {
            // The manifest binds provisional attribution and the full recipient proof.
            // Bind planned propagation recipients separately before retaining result records.
            let affected = frozen
                .native_scores()?
                .into_iter()
                .map(|event| event.recipient);
            frozen.settlement.cohort.extend(affected);
            frozen
                .initial
                .retain(|id, _| frozen.settlement.cohort.contains(id));
            let mut binding = Sha256::new();
            binding.update(b"import-epoch-result-cohort-v1");
            binding.update(frozen.settlement.manifest_digest);
            for id in &frozen.settlement.cohort {
                binding.update(id.0);
            }
            frozen.settlement.manifest_digest = binding.finalize().into();
            let scope = self.import_epoch_scope()?;
            let replay = crate::freshness_storage::ImportEventScope {
                owner,
                import_id: frozen.settlement.import_id,
                start_evidence_digest: scope.start_digest,
                born_memories: scope.cohort,
                affected_recipients: frozen
                    .propagation
                    .as_ref()
                    .unwrap()
                    .availability
                    .keys()
                    .copied()
                    .collect(),
                source_nodes: scope.source_nodes,
                start_prefix: scope.start_offset,
                freeze_prefix: frozen.settlement.input_bytes,
                base_cut: scope.base_cut,
                freeze_cut: frozen.cut(),
            };
            let mut history = self.freshness.clone();
            history.bind_history_path(self.container.path(), replay.freeze_prefix);
            frozen.retained = history
                .retained_import_scores(
                    &replay,
                    &frozen.settlement.cohort,
                    &frozen.settlement.superseded,
                )
                .map_err(CvaError::Freshness)?;
            frozen.settlement.epoch_prefix_digest =
                immutable_prefix_digest(self.container.path(), replay.freeze_prefix)?;
            frozen.settlement.epoch = Some(replay);
        }
        Ok(frozen)
    }
    /// Atomically replace provisional scoring. Exact accepted retries resolve before
    /// input drift checks and never erase legitimate post-checkpoint live events.
    fn commit_historical_import(
        &mut self,
        prepared: &PreparedHistoricalImport,
    ) -> Result<bool, CvaError> {
        let settlement = &prepared.settlement;
        if self.owner_uuid() != Some(settlement.owner) {
            return Err(fail("import settlement owner mismatch"));
        }
        if let Some(old) = self.freshness.import_settlement(settlement.import_id) {
            return if old == settlement {
                Ok(false)
            } else {
                Err(fail("ImportManifestConflict"))
            };
        }
        if prepared.epoch != settlement.epoch.is_some() {
            return Err(fail("historical settlement representation/scope mismatch"));
        }
        if self.latest_global_version() != settlement.global_version
            || self.archive_version() != settlement.archive_version
            || self.rel_turn_count() != settlement.settlement_cut
            || self.graph_version() != settlement.graph_version
            || self.memory_graph_version() != settlement.memory_graph_version
            || std::fs::metadata(self.container.path())
                .map_err(|e| fail(&e.to_string()))?
                .len()
                != settlement.input_bytes
            || file_digest(self.container.path())? != settlement.input_digest
        {
            return Err(fail("ImportInputDrift: discard and refreeze the attempt"));
        }
        if prepared.epoch {
            let current = self.import_epoch_scope()?;
            if current.start_digest != settlement.start_evidence_digest {
                return Err(fail(
                    "historical settlement does not belong to the open epoch",
                ));
            }
            input::validate_settlement(self, settlement)?;
        }
        let payload = settlement.encode();
        let mut staged = self.freshness.clone();
        if prepared.epoch {
            staged.bind_history_path(self.container.path(), settlement.input_bytes);
            staged.freeze_derived_birth_policy();
        }
        staged
            .ingest_import_settlement(&payload)
            .map_err(CvaError::Freshness)?;
        let mut due = crate::FreshnessDueDispatcher::default();
        due.rebuild(
            staged.records().iter().map(|(&id, &r)| (id, r)),
            settlement.settlement_cut,
            &self.freshness_policy(),
        )
        .map_err(|e| fail(&e.to_string()))?;
        // Admit only the authenticated settlement append; no worker or completion authority.
        self.container.set_owner_write_scope(prepared.epoch);
        let append_result = (|| {
            staged
                .persist_policy_if_needed(&mut self.container, settlement.owner)
                .map_err(CvaError::Freshness)?;
            self.container.append(&payload)?;
            self.container.sync()?;
            Ok::<(), CvaError>(())
        })();
        self.container.set_owner_write_scope(false);
        append_result?;
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
    pub(crate) fn validate_historical_import_settlements(&mut self) -> Result<(), CvaError> {
        for settlement in self.freshness.import_settlements().to_vec() {
            input::validate_settlement(self, &settlement)?;
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
        if (self.rel_turn_count() == 0 && self.open_import_epoch().is_none())
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

#[cfg(test)]
#[path = "historical_import_attribution_tests.rs"]
mod attribution_tests;
