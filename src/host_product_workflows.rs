//! Narrow authenticated product operations over existing named workflows.

use super::{Gateway, Instance, OwnerGrant, OwnerId};
use crate::{
    CapabilityRouter, CompatibilityProfileId, CreateRelationshipRequest, EntityRetirementRequest,
    EntityTombstone, GeneralEndpoint, ImportFreshnessSettlement, MemoryEntityResolution, MemoryId,
    MemoryRetirementRequest, MemoryRetrievalConfig, MemoryRetrievalResult, MemoryRetrievalRoute,
    NativeImportBatch, OwnerMemorySelection, ProductError, Relationship,
    RelationshipRetirementRequest, RelationshipTombstone, RetrievalAction, RetrievalAnchorReport,
    RetrievalConfig, StagedNativeImport, TerminalTombstone, WithdrawEntityMentionRequest,
};
use reliquary_runtime_gateway as runtime;
use std::{collections::BTreeSet, sync::Arc, time::Duration};

/// Authenticated external turn scoped to one owner and principal.
pub struct MemoryRetrievalTurn {
    inner: runtime::MemoryRetrievalTurn,
    owner: OwnerId,
}
impl MemoryRetrievalTurn {
    pub fn owner_id(&self) -> OwnerId {
        self.owner
    }

    pub fn turn_id(&self) -> &str {
        self.inner.turn_id()
    }
}

/// Opaque prepared host manifest. Its content remains inside the trusted adapter path.
pub struct PreparedMemoryDelivery {
    inner: runtime::PreparedMemoryDelivery,
    owner: OwnerId,
}
impl PreparedMemoryDelivery {
    pub fn owner_id(&self) -> OwnerId {
        self.owner
    }

    pub fn stable_id(&self) -> &str {
        self.inner.stable_id()
    }
}

pub struct RetrievalOutcome {
    pub prepared: Option<PreparedMemoryDelivery>,
    pub already_provided: BTreeSet<MemoryId>,
    pub anchors: Vec<RetrievalAnchorReport>,
}

impl Instance {
    /// Stage an idempotent, source-bound copied snapshot through native import.
    /// This never fabricates live Gateway turns or converts historical files.
    pub fn stage_native_import(
        &self,
        owner: OwnerId,
        batch: NativeImportBatch,
    ) -> Result<StagedNativeImport, ProductError> {
        self.require(OwnerGrant::OperateRel(owner))?;
        self.inner
            .stage_native_import(owner.into(), batch)
            .map_err(Into::into)
    }

    /// Settle the accepted copied import after staging.
    pub fn settle_native_import(
        &self,
        owner: OwnerId,
        import_id: &str,
    ) -> Result<ImportFreshnessSettlement, ProductError> {
        self.require(OwnerGrant::OperateRel(owner))?;
        self.inner
            .settle_native_import(owner.into(), import_id)
            .map_err(Into::into)
    }

    pub fn rank_rel_memories(
        &self,
        owner: OwnerId,
        query: &str,
        profile: Option<CompatibilityProfileId>,
        vector: Option<&[f32]>,
        config: RetrievalConfig,
    ) -> Result<OwnerMemorySelection, ProductError> {
        self.require_rel_read(owner)?;
        self.inner
            .rank_rel_memories(owner.into(), query, profile, vector, config)
            .map_err(Into::into)
    }

    pub fn route_rel_memories(
        &self,
        owner: OwnerId,
        profile: CompatibilityProfileId,
        vector: &[f32],
        config: MemoryRetrievalConfig,
    ) -> Result<MemoryRetrievalResult, ProductError> {
        self.require_rel_read(owner)?;
        self.inner
            .route_rel_memories(owner.into(), profile, vector, config)
            .map_err(Into::into)
    }

    pub fn begin_memory_retrieval_turn(
        &self,
        owner: OwnerId,
        turn_id: &str,
    ) -> Result<MemoryRetrievalTurn, ProductError> {
        self.require_rel_read(owner)?;
        let inner = self
            .inner
            .begin_memory_retrieval_turn(owner.into(), turn_id)
            .map_err(ProductError::from)?;
        Ok(MemoryRetrievalTurn { inner, owner })
    }

    pub fn prepare_agentic_memory_delivery(
        &self,
        turn: &MemoryRetrievalTurn,
        route: &MemoryRetrievalRoute,
        action: RetrievalAction,
        profile: Option<CompatibilityProfileId>,
        query_vector: Option<&[f32]>,
    ) -> Result<RetrievalOutcome, ProductError> {
        self.require_rel_read(turn.owner_id())?;
        let outcome = self
            .inner
            .prepare_agentic_memory_delivery(&turn.inner, route, action, profile, query_vector)
            .map_err(ProductError::from)?;
        Ok(RetrievalOutcome {
            prepared: outcome.prepared.map(|inner| PreparedMemoryDelivery {
                inner,
                owner: turn.owner,
            }),
            already_provided: outcome
                .already_provided
                .into_iter()
                .map(|id| MemoryId(id.0))
                .collect(),
            anchors: outcome.anchors,
        })
    }

