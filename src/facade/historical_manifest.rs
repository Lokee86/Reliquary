//! H1 evidence assembled only while the Cva owner is exclusively borrowed.
use super::{
    encoding::{Value, policy},
    fail, file_digest, input,
};
pub(super) use crate::history::community_proof::community_proof;
use crate::{Cva, CvaError, MemoryId, freshness::FreshnessRecord};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(super) use crate::imports::historical_manifest_model::Manifest;
pub(super) use crate::imports::historical_settlement_plan::PropagationInputs;
fn graph_proof(
    cva: &mut Cva,
    ids: &BTreeSet<MemoryId>,
    cohort: &BTreeSet<MemoryId>,
    scoped: bool,
) -> Result<(Value, crate::freshness::MemoryAdjacency), CvaError> {
    let proof = crate::history::graph_proof::read_graph_admission(
        &crate::owner_lifecycle::HistoryReadAccess::new(&cva.container),
        &cva.archive,
        &cva.memories,
        &cva.dream_freshness,
        &cva.graph,
        ids,
        cohort,
        scoped,
    );
    #[cfg(feature = "freshness-performance-trace")]
    for trace in proof.traces {
        crate::cva_freshness::record_dream_evidence_trace(trace);
    }
    proof.result
}
fn vector_proof(
    cva: &mut Cva,
    order: &[crate::freshness::historical_planner::SourceKey],
    requested_complete: bool,
    target_ids: &BTreeSet<MemoryId>,
    scoped_nodes: Option<&BTreeSet<(String, String)>>,
    scoped_base_cut: Option<u64>,
) -> Result<(Value, Vec<Value>), CvaError> {
    use crate::owner_lifecycle::semantic_source::{
        HistoricalVectorReadAccess, SemanticSourceReadParts,
    };
    let vectors = HistoricalVectorReadAccess::new(
        SemanticSourceReadParts {
            container: &cva.container,
            archive: &cva.archive,
            freshness: &cva.freshness,
            generations: &cva.vector_generations,
            compatibility_profiles: &cva.compatibility_profiles,
            archive_vectors: &cva.archive_vectors,
            packed_vectors: &cva.packed_vectors,
        },
        &cva.memory_vectors,
    );
    let proof = crate::history::vector_proof::read_vector_admission(
        &cva.archive,
        &cva.memories,
        &vectors,
        order,
        target_ids,
        scoped_nodes,
        scoped_base_cut,
    )?;
    let embedding_pass = if requested_complete {
        super::embedding::manifest_pass(cva, proof.fragments_for_embedding())?
    } else {
        Value::text("not-requested")
    };
    Ok(proof.finish(embedding_pass))
}
#[cfg(test)]
pub(super) fn build(cva: &mut Cva) -> Result<Manifest, CvaError> {
    build_with_fill(cva, false)
}
pub(super) fn build_with_fill(
    cva: &mut Cva,
    requested_complete: bool,
) -> Result<Manifest, CvaError> {
    let scope = if cva.open_import_epoch().is_some() {
        Some(cva.import_epoch_scope()?)
    } else {
        None
    };
    let owner = if let Some(scope) = &scope {
        let owner = crate::imports::historical_manifest_plan::validate_epoch_owner(
            cva.owner_uuid(),
            scope.start.owner_uuid,
        )?;
        crate::imports::historical_manifest_plan::validate_epoch_policy(cva.freshness_policy())?;
        owner
    } else {
        cva.historical_reconstruction_owner()?
    };
    // Validate ancestry before the timeline planner follows parent chains.
    input::required_nodes(&cva.archive, cva.rel_turn_count())?;
    let timeline = cva
        .historical_freshness_timeline()
        .map_err(|e| fail(&e.to_string()))?;
    let corpus_digest = if let Some(scope) = &scope {
        input::scoped_corpus_digest(cva, &scope.source_nodes)?
    } else {
        input::corpus_digest(cva, cva.rel_turn_count())?
    };
    let import_id = scope.as_ref().map_or_else(
        || historical_import_id(owner, corpus_digest),
        |scope| crate::imports::historical_manifest_plan::decode_import_id(&scope.start.import_id),
    );
    let admission = input::admission_proof_scoped(cva, scope.as_ref())?;
    let born_ids: BTreeSet<_> = scope.as_ref().map_or_else(
        || cva.memory_ids().into_iter().collect(),
        |scope| scope.cohort.clone(),
    );
    for &id in &born_ids {
        let publication_birth = cva.freshness_is_publication_birth(id);
        let settled = publication_birth
            && cva.initial_dream_pass_settled(&crate::dream_pass_identity::initial_pass_id(
                owner, id,
            ));
        crate::imports::historical_manifest_plan::validate_settled_birth(
            publication_birth,
            settled,
        )?;
    }
    crate::imports::historical_manifest_plan::validate_supported_cohort(
        scope.is_some(),
        admission.initial.is_empty(),
    )?;
    let owner_ids: BTreeSet<_> = if scope.is_none() {
        cva.memory_ids().into_iter().collect()
    } else {
        BTreeSet::new()
    };
    let ids: BTreeSet<_> = admission.memory_inputs.keys().copied().collect();
    let replacement_ids: BTreeSet<_> = admission.initial.keys().copied().collect();
    let scoped_event_evidence = if let Some(scope) = &scope {
        let freeze_prefix = std::fs::metadata(cva.container.path())
            .map_err(|error| fail(&error.to_string()))?
            .len();
        let mut frozen = cva.freshness.clone();
        frozen
            .bind_history_source(&cva.container, freeze_prefix)
            .map_err(CvaError::Freshness)?;
        Some(
            frozen
                .import_event_evidence_in_scope(&crate::freshness_storage::ImportEventScope {
                    owner,
                    import_id,
                    start_evidence_digest: scope.start_digest,
                    born_memories: scope.cohort.clone(),
                    affected_recipients: replacement_ids.clone(),
                    source_nodes: scope.source_nodes.clone(),
                    start_prefix: scope.marker_end,
                    freeze_prefix,
                    base_cut: scope.base_cut,
                    freeze_cut: cva.rel_turn_count(),
                })
                .map_err(CvaError::Freshness)?,
        )
    } else {
        None
    };
    let superseded = if let Some(evidence) = &scoped_event_evidence {
        evidence.event_ids.clone()
    } else {
        cva.freshness
            .provisional_import_events(&replacement_ids, 0, cva.rel_turn_count())
            .map_err(CvaError::Freshness)?
    };
    let replay_entries = if scope.is_some() {
        Vec::new()
    } else {
        cva.freshness
            .replay_entries()
            .map_err(CvaError::Freshness)?
    };
    let replacement = crate::imports::historical_manifest_plan::replacement_proof(
        scope.is_some(),
        &ids,
        &born_ids,
        &owner_ids,
        &admission.initial,
        &superseded,
        scoped_event_evidence
            .as_ref()
            .map(|evidence| &evidence.affected_recipients),
        &replay_entries,
    )?;
    let (graph, adjacency) = graph_proof(cva, &ids, &born_ids, scope.is_some())?;
    let snapshot = cva.community_snapshot();
    let scoped_snapshot = if scope.is_some() {
        snapshot.clone().map(|mut snapshot| {
            for community in &mut snapshot.communities {
                community.members.retain(|id| ids.contains(id));
            }
            snapshot
                .communities
                .retain(|community| !community.members.is_empty());
            snapshot
        })
    } else {
        snapshot.clone()
    };
    let community = community_proof(scoped_snapshot.clone(), cva.memory_graph_version(), &ids)?;
    let memberships = scoped_snapshot
        .filter(|s| s.derived_graph_version == cva.memory_graph_version())
        .into_iter()
        .flat_map(|s| s.communities)
        .flat_map(|c| c.members.into_iter().map(move |id| (id, c.id)))
        .collect();
    let memory_facts = admission
        .memory_inputs
        .iter()
        .map(|(&id, input)| (id, (input.availability, input.credit_eligible)))
        .collect();
    let propagation = crate::imports::historical_manifest_plan::authorize_propagation(
        adjacency,
        memberships,
        &memory_facts,
    );
    let (vectors, sources) = vector_proof(
        cva,
        &timeline.activity_order,
        requested_complete,
        &ids,
        scope.as_ref().map(|scope| &scope.source_nodes),
        scope.as_ref().map(|scope| scope.base_cut),
    )?;
    let order = Value::Array(
        timeline
            .activity_order
            .iter()
            .map(|k| input::source_key(&k.conversation_id, &k.node_id))
            .collect(),
    );
    let historical_policy = policy();
    let policy_digest = historical_policy.digest("reliquary:historical-policy:v1")?;
    if policy_digest != crate::freshness::historical_planner::HISTORICAL_POLICY_DIGEST {
        return Err(fail("unsupported historical policy digest"));
    }
    let base = cva.freshness_policy();
    let mut base_bytes = Vec::new();
    base_bytes.extend(base.decay_turns_per_point.to_le_bytes());
    for n in [
        base.access_principal,
        base.linkage_principal,
        base.local_hop_cost,
        base.outside_hop_cost,
    ] {
        base_bytes.extend(n.to_le_bytes());
    }
    let mut base_hash = Sha256::new();
    base_hash.update(b"reliquary:freshness-policy:v1\0");
    base_hash.update(&base_bytes);
    let semantic = crate::freshness::semantic_policy::SemanticPolicy::default();
    cva.container.sync()?;
    let input_bytes = std::fs::metadata(cva.container.path())
        .map_err(|e| fail(&e.to_string()))?
        .len();
    let prefix = file_digest(cva.container.path())?;
    if std::fs::metadata(cva.container.path())
        .map_err(|e| fail(&e.to_string()))?
        .len()
        != input_bytes
        || file_digest(cva.container.path())? != prefix
    {
        return Err(fail("ImportInputDrift: discard and refreeze the manifest"));
    }
    let semantic_descriptor = semantic
        .canonical_descriptor()
        .map_err(|error| CvaError::Freshness(error.to_string()));
    let semantic_digest = semantic
        .digest()
        .map_err(|error| CvaError::Freshness(error.to_string()));
    let encoded = crate::history::manifest_proof::build(
        crate::history::manifest_proof::ManifestProofParts {
            owner,
            import_id,
            corpus_digest,
            global_version: cva.latest_global_version(),
            archive_version: cva.archive_version(),
            activity_cut: cva.rel_turn_count(),
            memory_version: cva.memory_version(),
            graph_version: cva.graph_version(),
            memory_graph_version: cva.memory_graph_version(),
            vector_version: cva.vector_version(),
            community_generation: cva.community_snapshot().map(|snapshot| snapshot.generation),
            graph,
            replacement: replacement.value,
            order,
            last_source: timeline
                .activity_order
                .last()
                .map(|key| (key.conversation_id.clone(), key.node_id.clone())),
            source_records: sources,
            memory_records: admission.memories,
            cohort_records: admission.cohorts,
            community,
            vector: vectors,
            base_descriptor: base_bytes,
            base_digest: base_hash.finalize().into(),
            semantic_descriptor,
            semantic_digest,
            historical_policy,
            historical_digest: policy_digest,
            scope: scope
                .as_ref()
                .map(|scope| crate::history::manifest_proof::ImportScopeProof {
                    start_digest: scope.start_digest,
                    base_cut: scope.base_cut,
                    import_key: scope.start.import_key.clone(),
                    source_nodes: scope.source_nodes.clone(),
                    episodes: scope.episodes.clone(),
                    recipient_cohort: replacement.cohort.clone(),
                }),
        },
    )?;
    let value = encoded.value;
    let digest = encoded.digest;
    Ok(Manifest {
        propagation,
        input_digest: prefix,
        input_bytes,
        memory_inputs: admission.memory_inputs,
        timeline,
        digest,
        corpus_digest,
        import_id,
        initial: admission.initial,
        superseded,
        epoch: scope.is_some(),
        cohort: replacement.cohort,
        base_cut: scope.as_ref().map_or(0, |scope| scope.base_cut),
        start_evidence_digest: scope.as_ref().map_or(prefix, |scope| scope.start_digest),
        #[cfg(test)]
        value,
    })
}

pub(super) fn historical_import_id(owner: [u8; 16], corpus: [u8; 32]) -> [u8; 32] {
    crate::imports::historical_manifest_plan::historical_import_id(owner, corpus)
}
