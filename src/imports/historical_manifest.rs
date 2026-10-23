//! Native Import manifest orchestration over narrow immutable owner reads.
use crate::freshness::FreshnessPolicy;
use crate::history::manifest_encoding::Value;
use crate::owner_lifecycle::{HistoryReadAccess, OwnerError};
use crate::{Archive, CommunitySnapshot};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(crate) use super::historical_manifest_model::Manifest;

/// Owner versions and retained snapshots captured by the facade at the same
/// checkpoint as the narrow read capabilities.
pub(crate) struct ManifestOwnerCut {
    pub(crate) owner_uuid: Option<[u8; 16]>,
    pub(crate) global_version: u64,
    pub(crate) archive_version: u64,
    pub(crate) activity_cut: u64,
    pub(crate) memory_version: u64,
    pub(crate) graph_version: u64,
    pub(crate) memory_graph_version: u64,
    pub(crate) vector_version: u64,
    pub(crate) community_snapshot: Option<CommunitySnapshot>,
    pub(crate) insomnia: crate::InsomniaStats,
}

pub(crate) struct HistoricalManifestReadViews<'a> {
    pub(crate) scope: Option<crate::imports::ImportEpochScope>,
    pub(crate) history: &'a HistoryReadAccess<'a>,
    pub(crate) archive: &'a Archive,
    pub(crate) memories: &'a crate::memory_store::MemoryStore,
    pub(crate) admission: &'a crate::history::import_admission::ImportAdmissionReadParts<'a>,
    pub(crate) freshness:
        &'a crate::owner_lifecycle::historical_freshness_read::HistoricalFreshnessReadAccess<'a>,
    pub(crate) dream: &'a crate::history::dream_evidence::DreamEvidenceReadAccess<'a>,
    pub(crate) vectors: &'a crate::owner_lifecycle::semantic_source::HistoricalVectorReadAccess<'a>,
    pub(crate) cut: ManifestOwnerCut,
}

fn fail(message: &str) -> OwnerError {
    OwnerError::Freshness(message.into())
}

