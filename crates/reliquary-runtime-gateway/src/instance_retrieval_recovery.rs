use super::*;

impl Instance {
    /// Recover an already host-confirmed exact manifest after Instance/process
    /// reconstruction. The host supplies the retained operation; callers cannot
    /// provide content or a consumption receipt.
    pub fn recover_agentic_memory_delivery(
        &self,
        turn: &MemoryRetrievalTurn,
        stable_id: &str,
        router: &CapabilityRouter,
    ) -> Result<bool, GatewayError> {
        validate_turn_binding(self, turn, turn.owner, &turn.turn_id)?;
        if turn.mode != RetrievalMode::Deliver {
            return Err(GatewayError::InvalidPreparedRetrieval);
        }
        let _delivery = turn
            .delivery
            .try_lock()
            .map_err(|_| GatewayError::HostStatusUnknown)?;
        let owner = turn.owner;
        self.with_rel_writer(owner, |_| ())?;
        {
            let state = turn
                .state
                .lock()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            if state
                .pending_use_id
                .as_deref()
                .is_some_and(|pending| pending != stable_id)
            {
                return Err(GatewayError::InvalidPreparedRetrieval);
            }
        }
        crate::host_memory_delivery::require_delivery_contract(router, self.gateway_binding())?;
        {
            let mut state = turn
                .state
                .lock()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            state.handoff_checked |= state.pending_use_id.is_some();
        }
        let status = router
            .adapter()
            .memory_delivery_status(stable_id)
            .map_err(GatewayError::HostAdapter)?;
        validate_turn_binding(self, turn, owner, &turn.turn_id)?;
        let operation = match status {
            MemoryDeliveryStatus::Confirmed(operation) => operation,
            MemoryDeliveryStatus::Rejected => {
                let mut state = turn
                    .state
                    .lock()
                    .map_err(|_| GatewayError::RegistryPoisoned)?;
                if state.pending_use_id.as_deref() == Some(stable_id) {
                    if state.consumption_confirmed {
                        return Err(GatewayError::HostStatusUnknown);
                    }
                    state.pending_use_id = None;
                    state.pending_profile = None;
                    state.handoff_checked = false;
                    state.generation = state.generation.saturating_add(1);
                }
                return Err(GatewayError::InvalidHandoffReceipt);
            }
            _ => return Err(GatewayError::HostStatusUnknown),
        };
        let actual_digest: [u8; 32] = Sha256::digest(&operation.payload).into();
        if operation.stable_id != stable_id || actual_digest != operation.payload_digest {
            return Err(GatewayError::InvalidHandoffReceipt);
        }
        let manifest: DeliveryManifest = serde_json::from_slice(&operation.payload)
            .map_err(|_| GatewayError::InvalidHandoffReceipt)?;
        if serde_json::to_vec(&manifest).map_err(|_| GatewayError::InvalidHandoffReceipt)?
            != operation.payload
            || manifest.principal != self.principal.as_ref()
            || manifest.owner_id != hex(&owner.as_bytes())
            || manifest.turn_id != turn.turn_id
            || manifest.turn_id.trim().is_empty()
            || stable_memory_use_id(
                owner,
                &manifest.principal,
                &manifest.turn_id,
                operation.payload_digest,
            ) != stable_id
        {
            return Err(GatewayError::InvalidHandoffReceipt);
        }
        let use_id = stable_id.to_owned();
        let ids = manifest
            .memories
            .iter()
            .map(|memory| parse_memory_id(&memory.memory_id))
            .collect::<Result<Vec<_>, _>>()?;
        if manifest.memories.is_empty()
            || ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len()
            || manifest
                .memories
                .iter()
                .any(|memory| memory.owner_id != manifest.owner_id)
        {
            return Err(GatewayError::InvalidHandoffReceipt);
        }
        let sources = ids
            .iter()
            .map(|id| FreshnessMemoryId(id.0))
            .collect::<Vec<_>>();
        // Validate the retained frontier before publishing any receipt.
        for id in manifest.selected_ids.iter().chain(&manifest.searched_ids) {
            parse_memory_id(id)?;
        }
        if let Some(query) = &manifest.query {
            RetrievalTurn::default()
                .begin_query(query)
                .map_err(|_| GatewayError::InvalidHandoffReceipt)?;
        }
        let recorded = self.with_rel_writer(owner, |writer| -> Result<bool, GatewayError> {
            validate_turn_binding(self, turn, owner, &turn.turn_id)?;
            let rel = writer.read_session();
            if let Some(existing) = rel.accepted_external_use(&use_id) {
                if existing.accepted_turn != manifest.accepted_turn || existing.sources != sources {
                    return Err(GatewayError::InvalidHandoffReceipt);
                }
                return Ok(false);
            }
            for (memory, id) in manifest.memories.iter().zip(&ids) {
                let exact = rel.memory_history(*id)?.is_some_and(|history| {
                    history.revisions.iter().any(|revision| {
                        revision.record.revision == memory.revision
                            && hex(&revision.record.body_id.0) == memory.body_id
                            && matches!(&revision.payload, reliquary_owner_session::model::PayloadState::Available(body) if body.title == memory.title && body.content == memory.content)
                    })
                });
                if !exact {
                    return Err(GatewayError::StaleRetrieval);
                }
            }
            Ok(writer.record_external_accepted_use(
                &use_id,
                sources,
                manifest.accepted_turn,
                FreshnessEventProof {
                    graph_version: manifest.graph_version,
                    community_generation: manifest.community_generation,
                    community_graph_version: manifest.community_graph_version,
                },
            )?)
        })??;
        self.restore_selected_turn(turn, &manifest, &ids, &use_id)?;
        Ok(recorded)
    }

