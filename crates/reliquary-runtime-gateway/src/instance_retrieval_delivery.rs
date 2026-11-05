use super::*;

impl Instance {
    /// Idempotently deliver selected content, then persist accepted use. A
    /// Pending/unknown host response never updates turn selection or Freshness.
    pub fn deliver_agentic_memory(
        &self,
        turn: &MemoryRetrievalTurn,
        prepared: &PreparedMemoryDelivery,
        router: &CapabilityRouter,
    ) -> Result<bool, GatewayError> {
        validate_prepared(self, turn, prepared)?;
        let sources = prepared
            .evaluation
            .memories
            .iter()
            .map(|memory| FreshnessMemoryId(memory.id.0))
            .collect::<Vec<_>>();
        if let Some(receipt) = self.accepted_use_receipt(prepared.owner, &prepared.use_id)? {
            if receipt.accepted_turn != prepared.accepted_turn || receipt.sources != sources {
                return Err(GatewayError::InvalidPreparedRetrieval);
            }
            if self.pending_matches(turn, prepared)? {
                self.commit_selected(turn, prepared)?;
                self.clear_pending(turn, prepared)?;
            }
            return Ok(false);
        }
        crate::host_memory_delivery::require_delivery_contract(router, self.gateway_binding())?;
        self.validate_pending(turn, prepared)?;
        // Establish writer availability before the external receiver can
        // consume content; a read-only mount may prepare, but cannot deliver.
        self.with_rel_writer(prepared.owner, |_| ())?;
        let operation = MemoryDeliveryOperation {
            stable_id: prepared.use_id.clone(),
            payload_digest: prepared.digest,
            payload: prepared.payload.clone(),
        };
        let status = router
            .adapter()
            .memory_delivery_status(&prepared.use_id)
            .map_err(GatewayError::HostAdapter)?;
        match status {
            MemoryDeliveryStatus::Confirmed(confirmed)
                if confirmed.payload_digest == prepared.digest
                    && confirmed.payload == prepared.payload
                    && confirmed.stable_id == prepared.use_id => {}
            MemoryDeliveryStatus::Confirmed(_) => {
                return Err(GatewayError::InvalidHandoffReceipt);
            }
            MemoryDeliveryStatus::Pending | MemoryDeliveryStatus::Unknown => {
                return Err(GatewayError::HostStatusUnknown);
            }
            MemoryDeliveryStatus::Rejected => {
                self.clear_pending(turn, prepared)?;
                return Err(GatewayError::InvalidHandoffReceipt);
            }
            MemoryDeliveryStatus::NotFound => {
                self.revalidate_before_handoff(turn, prepared)?;
                let receipt = router
                    .adapter()
                    .deliver_memory(&operation)
                    .map_err(GatewayError::HostAdapter)?;
                if receipt.stable_id != prepared.use_id
                    || receipt.payload_digest != prepared.digest
                    || !receipt.confirmed_consumption
                {
                    return Err(GatewayError::InvalidHandoffReceipt);
                }
            }
        }
        self.commit_selected(turn, prepared)?;
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&prepared.owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner: prepared.owner,
            });
        }
        let recorded = self.with_rel_writer(prepared.owner, |writer| {
            writer.record_external_accepted_use(
                &prepared.use_id,
                sources.clone(),
                prepared.accepted_turn,
                prepared.proof,
            )
        })??;
        self.clear_pending(turn, prepared)?;
        Ok(recorded)
    }

    fn revalidate_before_handoff(
        &self,
        turn: &MemoryRetrievalTurn,
        prepared: &PreparedMemoryDelivery,
    ) -> Result<(), GatewayError> {
        validate_prepared(self, turn, prepared)?;
        let snapshot = self.retrieval_snapshot(prepared.owner, None)?;
        if snapshot.owner_id != prepared.owner.as_bytes()
            || prepared
                .body_bindings
                .iter()
                .any(|(id, revision, body_id)| {
                    !snapshot.memories.iter().any(|current| {
                        current.revision.record.id == *id
                            && current.revision.record.revision == *revision
                            && current.revision.record.body_id == *body_id
                    })
                })
        {
            return Err(GatewayError::StaleRetrieval);
        }
        if snapshot.graph_version != prepared.proof.graph_version
            || snapshot.community.as_ref().map(|c| c.generation)
                != prepared.proof.community_generation
            || snapshot.community.as_ref().map(|c| c.graph_version)
                != prepared.proof.community_graph_version
        {
            // A durable receipt takes precedence on a retry after a later graph cut.
            let already_recorded = self
                .accepted_use_receipt(prepared.owner, &prepared.use_id)?
                .is_some();
            if !already_recorded {
                return Err(GatewayError::StaleRetrieval);
            }
        }
        Ok(())
    }

    fn validate_pending(
        &self,
        turn: &MemoryRetrievalTurn,
        prepared: &PreparedMemoryDelivery,
    ) -> Result<(), GatewayError> {
        if self.pending_matches(turn, prepared)? {
            Ok(())
        } else {
            Err(GatewayError::InvalidPreparedRetrieval)
        }
    }

    fn pending_matches(
        &self,
        turn: &MemoryRetrievalTurn,
        prepared: &PreparedMemoryDelivery,
    ) -> Result<bool, GatewayError> {
        validate_prepared(self, turn, prepared)?;
        let state = turn
            .state
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        Ok(state.pending_use_id.as_deref() == Some(prepared.use_id.as_str()))
    }

    fn commit_selected(
        &self,
        turn: &MemoryRetrievalTurn,
        prepared: &PreparedMemoryDelivery,
    ) -> Result<(), GatewayError> {
        validate_turn_controller(self, turn, prepared.owner, &prepared.turn_id)?;
        let mut state = turn
            .state
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        if state.pending_use_id.as_deref() != Some(prepared.use_id.as_str()) {
            return Err(GatewayError::InvalidPreparedRetrieval);
        }
        state.turn = prepared.evaluation.next.clone();
        state
            .turn
            .confirm_selected(prepared.evaluation.memories.iter().map(|memory| memory.id))
            .map_err(|reason| GatewayError::Retrieval(reason.into()))?;
        state.generation = state.generation.saturating_add(1);
        Ok(())
    }

    fn clear_pending(
        &self,
        turn: &MemoryRetrievalTurn,
        prepared: &PreparedMemoryDelivery,
    ) -> Result<(), GatewayError> {
        validate_turn_controller(self, turn, prepared.owner, &prepared.turn_id)?;
        let mut state = turn
            .state
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        if state.pending_use_id.as_deref() != Some(prepared.use_id.as_str()) {
            return Err(GatewayError::InvalidPreparedRetrieval);
        }
        state.pending_use_id = None;
        Ok(())
    }

    fn accepted_use_receipt(
        &self,
        owner: OwnerId,
        use_id: &str,
    ) -> Result<Option<reliquary_owner_session::FreshnessUseReceipt>, GatewayError> {
        match self.reliquary(owner) {
            Ok(rel) => Ok(rel.accepted_external_use(use_id).cloned()),
            Err(GatewayError::WriterUnavailable) => self
                .with_writable_reliquary(owner, |rel| rel.accepted_external_use(use_id).cloned()),
            Err(error) => Err(error),
        }
    }
}