    /// Deliver only the opaque prepared manifest through the connected,
    /// contract-checked host adapter. Reads and previews never confirm use.
    pub fn deliver_agentic_memory(
        &self,
        turn: &MemoryRetrievalTurn,
        prepared: &PreparedMemoryDelivery,
        router: &CapabilityRouter,
    ) -> Result<bool, ProductError> {
        self.require(OwnerGrant::OperateRel(prepared.owner_id()))?;
        self.inner
            .deliver_agentic_memory(&turn.inner, &prepared.inner, router)
            .map_err(Into::into)
    }

    /// Recover only an already host-confirmed exact manifest.
    pub fn recover_agentic_memory_delivery(
        &self,
        turn: &MemoryRetrievalTurn,
        stable_id: &str,
        router: &CapabilityRouter,
    ) -> Result<bool, ProductError> {
        self.require(OwnerGrant::OperateRel(turn.owner_id()))?;
        self.inner
            .recover_agentic_memory_delivery(&turn.inner, stable_id, router)
            .map_err(Into::into)
    }

    pub fn withdraw_semantic_entity_mention(
        &self,
        owner: OwnerId,
        request: WithdrawEntityMentionRequest,
    ) -> Result<MemoryEntityResolution, ProductError> {
        self.require_owner_operation(owner)?;
        self.inner
            .withdraw_semantic_entity_mention(owner.into(), request)
            .map_err(Into::into)
    }

    pub fn retire_semantic_memory(
        &self,
        owner: OwnerId,
        request: MemoryRetirementRequest,
    ) -> Result<TerminalTombstone, ProductError> {
        self.require_owner_operation(owner)?;
        self.inner
            .retire_semantic_memory(owner.into(), request)
            .map_err(Into::into)
    }

    pub fn retire_semantic_entity(
        &self,
        owner: OwnerId,
        request: EntityRetirementRequest,
    ) -> Result<EntityTombstone, ProductError> {
        self.require_owner_operation(owner)?;
        self.inner
            .retire_semantic_entity(owner.into(), request)
            .map_err(Into::into)
    }

    pub fn publish_semantic_relationship(
        &self,
        owner: OwnerId,
        request: CreateRelationshipRequest,
    ) -> Result<Relationship, ProductError> {
        self.require_owner_operation(owner)?;
        self.inner
            .publish_semantic_relationship(owner.into(), request)
            .map_err(Into::into)
    }

    pub fn retire_semantic_relationship(
        &self,
        owner: OwnerId,
        request: RelationshipRetirementRequest,
    ) -> Result<RelationshipTombstone, ProductError> {
        self.require_owner_operation(owner)?;
        self.inner
            .retire_semantic_relationship(owner.into(), request)
            .map_err(Into::into)
    }

    fn require_owner_operation(&self, owner: OwnerId) -> Result<(), ProductError> {
        if self.grants.contains(&OwnerGrant::OperateRel(owner)) {
            self.require(OwnerGrant::OperateRel(owner))
        } else {
            self.require(OwnerGrant::OperatePhylactery(owner))
        }
    }
}

/// Root-native worker settings use opaque public owner IDs rather than runtime handles.
#[derive(Clone, Debug)]
pub struct InsomniaWorkerConfig {
    pub workers: usize,
    pub destination_phy_owner: Option<OwnerId>,
    pub max_admissions_per_claim: usize,
    pub lease_duration_ns: i64,
    pub idle_poll: Duration,
}
impl Default for InsomniaWorkerConfig {
    fn default() -> Self {
        Self {
            workers: 1,
            destination_phy_owner: None,
            max_admissions_per_claim: 16,
            lease_duration_ns: 120_000_000_000,
            idle_poll: Duration::from_millis(250),
        }
    }
}

pub struct InsomniaWorkerHandle {
    inner: runtime::InsomniaWorkerHandle,
}
impl InsomniaWorkerHandle {
    pub fn state(&self) -> runtime::InsomniaWorkerState {
        self.inner.state()
    }
    pub fn is_stopped(&self) -> bool {
        self.inner.is_stopped()
    }
    pub fn stop(&mut self) -> Result<(), ProductError> {
        self.inner.stop().map_err(Into::into)
    }
}

impl Gateway {
    pub fn start_insomnia_workers(
        &self,
        instance: &Instance,
        owner: OwnerId,
        endpoint: Arc<dyn GeneralEndpoint>,
        config: InsomniaWorkerConfig,
    ) -> Result<InsomniaWorkerHandle, ProductError> {
        instance.require(OwnerGrant::OperateRel(owner))?;
        if let Some(destination) = config.destination_phy_owner {
            instance.require(OwnerGrant::OperatePhylactery(destination))?;
        }
        let runtime_config = runtime::InsomniaWorkerConfig {
            workers: config.workers,
            destination_phy_owner: config.destination_phy_owner.map(Into::into),
            max_admissions_per_claim: config.max_admissions_per_claim,
            lease_duration_ns: config.lease_duration_ns,
            idle_poll: config.idle_poll,
        };
        let inner = self.inner.start_insomnia_workers(
            Arc::clone(&instance.inner),
            owner.into(),
            endpoint,
            runtime_config,
        )?;
        Ok(InsomniaWorkerHandle { inner })
    }
}
