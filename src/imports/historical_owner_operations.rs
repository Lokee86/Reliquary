//! Import-owned orchestration over concrete mounted REL owner contracts.
use crate::freshness::FreshnessRecord;
use crate::owner_lifecycle::ReliquaryOwner;
use crate::{CvaError, FreshnessDueDispatcher, MemoryId};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn reconstruct(
    owner: &mut ReliquaryOwner,
    options: crate::HistoricalReconstructionOptions,
) -> Result<
    (
        crate::HistoricalReconstructionReport,
        Option<FreshnessDueDispatcher>,
    ),
    crate::HistoricalReconstructionError,
> {
    let open_epoch = if options.fill_missing_embeddings {
        owner.import_epochs.open().cloned()
    } else {
        None
    };
    let accepted_settlement_exists = if options.fill_missing_embeddings && open_epoch.is_none() {
        Some(has_accepted_settlement(owner, None))
    } else {
        None
    };
    crate::imports::historical_reconstruction::validate_initial_fill_admission(
        options.fill_missing_embeddings,
        open_epoch.as_ref(),
        accepted_settlement_exists,
    )?;
    let frozen = prepare(owner, false)?;
    let prepared = crate::imports::historical_reconstruction::prepare_without_endpoint(
        frozen,
        options.fill_missing_embeddings,
    )?;
    let due = commit(owner, &prepared)?;
    Ok((
        crate::imports::historical_reconstruction::report(&prepared, due.is_some(), None),
        due,
    ))
}

pub(crate) fn prepare(
    owner: &ReliquaryOwner,
    requested_complete: bool,
) -> Result<
    crate::imports::historical_settlement_plan::FrozenHistoricalImport,
    crate::owner_lifecycle::OwnerError,
> {
    let history = owner.history_read_access();
    let admission = crate::history::import_admission::ImportAdmissionReadParts {
        access: owner.history_read_access(),
        archive: &owner.archive,
        memories: &owner.memories,
        freshness: &owner.freshness,
        dream: &owner.dream_freshness,
    };
    let freshness = owner.historical_freshness_read_access();
    let dream = crate::history::dream_evidence::DreamEvidenceReadAccess::new(
        &history,
        &owner.graph,
        &owner.dream_freshness,
    );
    let vectors = owner.historical_vector_read_access();
    #[cfg(feature = "freshness-performance-trace")]
    let mut traces = Vec::new();
    let result = crate::imports::historical_prepare::prepare(
        crate::imports::historical_manifest::HistoricalManifestReadViews {
            history: &history,
            archive: &owner.archive,
            memories: &owner.memories,
            admission: &admission,
            freshness: &freshness,
            dream: &dream,
            vectors: &vectors,
            cut: crate::imports::historical_manifest::ManifestOwnerCut {
                owner_uuid: history.owner_uuid(),
                global_version: owner.latest_global_version(),
                archive_version: owner.archive.archive_version(),
                activity_cut: owner.archive.rel_turn_count(),
                memory_version: owner.memories.memory_version(),
                graph_version: owner.owner_graph_version(),
                memory_graph_version: owner.memory_graph_version(),
                vector_version: owner.vector_generation_vector_version(),
                community_snapshot: owner.community_snapshot(),
                insomnia: owner.insomnia_stats(),
            },
        },
        owner.import_epochs.open().cloned(),
        requested_complete,
        #[cfg(feature = "freshness-performance-trace")]
        &mut traces,
    );
    #[cfg(feature = "freshness-performance-trace")]
    for trace in traces {
        crate::freshness::record_dream_evidence_scan(
            trace.elapsed_ns,
            trace.payload_bytes,
            trace.object_bytes,
            trace.chunks,
            trace.mutations,
            trace.filtered,
        );
    }
    result
}

pub(crate) fn commit(
    owner: &mut ReliquaryOwner,
    prepared: &crate::imports::historical_settlement_plan::PreparedHistoricalImport,
) -> Result<Option<FreshnessDueDispatcher>, crate::owner_lifecycle::OwnerError> {
    let cut = crate::imports::historical_freeze::HistoricalOwnerCut {
        owner: prepared.settlement.owner,
        activity: owner.archive.rel_turn_count(),
        global: owner.latest_global_version(),
        archive: owner.archive.archive_version(),
        graph: owner.owner_graph_version(),
        memory_graph: owner.memory_graph_version(),
    };
    let open_epoch = owner.import_epochs.open().cloned();
    let owners = crate::history::HistoricalSettlementReadOwners::new(
        &owner.archive,
        &owner.memories,
        &owner.graph,
        &owner.dream_freshness,
        &owner.import_epochs,
        &owner.freshness,
    );
    let mut writer =
        ReliquaryOwner::historical_settlement_write_access(&mut owner.container, &owner.freshness);
    let Some(publication) =
        crate::imports::historical_commit::commit(&mut writer, &owners, prepared, cut, open_epoch)?
    else {
        return Ok(None);
    };
    drop(writer);
    Ok(Some(owner.install_historical_settlement(publication)))
}

