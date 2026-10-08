use super::super::observation::GenerationStatus;
use super::super::{CapabilityState, RuntimeCapability};
use super::*;
use crate::multiplexing_journal::{DurableHostOperationPhase, SubmissionState};

/// Body-free exact REL source clocks. Missing clocks remain unavailable.
impl WorkspaceManagement {
    /// Coherent P3/P4 source cut; worker and capability facts are current samples.
    /// Reading health never runs repair, starts a worker, or registers a recipient.
    pub fn health(&self, owner: &str) -> Result<RelManagementHealth, ManagementError> {
        let token = self
            .gateway
            .authorize(&self.instance, owner, OwnerGrant::ViewRel)
            .map_err(unavailable)?;
        let state = self
            .gateway
            .with_rel_distribution_admission(&token, |sequencer| {
                let _gate = sequencer.gate(token.mount_epoch)?;
                self.gateway.read_rel_snapshot_state(&token)
            })
            .map_err(unavailable)?;
        let (workers, background_enabled) = self
            .gateway
            .background_worker_state_for(&self.instance, owner)
            .map_err(unavailable)?;
        let capability = self
            .gateway
            .capability_state(RuntimeCapability::ConversationExecution)
            .map_err(unavailable)?;
        let held_native_generations = state.conversation_observation.as_ref().map_or(0, |o| {
            o.conversations
                .iter()
                .flat_map(|c| c.generations.iter())
                .filter(|g| g.status == GenerationStatus::Unknown)
                .count()
        });
        let cancellation_requests = state
            .host_work
            .iter()
            .filter(|h| h.cancel_requested)
            .count()
            + state.conversation_observation.as_ref().map_or(0, |o| {
                o.conversations
                    .iter()
                    .flat_map(|c| c.generations.iter())
                    .filter(|g| g.status == GenerationStatus::CancellationRequested)
                    .count()
            });
        let value = RelManagementHealth {
            mount_epoch: token.mount_epoch,
            versions: ManagementVersions {
                global: state.watermarks.global_version,
                archive: state.watermarks.archive_version,
                memory: state.watermarks.memory_version,
                entity: state.watermarks.entity_version,
                graph: state.watermarks.graph_version,
                vector: state.watermarks.vector_version,
                journal: state.watermarks.conversation_journal_sequence,
            },
            conversations: state.conversations.len(),
            branches: state.branches.len(),
            pending_submissions: state
                .submissions
                .iter()
                .filter(|s| s.state == SubmissionState::Pending)
                .count(),
            unknown_host_operations: state
                .host_work
                .iter()
                .filter(|h| h.phase == DurableHostOperationPhase::Unknown)
                .count(),
            cancellation_requests,
            held_native_generations,
            insomnia: state.processing,
            insomnia_enabled: state.processing_enabled,
            workers,
            background_enabled,
            external_execution_ready: matches!(capability, CapabilityState::Ready),
            external_work_held: matches!(capability, CapabilityState::UnknownStatusHeld),
        };
        self.deliver(&token, value)
    }
    pub fn statistics(&self, owner: &str) -> Result<SemanticOwnerStatistics, ManagementError> {
        let token = self.token(owner)?;
        let status = if owner.starts_with("phy-") {
            self.gateway
                .phy_background_status_for(&self.instance, owner)
        } else {
            self.gateway.background_status_for(&self.instance, owner)
        }
        .map_err(unavailable)?;
        let entities = self
            .gateway
            .entity_stats_for_owner(&self.instance, owner)
            .map_err(unavailable)?;
        let (workers, background_enabled) = self
            .gateway
            .background_worker_state_for(&self.instance, owner)
            .map_err(unavailable)?;
        let (memories, vectors, vector_pending, dream_pending, perception_pending) =
            if owner.starts_with("phy-") {
                (
                    status
                        .phylactery_memories
                        .ok_or(ManagementError::Unavailable)?,
                    status
                        .phylactery_vectors
                        .ok_or(ManagementError::Unavailable)?,
                    status.phylactery_vector_pending,
                    status.phylactery_dream_pending,
                    status.phylactery_perception_pending,
                )
            } else {
                (
                    status.reliquary_memories,
                    status.reliquary_vectors,
                    status.reliquary_vector_pending,
                    status.reliquary_dream_pending,
                    status.reliquary_perception_pending,
                )
            };
        self.deliver(
            &token,
            SemanticOwnerStatistics {
                memories,
                vectors,
                entities,
                vector_pending,
                dream_pending,
                perception_pending,
                workers,
                background_enabled,
            },
        )
    }
    pub fn quiesce(&self, owner: &str) -> Result<(), ManagementError> {
        self.gateway
            .quiesce_rel_for(&self.instance, owner)
            .map_err(unavailable)
    }
    /// Explicit verification/repair uses the existing closed-and-drained owner path.
    pub fn reconcile_quiesced(&self, owner: &str) -> Result<(), ManagementError> {
        self.gateway
            .reconcile_quiesced_rel_for(&self.instance, owner)
            .map_err(unavailable)
    }
    pub fn reopen(&self, owner: &str) -> Result<u64, ManagementError> {
        self.gateway
            .reopen_rel_for(&self.instance, owner)
            .map_err(unavailable)
    }
}