pub(crate) fn build(
    views: HistoricalManifestReadViews<'_>,
    requested_complete: bool,
    #[cfg(feature = "freshness-performance-trace")] traces: &mut Vec<
        crate::history::dream_evidence::DreamEvidenceTrace,
    >,
) -> Result<Manifest, OwnerError> {
    let scope = views.scope;
    let owner = if let Some(scope) = &scope {
        let owner = super::historical_manifest_plan::validate_epoch_owner(
            views.cut.owner_uuid,
            scope.start.owner_uuid,
        )?;
        super::historical_manifest_plan::validate_epoch_policy(views.freshness.policy())?;
        owner
    } else {
        let owner = views
            .cut
            .owner_uuid
            .ok_or_else(|| fail("historical import requires a durable REL owner"))?;
        if (views.cut.activity_cut == 0 && scope.is_none())
            || views.freshness.policy() != FreshnessPolicy::default_v1()
            || views.dream.has_pending()
        {
            return Err(fail(
                "unsupported import owner/policy or unfinished generation",
            ));
        }
        let stats = views.cut.insomnia;
        if stats.pending + stats.processing + stats.failed + stats.terminal != 0 {
            return Err(fail("import Insomnia generation is not complete"));
        }
        owner
    };

    // Preserve the original preflight order: owner policy, ancestry, timeline,
    // corpus/import identity, and then native admission proof.
    crate::history::source_proof::required_nodes(views.archive, views.cut.activity_cut)?;
    let timeline = crate::history::source_proof::historical_freshness_timeline(views.archive)
        .map_err(|error| fail(&error.to_string()))?;
    let corpus_digest = if let Some(scope) = &scope {
        crate::history::import_admission::scoped_corpus_digest(
            views.admission,
            &scope.source_nodes,
        )?
    } else {
        crate::history::import_admission::corpus_digest(views.admission, views.cut.activity_cut)?
    };
    let import_id = scope.as_ref().map_or_else(
        || super::historical_manifest_plan::historical_import_id(owner, corpus_digest),
        |scope| super::historical_manifest_plan::decode_import_id(&scope.start.import_id),
    );
    let admission =
        crate::history::import_admission::admission_proof_scoped(views.admission, scope.as_ref())?;
    let born_ids: BTreeSet<_> = scope.as_ref().map_or_else(
        || views.memories.current_ids().into_iter().collect(),
        |scope| scope.cohort.clone(),
    );
    for &id in &born_ids {
        let publication_birth = views.dream.is_publication_birth(id);
        let settled = publication_birth
            && views
                .dream
                .initial_pass_settled(&crate::dream_pass_identity::initial_pass_id(owner, id));
        super::historical_manifest_plan::validate_settled_birth(publication_birth, settled)?;
    }
    super::historical_manifest_plan::validate_supported_cohort(
        scope.is_some(),
        admission.initial.is_empty(),
    )?;
    let owner_ids: BTreeSet<_> = if scope.is_none() {
        views.memories.current_ids().into_iter().collect()
    } else {
        BTreeSet::new()
    };
    let ids: BTreeSet<_> = admission.memory_inputs.keys().copied().collect();
    let replacement_ids: BTreeSet<_> = admission.initial.keys().copied().collect();

    let scoped_event_evidence = if let Some(scope) = &scope {
        let freeze_prefix = views
            .history
            .path_length()
            .map_err(|error| fail(&error.to_string()))?;
        Some(views.freshness.scoped_event_evidence(
            &crate::freshness_storage::ImportEventScope {
                owner,
                import_id,
                start_evidence_digest: scope.start_digest,
                born_memories: scope.cohort.clone(),
                affected_recipients: replacement_ids.clone(),
                source_nodes: scope.source_nodes.clone(),
                start_prefix: scope.marker_end,
                freeze_prefix,
                base_cut: scope.base_cut,
                freeze_cut: views.cut.activity_cut,
            },
        )?)
    } else {
        None
    };
    let superseded = if let Some(evidence) = &scoped_event_evidence {
        evidence.event_ids.clone()
    } else {
        views
            .freshness
            .provisional_events(&replacement_ids, 0, views.cut.activity_cut)?
    };
    let replay_entries = if scope.is_some() {
        Vec::new()
    } else {
        views.freshness.replay_entries()?
    };
    let replacement = super::historical_manifest_plan::replacement_proof(
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

    let graph_proof = views.dream.graph_admission(
        views.archive,
        views.memories,
        &ids,
        &born_ids,
        scope.is_some(),
    );
    let (graph, adjacency) = graph_proof.result?;
    #[cfg(feature = "freshness-performance-trace")]
    traces.extend(graph_proof.traces);

    let snapshot = views.cut.community_snapshot.clone();
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
    let community = crate::history::community_proof::community_proof(
        scoped_snapshot.clone(),
        views.cut.memory_graph_version,
        &ids,
    )?;
    let memberships = scoped_snapshot
        .filter(|snapshot| snapshot.derived_graph_version == views.cut.memory_graph_version)
        .into_iter()
        .flat_map(|snapshot| snapshot.communities)
        .flat_map(|community| {
            community
                .members
                .into_iter()
                .map(move |id| (id, community.id))
        })
        .collect();
    let memory_facts = admission
        .memory_inputs
        .iter()
        .map(|(&id, input)| (id, (input.availability, input.credit_eligible)))
        .collect();
    let propagation = super::historical_manifest_plan::authorize_propagation(
        adjacency,
        memberships,
        &memory_facts,
    );

    let vector_proof = crate::history::vector_proof::read_vector_admission(
        views.archive,
        views.memories,
        views.vectors,
        &timeline.activity_order,
        &ids,
        scope.as_ref().map(|scope| &scope.source_nodes),
        scope.as_ref().map(|scope| scope.base_cut),
    )?;
    let embedding_pass = if requested_complete {
        super::historical_manifest_plan::embedding_pass(
            views.vectors.generation(),
            vector_proof.fragments_for_embedding(),
        )?
    } else {
        Value::text("not-requested")
    };
    let (vectors, sources) = vector_proof.finish(embedding_pass);
    let order = Value::Array(
        timeline
            .activity_order
            .iter()
            .map(|key| crate::history::source_proof::source_key(&key.conversation_id, &key.node_id))
            .collect(),
    );
    let historical_policy = crate::history::manifest_encoding::policy();
    let policy_digest = historical_policy.digest("reliquary:historical-policy:v1")?;
    if policy_digest != crate::freshness::historical_planner::HISTORICAL_POLICY_DIGEST {
        return Err(fail("unsupported historical policy digest"));
    }
    let base = views.freshness.policy();
    let mut base_bytes = Vec::new();
    base_bytes.extend(base.decay_turns_per_point.to_le_bytes());
    for value in [
        base.access_principal,
        base.linkage_principal,
        base.local_hop_cost,
        base.outside_hop_cost,
    ] {
        base_bytes.extend(value.to_le_bytes());
    }
    let mut base_hash = Sha256::new();
    base_hash.update(b"reliquary:freshness-policy:v1\0");
    base_hash.update(&base_bytes);
    let semantic = crate::freshness::semantic_policy::SemanticPolicy::default();

    views.freshness.synchronize_source()?;
    let input_bytes = views
        .history
        .path_length()
        .map_err(|error| fail(&error.to_string()))?;
    let prefix = views.history.file_digest()?;
    if views
        .history
        .path_length()
        .map_err(|error| fail(&error.to_string()))?
        != input_bytes
        || views.history.file_digest()? != prefix
    {
        return Err(fail("ImportInputDrift: discard and refreeze the manifest"));
    }
    let semantic_descriptor = semantic
        .canonical_descriptor()
        .map_err(|e| OwnerError::Freshness(e.to_string()));
    let semantic_digest = semantic
        .digest()
        .map_err(|e| OwnerError::Freshness(e.to_string()));
    let encoded = crate::history::manifest_proof::build(
        crate::history::manifest_proof::ManifestProofParts {
            owner,
            import_id,
            corpus_digest,
            global_version: views.cut.global_version,
            archive_version: views.cut.archive_version,
            activity_cut: views.cut.activity_cut,
            memory_version: views.cut.memory_version,
            graph_version: views.cut.graph_version,
            memory_graph_version: views.cut.memory_graph_version,
            vector_version: views.cut.vector_version,
            community_generation: snapshot.map(|snapshot| snapshot.generation),
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
    Ok(Manifest {
        propagation,
        input_digest: prefix,
        input_bytes,
        memory_inputs: admission.memory_inputs,
        timeline,
        digest: encoded.digest,
        corpus_digest,
        import_id,
        initial: admission.initial,
        superseded,
        epoch: scope.is_some(),
        cohort: replacement.cohort,
        base_cut: scope.as_ref().map_or(0, |scope| scope.base_cut),
        start_evidence_digest: scope.as_ref().map_or(prefix, |scope| scope.start_digest),
        #[cfg(test)]
        value: encoded.value,
    })
}

pub(crate) fn historical_import_id(owner: [u8; 16], corpus: [u8; 32]) -> [u8; 32] {
    super::historical_manifest_plan::historical_import_id(owner, corpus)
}