pub(crate) fn historical_import_base(
    owner: &ReliquaryOwner,
    scope: &crate::imports::ImportEpochScope,
) -> Result<BTreeMap<MemoryId, FreshnessRecord>, CvaError> {
    let source = owner
        .history_source()
        .map_err(|error| CvaError::Container(error.0))?;
    let external_birth_facts = crate::owner_lifecycle::import_birth_facts::external_birth_facts(
        &source,
        scope.start_offset,
    );
    crate::history::historical_freshness_base(
        &source,
        &owner.memories,
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
        crate::history::HistoricalFreshnessBaseError::Source(error) => CvaError::Container(error.0),
    })
}

pub(crate) fn import_epoch_scope(
    owner: &ReliquaryOwner,
) -> Result<crate::history::ImportEpochScope, CvaError> {
    let start = owner.import_epochs.open().cloned().ok_or_else(|| {
        CvaError::Freshness("historical reconstruction requires an open import epoch".into())
    })?;
    crate::history::read_import_epoch_scope(
        &owner.history_read_access(),
        &owner.archive,
        &owner.memories,
        start,
    )
    .map_err(CvaError::from)
}

pub(crate) fn validate_semantic_scope(
    owner: &ReliquaryOwner,
    settlement: &crate::freshness_storage::HistoricalImportFreshnessSettlement,
    scope: &crate::freshness_storage::ImportSemanticScope,
) -> Result<(), CvaError> {
    let prepared = crate::history::prepare_semantic_import_scope(
        owner.owner_uuid(),
        &owner.archive,
        &owner.import_epochs,
        settlement,
        scope,
    )?;
    match prepared {
        crate::history::PreparedSemanticImportScope::InitialHistory => Ok(()),
        crate::history::PreparedSemanticImportScope::Framed(facts) => {
            let source = owner
                .history_source()
                .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?;
            crate::history::validate_semantic_import_births(
                &source,
                &owner.archive,
                &owner.memories,
                facts,
            )
        }
    }
    .map_err(CvaError::from)
}

pub(crate) fn corpus_digest(owner: &ReliquaryOwner, through: u64) -> Result<[u8; 32], CvaError> {
    let parts = admission_parts(owner);
    crate::history::import_admission::corpus_digest(&parts, through).map_err(CvaError::from)
}

pub(crate) fn scoped_corpus_digest(
    owner: &ReliquaryOwner,
    source_nodes: &BTreeSet<(String, String)>,
) -> Result<[u8; 32], CvaError> {
    let parts = admission_parts(owner);
    crate::history::import_admission::scoped_corpus_digest(&parts, source_nodes)
        .map_err(CvaError::from)
}

pub(crate) fn admission_proof(
    owner: &ReliquaryOwner,
) -> Result<crate::history::import_admission::AdmissionProof, CvaError> {
    crate::history::import_admission::admission_proof(&admission_parts(owner))
        .map_err(CvaError::from)
}

pub(crate) fn admission_proof_scoped(
    owner: &ReliquaryOwner,
    scope: Option<&crate::imports::ImportEpochScope>,
) -> Result<crate::history::import_admission::AdmissionProof, CvaError> {
    crate::history::import_admission::admission_proof_scoped(&admission_parts(owner), scope)
        .map_err(CvaError::from)
}

pub(crate) fn has_accepted_settlement(owner: &ReliquaryOwner, import_id: Option<&str>) -> bool {
    let freshness = owner.historical_freshness_read_access();
    match import_id {
        Some(import_id) => freshness.has_accepted_settlement_hex(import_id),
        None => freshness.has_accepted_settlements(),
    }
}

fn admission_parts(
    owner: &ReliquaryOwner,
) -> crate::history::import_admission::ImportAdmissionReadParts<'_> {
    crate::history::import_admission::ImportAdmissionReadParts {
        access: owner.history_read_access(),
        archive: &owner.archive,
        memories: &owner.memories,
        freshness: &owner.freshness,
        dream: &owner.dream_freshness,
    }
}
