//! Import-owned frozen settlement construction over accepted owner evidence.
use super::historical_event_plan::NativeHistoricalEvents;
use super::historical_manifest_model::Manifest;
use super::historical_settlement_plan::FrozenHistoricalImport;
use crate::MemoryId;
use crate::freshness::FreshnessRecord;
use crate::freshness_storage::{
    HistoricalImportFreshnessScope, HistoricalImportFreshnessSettlement, ImportEventScope,
};
use crate::owner_lifecycle::OwnerError;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct HistoricalOwnerCut {
    pub(crate) owner: [u8; 16],
    pub(crate) activity: u64,
    pub(crate) global: u64,
    pub(crate) archive: u64,
    pub(crate) graph: u64,
    pub(crate) memory_graph: u64,
}
pub(crate) fn accepted_retry(
    settlement: HistoricalImportFreshnessSettlement,
) -> Result<FrozenHistoricalImport, OwnerError> {
    let mut initial = BTreeMap::new();
    for (id, record) in &settlement.records {
        let mut birth = FreshnessRecord::created();
        if let Some(at) = record.admitted_at_turn {
            birth
                .admit(at)
                .map_err(|error| OwnerError::Freshness(error.to_string()))?;
        }
        initial.insert(*id, birth);
    }
    Ok(FrozenHistoricalImport {
        source_bytes: 0,
        source_digest: [0; 32],
        retained: Vec::new(),
        epoch: false,
        events: None,
        propagation: None,
        settlement,
        initial,
    })
}
/// Called after owner evidence has synchronized at the existing freeze boundary.
pub(crate) fn from_manifest(
    manifest: Manifest,
    events: NativeHistoricalEvents,
    cut: HistoricalOwnerCut,
) -> FrozenHistoricalImport {
    let settlement = HistoricalImportFreshnessSettlement::from_import_scope(
        HistoricalImportFreshnessScope {
            owner: cut.owner,
            import_id: manifest.import_id,
            start_evidence_digest: manifest.start_evidence_digest,
            base_cut: manifest.base_cut,
            settlement_cut: cut.activity,
        },
        manifest.corpus_digest,
        manifest.digest,
        manifest.digest,
        manifest.cohort,
        cut.global,
        manifest.input_bytes,
        cut.archive,
        cut.graph,
        cut.memory_graph,
        BTreeMap::new(),
        manifest.superseded,
    );
    FrozenHistoricalImport {
        source_bytes: manifest.input_bytes,
        source_digest: manifest.input_digest,
        retained: Vec::new(),
        epoch: manifest.epoch,
        events: Some(events),
        propagation: Some(manifest.propagation),
        settlement,
        initial: manifest.initial,
    }
}
pub(crate) fn bind_result_cohort(frozen: &mut FrozenHistoricalImport) -> Result<(), OwnerError> {
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
    Ok(())
}
pub(crate) fn framed_scope(
    frozen: &FrozenHistoricalImport,
    scope: crate::history::ImportEpochScope,
) -> ImportEventScope {
    ImportEventScope {
        owner: frozen.settlement.owner,
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
    }
}
pub(crate) fn unframed_scope(
    frozen: &FrozenHistoricalImport,
    source_nodes: BTreeSet<(String, String)>,
) -> ImportEventScope {
    ImportEventScope {
        owner: frozen.settlement.owner,
        import_id: frozen.settlement.import_id,
        start_evidence_digest: frozen.settlement.start_evidence_digest,
        born_memories: frozen.settlement.cohort.clone(),
        affected_recipients: frozen.settlement.cohort.clone(),
        source_nodes,
        start_prefix: 0,
        freeze_prefix: frozen.source_bytes,
        base_cut: frozen.settlement.base_cut,
        freeze_cut: frozen.cut(),
    }
}

pub(crate) fn attach_semantic_evidence(
    frozen: &mut FrozenHistoricalImport,
    reader: &crate::owner_lifecycle::historical_freshness_read::HistoricalFreshnessReadAccess<'_>,
    replay: &ImportEventScope,
) -> Result<(), OwnerError> {
    let (retained, semantic) = reader.settlement_evidence(
        replay,
        &frozen.settlement.cohort,
        &frozen.settlement.superseded,
        frozen.epoch,
    )?;
    frozen.retained = retained;
    frozen.settlement.semantic = Some(semantic);
    Ok(())
}

/// Accepted retry resolves before representation, drift and scoped-source checks.
pub(crate) fn admit_prepared(
    owner: Option<[u8; 16]>,
    prepared: &super::historical_settlement_plan::PreparedHistoricalImport,
    accepted: Option<&HistoricalImportFreshnessSettlement>,
) -> Result<Option<bool>, OwnerError> {
    let settlement = &prepared.settlement;
    if owner != Some(settlement.owner) {
        return Err(OwnerError::Freshness(
            "import settlement owner mismatch".into(),
        ));
    }
    if let Some(old) = accepted {
        return if old == settlement {
            Ok(Some(false))
        } else {
            Err(OwnerError::Freshness("ImportManifestConflict".into()))
        };
    }
    if settlement.semantic.is_none() {
        return Err(OwnerError::Freshness(
            "historical settlement representation/scope mismatch".into(),
        ));
    }
    Ok(None)
}
pub(crate) fn validate_owner_cut(
    prepared: &super::historical_settlement_plan::PreparedHistoricalImport,
    cut: HistoricalOwnerCut,
) -> Result<(), OwnerError> {
    let settlement = &prepared.settlement;
    if cut.global != settlement.global_version
        || cut.archive != settlement.archive_version
        || cut.activity != settlement.settlement_cut
        || cut.graph != settlement.graph_version
        || cut.memory_graph != settlement.memory_graph_version
    {
        return Err(OwnerError::Freshness(
            "ImportInputDrift: discard and refreeze the attempt".into(),
        ));
    }
    Ok(())
}
pub(crate) fn validate_start_binding(
    prepared: &super::historical_settlement_plan::PreparedHistoricalImport,
    start_digest: [u8; 32],
) -> Result<(), OwnerError> {
    if start_digest != prepared.settlement.start_evidence_digest {
        return Err(OwnerError::Freshness(
            "historical settlement does not belong to the open epoch".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_physical_freeze(
    prepared: &super::historical_settlement_plan::PreparedHistoricalImport,
    source: &crate::owner_lifecycle::HistoryReadAccess<'_>,
) -> Result<(), OwnerError> {
    if source
        .path_length()
        .map_err(|error| OwnerError::Freshness(error.to_string()))?
        != prepared.source_bytes
        || source.file_digest()? != prepared.source_digest
    {
        return Err(OwnerError::Freshness(
            "ImportInputDrift: discard and refreeze the attempt".into(),
        ));
    }
    Ok(())
}
