//! Mounted LIVE Entity work admission and publication.
use super::*;
use reliquary_application::{
    EntityOwnerSession,
    live_processing::{LiveEntityInput, accepted_live_mentions},
    prepare_memory_mention, prepare_phy_writer_mention,
};
use reliquary_owner_session::{LiveExtractionOutput, ManagementRead, RelSession, RelWriter};
use std::collections::BTreeSet;

impl Instance {
    pub(super) fn live_entity_work(
        &self,
        owner: OwnerId,
        phy: Option<OwnerId>,
        limit: usize,
    ) -> Result<(LiveStageProgress, Vec<LiveEntityInput>), GatewayError> {
        let collect = |rel: &RelSession, writer: Option<&RelWriter>| {
            if let Some(phy) = phy {
                self.with_management_read(phy, true, |read| {
                    collect(rel, writer, Some(read), limit)
                })?
                .map_err(Into::into)
            } else {
                collect(rel, writer, None, limit).map_err(Into::into)
            }
        };
        match self.reliquary(owner) {
            Ok(read) => collect(&read, None),
            Err(GatewayError::WriterUnavailable) => {
                self.with_rel_writer(owner, |writer| collect(writer.read_session(), Some(writer)))?
            }
            Err(error) => Err(error),
        }
    }

    pub(super) fn process_live_mention(
        &self,
        source: OwnerId,
        input: LiveEntityInput,
        endpoint: SharedEndpoint,
        stop: &AtomicBool,
    ) -> Result<bool, GatewayError> {
        let owner = OwnerId::from_bytes(input.mention.owner_id);
        let require_exact = |read: ManagementRead<'_>| match input.resolved(&read) {
            Some(false) => Ok(true),
            Some(true) => Ok(false),
            None => Err(GatewayError::HostStatusUnknown),
        };
        let prepared = if owner == source {
            self.with_writable_reliquary(owner, |rel| {
                require_exact(rel.management_read())?
                    .then(|| {
                        prepare_memory_mention(EntityOwnerSession::Rel(rel), input.mention.clone())
                            .map_err(stage_error)
                    })
                    .transpose()
            })??
        } else {
            self.authorize_routed_owners(source, owner)?;
            self.with_entity_phy_writer(owner, |writer| {
                require_exact(writer.management_read()?)?
                    .then(|| {
                        prepare_phy_writer_mention(writer, input.mention.clone())
                            .map_err(stage_error)
                    })
                    .transpose()
            })??
        };
        let Some(prepared) = prepared else {
            return Ok(true);
        };
        self.validate_live_processing(source)?;
        if stop.load(Ordering::Acquire) {
            return Ok(false);
        }
        let proposal = prepared.evaluate_with_endpoint(endpoint).map_err(|_| {
            GatewayError::RetrievalUnavailable(
                "configured LIVE Entity provider failed or returned invalid output",
            )
        })?;
        if stop.load(Ordering::Acquire) {
            return Ok(false);
        }
        if owner == source {
            self.publish_entity_mention(proposal, now_ns())?;
        } else {
            self.authorize_routed_owners(source, owner)?;
            self.with_live_source(source, || self.publish_entity_mention(proposal, now_ns()))?;
        }
        Ok(true)
    }
}

fn collect(
    rel: &RelSession,
    writer: Option<&RelWriter>,
    phy: Option<ManagementRead<'_>>,
    limit: usize,
) -> Result<(LiveStageProgress, Vec<LiveEntityInput>), OwnerSessionError> {
    let read = rel.management_read();
    let destination = phy.as_ref().map(|phy| phy.owner_id);
    let mut expected = BTreeSet::new();
    rel.visit_live_outputs(destination, |output| {
        expected.insert(output.episode);
    });
    let has_outputs = !expected.is_empty();
    let mut progress = LiveStageProgress::new(ImportStage::Entity);
    let mut work = Vec::new();
    let mut outcome = Ok(());
    let mut visit = |output: LiveExtractionOutput<'_>, payload: &[u8]| {
        if outcome.is_err() || !expected.remove(&output.episode) {
            return;
        }
        let result = accepted_live_mentions(
            read.owner_id,
            output.episode,
            output.model,
            &output.local,
            destination.map(|id| (id, output.routed.as_slice())),
            payload,
        );
        match result {
            Ok(mentions) => {
                let mut seen = BTreeSet::new();
                for input in mentions {
                    let id = input.work_id();
                    if !seen.insert(id.clone()) {
                        continue;
                    }
                    let owner = if input.mention.owner_id == read.owner_id {
                        &read
                    } else {
                        phy.as_ref().expect("authorized destination output")
                    };
                    match input.resolved(owner) {
                        Some(true) => progress.completed += 1,
                        Some(false) => {
                            progress.pending_work(id);
                            if work.len() < limit {
                                work.push(input);
                            }
                        }
                        None => progress.unsupported += 1,
                    }
                }
            }
            Err(error) => outcome = Err(error),
        }
    };
    if has_outputs {
        if let Some(writer) = writer {
            writer.visit_live_checkpoints(destination, &mut visit)?;
        } else {
            rel.visit_live_checkpoints(destination, &mut visit)?;
        }
    }
    outcome?;
    progress.unsupported += expected.len();
    progress.finish_status();
    Ok((progress, work))
}
