//! The regular prepared Dream policy, pinned to one accepted import Memory.
use crate::{
    DreamClassifier, DreamOrganizationDraft, DreamVerificationPolicy, DreamVerifier,
    DreamWorkflowError, EntityOwnerSession, PreparedDreamFrontier,
    dream_batch_workflow::{
        PreparedDreamBatch, map_dream_owner_write, prepare_dream_batch, prepare_dream_batch_cut,
    },
    dream_candidates::ranked_owner_dream_candidates_filtered,
    dream_empty_publication::prepare_initial_dream_settlement,
    prepare_explicit_dream_frontier,
};
use reliquary_dream::DreamCandidateConfig;
use reliquary_owner_session::{
    DreamMemoryCut, RelWriter,
    model::{CompatibilityProfileId, MemoryId},
};
use reliquary_switchboard::GeneralEndpoint;

pub enum NativeImportDreamCandidates<'a> {
    Explicit(&'a [MemoryId]),
    Ranked {
        profile: CompatibilityProfileId,
        config: DreamCandidateConfig,
    },
}

/// Owner-bound input that can be evaluated after releasing the writer lock.
pub struct PreparedNativeImportDream {
    owner: [u8; 16],
    import_id: String,
    source: MemoryId,
    batch: PreparedDreamBatch,
}

impl PreparedNativeImportDream {
    pub fn evaluate<C: GeneralEndpoint, V: GeneralEndpoint>(
        self,
        classifier: &DreamClassifier<C>,
        verifier: Option<&DreamVerifier<V>>,
        policy: DreamVerificationPolicy,
    ) -> Result<EvaluatedNativeImportDream, DreamWorkflowError> {
        Ok(EvaluatedNativeImportDream {
            owner: self.owner,
            import_id: self.import_id,
            source: self.source,
            draft: self.batch.evaluate(classifier, verifier, policy)?,
        })
    }
}

pub struct EvaluatedNativeImportDream {
    owner: [u8; 16],
    import_id: String,
    source: MemoryId,
    draft: DreamOrganizationDraft,
}

pub(super) fn dream_pass(import_id: &str, source: MemoryId) -> (String, [u8; 32]) {
    RelWriter::native_import_dream_identity(import_id, source)
}

/// Pin the selected frontier before model calls. On reopen the durable
/// frontier determines the candidates. An already-published batch resumes
/// directly at settlement. None means the source's first cycle is settled.
pub fn prepare_native_import_dream(
    writer: &mut RelWriter,
    import_id: &str,
    source: MemoryId,
    candidates: NativeImportDreamCandidates<'_>,
) -> Result<Option<PreparedNativeImportDream>, DreamWorkflowError> {
    writer
        .validate_native_import_memory(import_id, source)
        .map_err(map_dream_owner_write)?;
    let (pass, publication) = dream_pass(import_id, source);
    if writer.initial_dream_settled(&pass) {
        return Ok(None);
    }
    if writer.dream_batch_published(publication) {
        settle_native_import_initial_dream(writer, import_id, source)?;
        return Ok(None);
    }
    let owner = writer.owner_id();
    let view = EntityOwnerSession::Rel(writer.read_session());
    let (ids, prepared): (Vec<MemoryId>, Option<PreparedDreamFrontier>) =
        if let Some(frontier) = writer.initial_dream_frontier(&pass) {
            if writer.dream_graph_version() != frontier.graph_version {
                return Err(DreamWorkflowError::StaleSnapshot);
            }
            (
                frontier
                    .candidates
                    .iter()
                    .map(|candidate| candidate.id)
                    .collect(),
                None,
            )
        } else {
            let ids = match candidates {
                NativeImportDreamCandidates::Explicit(ids) => ids.to_vec(),
                NativeImportDreamCandidates::Ranked { profile, config } => {
                    ranked_owner_dream_candidates_filtered(
                        &view,
                        owner,
                        source,
                        profile,
                        config,
                        |id| writer.native_import_dream_candidate_ready(import_id, id),
                    )?
                    .0
                }
            };
            let prepared = prepare_explicit_dream_frontier(&view, owner, source, &ids)?;
            (ids, Some(prepared))
        };
    let batch = prepare_dream_batch(&view, owner, source, &ids)?;
    if let Some(prepared) = prepared {
        let turn = writer.read_session().rel_turn_count();
        prepared.begin_native_import(writer, import_id, pass, turn)?;
    }
    Ok(Some(PreparedNativeImportDream {
        owner,
        import_id: import_id.into(),
        source,
        batch,
    }))
}

pub fn commit_native_import_dream(
    writer: &mut RelWriter,
    evaluated: &EvaluatedNativeImportDream,
    updated_at_ns: i64,
) -> Result<bool, DreamWorkflowError> {
    if writer.owner_id() != evaluated.owner {
        return Err(DreamWorkflowError::OwnerMismatch);
    }
    let import_id = &evaluated.import_id;
    let source = evaluated.source;
    writer
        .validate_native_import_memory(import_id, source)
        .map_err(map_dream_owner_write)?;
    let (pass, publication) = dream_pass(import_id, source);
    if writer.initial_dream_settled(&pass) {
        return Ok(false);
    }
    if !writer.dream_batch_published(publication) {
        let draft = &evaluated.draft;
        let frontier = writer
            .initial_dream_frontier(&pass)
            .ok_or(DreamWorkflowError::StaleSnapshot)?;
        if !frontier.candidates.is_empty()
            || !draft.plan.relation_changes.is_empty()
            || !draft.plan.lifecycle_intents.is_empty()
        {
            let cut = prepare_dream_batch_cut(evaluated.owner, publication, draft, updated_at_ns)?;
            writer
                .commit_native_import_dream_batch(import_id, &pass, cut)
                .map_err(map_dream_owner_write)?;
        } else if writer.dream_graph_version() != draft.graph_version {
            return Err(DreamWorkflowError::StaleSnapshot);
        }
    }
    settle_native_import_initial_dream(writer, import_id, source)
}

/// Settle the owner-pinned, fully evaluated frontier and its Freshness effects.
pub fn settle_native_import_initial_dream(
    writer: &mut RelWriter,
    import_id: &str,
    source: MemoryId,
) -> Result<bool, DreamWorkflowError> {
    writer
        .validate_native_import_memory(import_id, source)
        .map_err(map_dream_owner_write)?;
    let (pass, _) = dream_pass(import_id, source);
    if writer.initial_dream_settled(&pass) {
        return Ok(false);
    }
    if !writer.initial_dream_frontier_evaluated(&pass) {
        return Err(DreamWorkflowError::IncompleteFrontier);
    }
    let snapshot = writer
        .initial_dream_snapshot(&pass)
        .map_err(map_dream_owner_write)?;
    if snapshot.source.id != source {
        return Err(DreamWorkflowError::StaleSnapshot);
    }
    let observed = DreamMemoryCut {
        id: source,
        revision: snapshot.source.revision,
        body_id: snapshot.source.body_id,
    };
    let (effects, promote) = prepare_initial_dream_settlement(snapshot, true)?;
    writer
        .commit_native_import_initial_dream_pass(
            import_id,
            &pass,
            observed,
            writer.dream_graph_version(),
            writer.read_session().rel_turn_count(),
            effects,
            promote,
        )
        .map_err(map_dream_owner_write)
}