    fn restore_selected_turn(
        &self,
        turn: &MemoryRetrievalTurn,
        manifest: &DeliveryManifest,
        ids: &[MemoryId],
        use_id: &str,
    ) -> Result<(), GatewayError> {
        validate_turn_binding(self, turn, turn.owner, &turn.turn_id)?;
        if manifest.owner_id != hex(&turn.owner.as_bytes()) || manifest.turn_id != turn.turn_id {
            return Err(GatewayError::InvalidPreparedRetrieval);
        }
        let mut entry = turn
            .state
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let is_empty = entry.turn.query().is_none()
            && entry.turn.searched().is_empty()
            && entry.turn.selected().is_empty();
        let (old_query, old_searched, old_selected) = (
            entry.turn.query().map(str::to_owned),
            entry.turn.searched().iter().copied().collect::<Vec<_>>(),
            entry.turn.selected().iter().copied().collect::<Vec<_>>(),
        );
        let restore_manifest_frontier = is_empty || entry.pending_use_id.as_deref() == Some(use_id);
        if entry
            .pending_use_id
            .as_deref()
            .is_some_and(|pending| pending != use_id)
        {
            return Err(GatewayError::InvalidPreparedRetrieval);
        }
        let selected = manifest
            .selected_ids
            .iter()
            .map(|id| parse_memory_id(id))
            .collect::<Result<Vec<_>, _>>()?;
        let searched = manifest
            .searched_ids
            .iter()
            .map(|id| parse_memory_id(id))
            .collect::<Result<Vec<_>, _>>()?;
        let selected = old_selected
            .into_iter()
            .chain(selected)
            .chain(ids.iter().copied())
            .collect::<BTreeSet<_>>();
        let mut state = RetrievalTurn::default();
        state.record_evaluated(selected.iter().copied());
        state
            .confirm_selected(selected.iter().copied())
            .map_err(|reason| GatewayError::Retrieval(reason.into()))?;
        let query = if restore_manifest_frontier {
            manifest.query.as_deref()
        } else {
            old_query.as_deref()
        };
        if let Some(query) = query {
            state
                .begin_query(query)
                .map_err(|reason| GatewayError::Retrieval(reason.into()))?;
        }
        if restore_manifest_frontier {
            state.record_evaluated(searched);
            state.record_evaluated(ids.iter().copied());
        } else {
            // Preserve the current query frontier exactly. The recovered
            // delivery remains selected, but its old declines cannot leak in.
            state.record_evaluated(old_searched);
        }
        if entry.pending_use_id.as_deref() == Some(use_id) {
            entry.profile = entry.pending_profile;
        }
        entry.turn = state;
        entry.generation = entry.generation.saturating_add(1);
        entry.pending_use_id = None;
        entry.pending_profile = None;
        entry.handoff_checked = false;
        entry.consumption_confirmed = false;
        Ok(())
    }
}
