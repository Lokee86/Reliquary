//! Run Application-declared LIVE stages on the normal processing lease.
mod dream;
mod entity;
use crate::{
    GatewayError, Instance, OwnerId, OwnerSessionError, ProcessingState,
    insomnia_worker::{SharedEndpoint, now_ns},
    processing_stages::ImportProcessingRoutes,
};
use reliquary_application::{
    SwitchboardEmbeddingEndpoint,
    live_processing::{LiveDreamState, LiveProcessingSelection, LiveStageProgress},
    processing_plan::{ImportStage, StageState},
};
use reliquary_switchboard::GeneralEndpoint;
use std::sync::atomic::{AtomicBool, Ordering};

impl Instance {
    pub(crate) fn validate_live_processing(&self, owner: OwnerId) -> Result<(), GatewayError> {
        Ok(self.with_rel_writer(owner, |writer| writer.validate_live_processing())??)
    }

    fn with_live_source<T>(
        &self,
        source: OwnerId,
        operation: impl FnOnce() -> Result<T, GatewayError>,
    ) -> Result<T, GatewayError> {
        self.with_rel_writer(source, |writer| {
            writer.validate_live_processing()?;
            operation()
        })?
    }

    pub fn live_stage_status(
        &self,
        owner: OwnerId,
        phy: Option<OwnerId>,
        stage: ImportStage,
    ) -> Result<LiveStageProgress, GatewayError> {
        match stage {
            ImportStage::Entity => Ok(self.live_entity_work(owner, phy, 0)?.0),
            ImportStage::Dream => Ok(self.live_dream_work(owner, phy, 0)?.0),
            _ => Err(GatewayError::RetrievalUnavailable(
                "unsupported LIVE stage status",
            )),
        }
    }

    pub fn live_routed_memory_count(
        &self,
        owner: OwnerId,
        phy: OwnerId,
    ) -> Result<usize, GatewayError> {
        self.authorize_routed_owners(owner, phy)?;
        let count = self.with_reliquary(owner, |rel| {
            let mut count = 0;
            rel.visit_live_outputs(Some(phy.as_bytes()), |output| count += output.routed.len());
            count
        })?;
        self.authorize_routed_owners(owner, phy)?;
        Ok(count)
    }
}

