//! Authenticated LIVE Dream and its source vectors.
use super::*;
use reliquary_application::{
    DreamOwnerWriter, DreamWorkflowError, SwitchboardEmbeddingEndpoint,
    live_processing::{LiveDreamState, commit_live_dream, live_dream_state, prepare_live_dream},
};
use reliquary_owner_session::{
    ManagementRead,
    model::{CompatibilityProfileId, MemoryId},
};
use std::collections::BTreeSet;

pub(super) type DreamWork = (LiveDreamState, OwnerId, MemoryId);

impl Instance {
    pub(super) fn live_dream_work(
        &self,
        owner: OwnerId,
        phy: Option<OwnerId>,
        limit: usize,
    ) -> Result<(LiveStageProgress, Vec<DreamWork>), GatewayError> {
        self.with_reliquary(owner, |rel| {
            let read = rel.management_read();
            let collect = |phy: Option<ManagementRead<'_>>| {
                let mut progress = LiveStageProgress::new(ImportStage::Dream);
                let mut work = BTreeSet::new();
                rel.visit_live_outputs(phy.as_ref().map(|view| view.owner_id), |output| {
                    for (view, memories) in std::iter::once((&read, output.local))
                        .chain(phy.as_ref().map(|view| (view, output.routed)))
                    {
                        let owner = OwnerId::from_bytes(view.owner_id);
                        for memory in memories {
                            match live_dream_state(view, memory) {
                                LiveDreamState::Complete => progress.completed += 1,
                                LiveDreamState::Unavailable => progress.unsupported += 1,
                                state => {
                                    progress.pending_work(format!(
                                        "{}:{}",
                                        crate::hex(&owner.as_bytes()),
                                        crate::hex(&memory.0)
                                    ));
                                    work.insert((state, owner, memory));
                                    if work.len() > limit {
                                        work.pop_last();
                                    }
                                }
                            }
                        }
                    }
                });
                progress.finish_status();
                (progress, work.into_iter().collect())
            };
            if let Some(phy) = phy {
                self.with_management_read(phy, true, |read| collect(Some(read)))
            } else {
                Ok(collect(None))
            }
        })?
    }

    pub(super) fn live_dream_profile(
        &self,
        owner: OwnerId,
        phy: Option<OwnerId>,
        endpoint: &SwitchboardEmbeddingEndpoint,
    ) -> Result<CompatibilityProfileId, GatewayError> {
        let profile = self.publish_embedding_profile(owner, endpoint)?;
        if let Some(phy) = phy {
            self.authorize_routed_owners(owner, phy)?;
            let accepted = self
                .with_writable_reliquary(owner, |rel| {
                    rel.active_compatibility_profile(profile).cloned()
                })?
                .ok_or(GatewayError::RetrievalUnavailable(
                    "LIVE embedding profile changed",
                ))?;
            self.with_live_source(owner, || {
                Ok(self.with_entity_phy_writer(phy, |writer| {
                    writer.publish_compatibility_profile(accepted)
                })??)
            })?;
        }
        Ok(profile)
    }

    pub(super) fn process_live_dream(
        &self,
        source: OwnerId,
        owner: OwnerId,
        memory: MemoryId,
        profile: CompatibilityProfileId,
        route_policy: [u8; 32],
        endpoint: SharedEndpoint,
        stop: &AtomicBool,
    ) -> Result<bool, GatewayError> {
        let prepare = |writer: &mut DreamOwnerWriter<'_>| {
            prepare_live_dream(writer, memory, profile, endpoint.model(), route_policy)
        };
        self.validate_live_processing(source)?;
        let prepared = if owner == source {
            self.with_rel_writer(owner, |writer| prepare(&mut DreamOwnerWriter::Rel(writer)))?
        } else {
            self.authorize_routed_owners(source, owner)?;
            self.with_live_source(source, || {
                self.with_entity_phy_writer(owner, |writer| {
                    prepare(&mut DreamOwnerWriter::Phy(writer))
                })
            })?
        }
        .map_err(dream_error)?;
        let Some(prepared) = prepared else {
            return Ok(true);
        };
        self.validate_live_processing(source)?;
        if stop.load(Ordering::Acquire) {
            return Ok(false);
        }
        let evaluated = prepared.evaluate(endpoint).map_err(dream_error)?;
        if stop.load(Ordering::Acquire) {
            return Ok(false);
        }
        let commit =
            |writer: &mut DreamOwnerWriter<'_>| commit_live_dream(writer, &evaluated, now_ns());
        if owner == source {
            self.with_rel_writer(owner, |writer| commit(&mut DreamOwnerWriter::Rel(writer)))?
        } else {
            self.authorize_routed_owners(source, owner)?;
            self.with_live_source(source, || {
                self.with_entity_phy_writer(owner, |writer| {
                    commit(&mut DreamOwnerWriter::Phy(writer))
                })
            })?
        }
        .map_err(dream_error)?;
        Ok(true)
    }
}

fn dream_error(error: DreamWorkflowError) -> GatewayError {
    match error {
        DreamWorkflowError::IncompleteFrontier | DreamWorkflowError::StaleSnapshot => {
            GatewayError::HostStatusUnknown
        }
        DreamWorkflowError::Classifier(_) | DreamWorkflowError::Verifier(_) => {
            GatewayError::RetrievalUnavailable(
                "configured LIVE Dream provider failed or returned invalid output",
            )
        }
        other => stage_error(other),
    }
}
