//! First-cycle LIVE Dream over the same durable frontier and batch operations.
use crate::{
    DreamClassifier, DreamOrganizationDraft, DreamOwnerWriter, DreamVerificationPolicy,
    DreamVerifier, DreamWorkflowError, EntityOwnerSession, begin_prepared_dream_frontier,
    commit_dream_batch,
    dream_batch_workflow::{PreparedDreamBatch, map_dream_owner_write, prepare_dream_batch},
    dream_candidates::ranked_owner_dream_candidates_filtered,
    dream_empty_publication::prepare_initial_dream_settlement,
    prepare_explicit_dream_frontier,
};
use reliquary_owner_session::{
    DreamMemoryCut,
    model::{CompatibilityProfileId, MemoryId},
};
use reliquary_switchboard::GeneralEndpoint;
use sha2::{Digest, Sha256};

pub struct PreparedLiveDream {
    pass: String,
    publication: [u8; 32],
    batch: PreparedDreamBatch,
}

pub struct EvaluatedLiveDream {
    pass: String,
    publication: [u8; 32],
    draft: DreamOrganizationDraft,
}

impl PreparedLiveDream {
    pub fn evaluate<E: GeneralEndpoint>(
        self,
        endpoint: E,
    ) -> Result<EvaluatedLiveDream, DreamWorkflowError>
    where
        E: Clone,
    {
        let classifier = DreamClassifier::new(endpoint.clone());
        let verifier = DreamVerifier::new(endpoint);
        Ok(EvaluatedLiveDream {
            pass: self.pass,
            publication: self.publication,
            draft: self.batch.evaluate(
                &classifier,
                Some(&verifier),
                DreamVerificationPolicy::default(),
            )?,
        })
    }
}

fn identity(
    owner: [u8; 16],
    source: MemoryId,
    profile: CompatibilityProfileId,
    model: &str,
    route_policy: [u8; 32],
) -> (String, [u8; 32]) {
    let mut digest = Sha256::new();
    digest.update(b"reliquary-live-initial-dream-v1");
    digest.update(owner);
    digest.update(source.0);
    digest.update(profile.0);
    digest.update((model.len() as u64).to_le_bytes());
    digest.update(model.as_bytes());
    digest.update(route_policy);
    let digest = digest.finalize();
    (format!("live-dream:{digest:x}"), digest.into())
}

pub fn prepare_live_dream(
    writer: &mut DreamOwnerWriter<'_>,
    source: MemoryId,
    profile: CompatibilityProfileId,
    model: &str,
    route_policy: [u8; 32],
) -> Result<Option<PreparedLiveDream>, DreamWorkflowError> {
    let owner = writer.owner_id();
    let (pass, publication) = identity(owner, source, profile, model, route_policy);
    let (existing, frontier, published, view, turn) = match writer {
        DreamOwnerWriter::Rel(w) => {
            let read = w.read_session();
            (
                read.management_read()
                    .first_dream_pass(source)
                    .map(|(id, done)| (id.to_owned(), done)),
                read.initial_dream_frontier(&pass),
                w.dream_batch_published(publication),
                EntityOwnerSession::Rel(read),
                read.rel_turn_count(),
            )
        }
        DreamOwnerWriter::Phy(w) => (
            w.management_read()
                .map_err(map_dream_owner_write)?
                .first_dream_pass(source)
                .map(|(id, done)| (id.to_owned(), done)),
            w.initial_dream_frontier(&pass),
            w.dream_batch_published(publication),
            EntityOwnerSession::MountedPhy(w),
            1,
        ),
    };
    if let Some((id, settled)) = existing {
        if settled {
            return Ok(None);
        }
        if id != pass || frontier.is_none() {
            return Err(DreamWorkflowError::IncompleteFrontier);
        }
    }
    if published || frontier.as_ref().is_some_and(|f| f.candidates.is_empty()) {
        settle(writer, &pass)?;
        return Ok(None);
    }

    let ids = if let Some(frontier) = &frontier {
        if view.graph_version() != frontier.graph_version {
            return Err(DreamWorkflowError::StaleSnapshot);
        }
        for cut in std::iter::once(&frontier.source).chain(&frontier.candidates) {
            let revision = view
                .current_memory(cut.id)
                .map_err(|_| DreamWorkflowError::StaleSnapshot)?;
            if revision.record.revision != cut.revision || revision.record.body_id != cut.body_id {
                return Err(DreamWorkflowError::StaleSnapshot);
            }
        }
        frontier.candidates.iter().map(|cut| cut.id).collect()
    } else {
        ranked_owner_dream_candidates_filtered(
            &view,
            owner,
            source,
            profile,
            Default::default(),
            |id| {
                view.current_memory(id).is_ok_and(|current| {
                    matches!(
                        current.record.lifecycle_state.as_str(),
                        "knowledge" | "canonical"
                    )
                })
            },
        )?
        .0
    };

    let batch = if ids.is_empty() {
        None
    } else {
        Some(prepare_dream_batch(&view, owner, source, &ids)?)
    };
    if frontier.is_none() {
        let prepared = prepare_explicit_dream_frontier(&view, owner, source, &ids)?;
        begin_prepared_dream_frontier(writer, owner, &pass, &prepared, turn)?;
    }
    if let Some(batch) = batch {
        Ok(Some(PreparedLiveDream {
            pass,
            publication,
            batch,
        }))
    } else {
        settle(writer, &pass)?;
        Ok(None)
    }
}

pub fn commit_live_dream(
    writer: &mut DreamOwnerWriter<'_>,
    evaluated: &EvaluatedLiveDream,
    now_ns: i64,
) -> Result<(), DreamWorkflowError> {
    let owner = writer.owner_id();
    commit_dream_batch(
        writer,
        owner,
        evaluated.publication,
        &evaluated.draft,
        now_ns,
    )?;
    settle(writer, &evaluated.pass)
}

fn settle(writer: &mut DreamOwnerWriter<'_>, pass: &str) -> Result<(), DreamWorkflowError> {
    let (snapshot, graph, turn) = match writer {
        DreamOwnerWriter::Rel(w) => (
            w.initial_dream_snapshot(pass),
            w.dream_graph_version(),
            w.read_session().rel_turn_count(),
        ),
        DreamOwnerWriter::Phy(w) => (w.initial_dream_snapshot(pass), w.dream_graph_version(), 1),
    };
    let snapshot = snapshot.map_err(map_dream_owner_write)?;
    let cut = DreamMemoryCut {
        id: snapshot.source.id,
        revision: snapshot.source.revision,
        body_id: snapshot.source.body_id,
    };
    let (effects, promote) =
        prepare_initial_dream_settlement(snapshot, matches!(writer, DreamOwnerWriter::Rel(_)))?;
    match writer {
        DreamOwnerWriter::Rel(w) => {
            w.commit_initial_dream_pass(pass, cut, graph, turn, effects, promote)
        }
        DreamOwnerWriter::Phy(w) => {
            w.commit_initial_dream_pass(pass, cut, graph, turn, effects, promote)
        }
    }
    .map_err(map_dream_owner_write)?;
    Ok(())
}