pub(crate) fn run_live_stages(
    instance: &Instance,
    owner: OwnerId,
    phy: Option<OwnerId>,
    routes: ImportProcessingRoutes,
    selection: LiveProcessingSelection,
    limit: usize,
    route_policy: [u8; 32],
    stop: &AtomicBool,
    mut report: impl FnMut(LiveStageProgress),
) -> Result<ProcessingState, GatewayError> {
    let stopped = || stop.load(Ordering::Acquire);
    let mut dream_work = Vec::new();
    let mut dream_status = LiveStageProgress::new(ImportStage::Dream);
    let mut profile = None;
    let mut unsupported = false;
    for &stage in selection.stages() {
        if stopped() {
            return Ok(ProcessingState::Stopped);
        }
        let mut progress = LiveStageProgress::new(stage);
        progress.state = StageState::Executed;
        report(progress.clone());
        let result = (|| -> Result<(), GatewayError> {
            match stage {
                ImportStage::Entity => {
                    let (status, work) = instance.live_entity_work(owner, phy, limit)?;
                    progress = status;
                    if !work.is_empty() {
                        let Some(endpoint) = routes.entity.as_ref().cloned().map(SharedEndpoint)
                        else {
                            progress.state = StageState::NotConfigured;
                            return Ok(());
                        };
                        for input in work {
                            if stopped() {
                                break;
                            }
                            let id = input.work_id();
                            if instance.process_live_mention(
                                owner,
                                input,
                                endpoint.clone(),
                                stop,
                            )? {
                                progress.pending -= 1;
                                progress.completed += 1;
                                progress.work_ids.retain(|work| work != &id);
                            }
                        }
                        progress.finish_status();
                    }
                }
                ImportStage::Vectors => {
                    (dream_status, dream_work) = instance.live_dream_work(owner, phy, limit)?;
                    if !dream_work.is_empty() {
                        progress.pending = dream_work
                            .iter()
                            .filter(|(state, _, _)| *state == LiveDreamState::Pending)
                            .count();
                        let Some(endpoint) = routes
                            .embedding
                            .as_ref()
                            .cloned()
                            .map(SwitchboardEmbeddingEndpoint::new)
                        else {
                            progress.state = StageState::NotConfigured;
                            return Ok(());
                        };
                        let routed =
                            phy.filter(|phy| dream_work.iter().any(|(_, owner, _)| owner == phy));
                        if stopped() {
                            return Ok(());
                        }
                        instance.validate_live_processing(owner)?;
                        let id = instance.live_dream_profile(owner, routed, &endpoint)?;
                        profile = Some(id);
                        for &(state, target, memory) in &dream_work {
                            if stopped() {
                                break;
                            }
                            if state == LiveDreamState::Started {
                                continue;
                            }
                            instance.validate_live_processing(owner)?;
                            let vectors = instance.prepare_owner_memory_vectors(
                                target,
                                id,
                                &[memory],
                                &endpoint,
                            )?;
                            if stopped() {
                                break;
                            }
                            if target == owner {
                                instance.publish_owner_memory_vectors(&vectors)?;
                            } else {
                                instance.authorize_routed_owners(owner, target)?;
                                instance.with_live_source(owner, || {
                                    instance.publish_owner_memory_vectors(&vectors)
                                })?;
                            }
                            progress.pending -= 1;
                            progress.completed += 1;
                        }
                    }
                    progress.finish_status();
                }
                ImportStage::Dream => {
                    progress = dream_status.clone();
                    if let Some(profile) = profile {
                        let Some(endpoint) = routes.dream.as_ref().cloned().map(SharedEndpoint)
                        else {
                            progress.state = StageState::NotConfigured;
                            return Ok(());
                        };
                        for &(_, target, memory) in &dream_work {
                            if stopped() {
                                break;
                            }
                            if instance.process_live_dream(
                                owner,
                                target,
                                memory,
                                profile,
                                route_policy,
                                endpoint.clone(),
                                stop,
                            )? {
                                progress.pending -= 1;
                                progress.completed += 1;
                                let id = format!(
                                    "{}:{}",
                                    crate::hex(&target.as_bytes()),
                                    crate::hex(&memory.0)
                                );
                                progress.work_ids.retain(|work| work != &id);
                            }
                        }
                        progress.finish_status();
                    }
                }
                ImportStage::Relationship => {
                    // Typed relationship proposals have no automatic LIVE source.
                    progress.state = StageState::Unsupported;
                }
                ImportStage::HistoricalFreshness => unreachable!("import-only stage"),
            }
            Ok(())
        })();
        match result {
            Err(GatewayError::HostStatusUnknown) => {
                progress.state = StageState::Held;
                progress.pending = progress.pending.saturating_sub(1);
                progress.held += 1;
                report(progress);
                return Ok(ProcessingState::Held);
            }
            Err(error) => {
                progress.state = StageState::Failed;
                progress.pending = progress.pending.saturating_sub(1);
                progress.failed += 1;
                report(progress);
                return Err(error);
            }
            Ok(()) => {}
        }
        let state = progress.state;
        unsupported |= progress.unsupported > 0;
        report(progress);
        if stopped() {
            return Ok(ProcessingState::Stopped);
        }
        if matches!(state, StageState::Pending | StageState::NotConfigured) {
            return Ok(ProcessingState::Incomplete);
        }
    }
    if unsupported {
        return Ok(ProcessingState::Incomplete);
    }
    if selection == LiveProcessingSelection::All
        && dream_status.completed == 0
        && dream_work.is_empty()
    {
        Ok(ProcessingState::Idle)
    } else {
        Ok(ProcessingState::Ready)
    }
}

fn stage_error(error: impl std::fmt::Display) -> GatewayError {
    OwnerSessionError::Record(error.to_string()).into()
}
