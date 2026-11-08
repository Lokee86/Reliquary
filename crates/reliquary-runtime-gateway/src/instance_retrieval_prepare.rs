use super::*;

impl Instance {
    /// Start the established trusted-delivery controller.
    pub fn begin_memory_retrieval_turn(
        &self,
        owner: OwnerId,
        turn_id: &str,
    ) -> Result<MemoryRetrievalTurn, GatewayError> {
        self.begin_memory_retrieval(owner, turn_id, RetrievalMode::Deliver)
    }

    pub fn begin_memory_retrieval(
        &self,
        owner: OwnerId,
        turn_id: &str,
        mode: RetrievalMode,
    ) -> Result<MemoryRetrievalTurn, GatewayError> {
        validate_turn_id(turn_id)?;
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner,
            });
        }
        Ok(MemoryRetrievalTurn {
            instance_id: self.instance_id,
            principal: self.principal.to_string(),
            owner,
            turn_id: turn_id.to_owned(),
            controller_id: Uuid::new_v4(),
            state: Mutex::new(TurnState {
                turn: RetrievalTurn::default(),
                generation: 0,
                pending_use_id: None,
                pending_profile: None,
                handoff_checked: false,
                consumption_confirmed: false,
                profile: None,
            }),
            mount: RetrievalMount::capture(self, owner)?,
            mode,
            delivery: Mutex::new(()),
        })
    }

    pub fn prepare_agentic_memory_delivery(
        &self,
        turn: &MemoryRetrievalTurn,
        route: &MemoryRetrievalRoute,
        action: RetrievalAction,
        profile: Option<CompatibilityProfileId>,
        query_vector: Option<&[f32]>,
    ) -> Result<RetrievalOutcome, GatewayError> {
        if turn.mode != RetrievalMode::Deliver {
            return Err(GatewayError::InvalidPreparedRetrieval);
        }
        self.evaluate_agentic_memory(turn, route, action, || {
            Ok(profile.zip(query_vector.map(<[f32]>::to_vec)))
        })
    }

    /// Evaluate either immutable session mode. Query embedding happens inside
    /// the same generation check as decision evaluation so cancel covers both.
    pub fn evaluate_agentic_memory(
        &self,
        turn: &MemoryRetrievalTurn,
        route: &MemoryRetrievalRoute,
        action: RetrievalAction,
        embed_query: impl FnOnce() -> Result<Option<(CompatibilityProfileId, Vec<f32>)>, GatewayError>,
    ) -> Result<RetrievalOutcome, GatewayError> {
        let owner = turn.owner;
        let turn_id = turn.turn_id.as_str();
        validate_turn_binding(self, turn, owner, turn_id)?;
        let (prior, generation, prior_profile) = {
            let state = turn
                .state
                .lock()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            if state.pending_use_id.is_some() {
                return Err(GatewayError::HostStatusUnknown);
            }
            (state.turn.clone(), state.generation, state.profile)
        };
        let (profile, query_vector) = match &action {
            RetrievalAction::Query(_) => match embed_query()? {
                Some((profile, vector)) => (Some(profile), Some(vector)),
                None => (None, None),
            },
            _ => (prior_profile, None),
        };
        validate_turn_binding(self, turn, owner, turn_id)?;
        let include_vectors = query_vector.is_some();
        let snapshot = self.retrieval_cut_snapshot(owner, profile, include_vectors)?;
        if query_vector.is_some() && snapshot.vectors.is_empty() {
            return Err(GatewayError::RetrievalUnavailable(
                "no current Memory vectors for the embedding profile",
            ));
        }
        let memories = reliquary_application::agentic_memories(&snapshot);
        let authorized_route = AuthorizedDecision {
            instance: self,
            turn,
            generation,
            endpoint: route.0.as_ref(),
        };
        let evaluation = evaluate_retrieval(
            &authorized_route,
            owner.as_bytes(),
            &prior,
            &memories,
            &snapshot.active_relations,
            action,
            query_vector.as_deref(),
        );
        authorized_route.check()?;
        let evaluation = evaluation.map_err(GatewayError::Retrieval)?;
        let current = self
            .retrieval_cut_snapshot(owner, profile, include_vectors)
            .map_err(|error| match error {
                GatewayError::RetrievalUnavailable(_) => GatewayError::StaleRetrieval,
                other => other,
            })?;
        if snapshot != current {
            return Err(GatewayError::StaleRetrieval);
        }
        let preview = RetrievalPreview::from_evaluation(&snapshot, profile, &evaluation)?;
        let _delivery = turn
            .delivery
            .try_lock()
            .map_err(|_| GatewayError::HostStatusUnknown)?;
        let mut state = turn
            .state
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        validate_turn_binding(self, turn, owner, turn_id)?;
        if state.generation != generation || state.pending_use_id.is_some() {
            return Err(GatewayError::StaleRetrieval);
        }
        if turn.mode == RetrievalMode::Inspect || evaluation.memories.is_empty() {
            state.turn = evaluation.next;
            state.profile = profile;
            state.generation = state.generation.saturating_add(1);
            return Ok(RetrievalOutcome {
                prepared: None,
                preview: Some(preview),
                already_provided: evaluation.already_provided,
                anchors: evaluation.anchors,
            });
        }
        let payload = serde_json::to_vec(&DeliveryPayload {
            principal: &self.principal,
            owner_id: hex(&owner.as_bytes()),
            turn_id,
            accepted_turn: snapshot.rel_turn,
            graph_version: snapshot.graph_version,
            community_generation: snapshot.community.as_ref().map(|c| c.generation),
            community_graph_version: snapshot.community.as_ref().map(|c| c.graph_version),
            query: evaluation.next.query(),
            searched_ids: evaluation
                .next
                .searched()
                .iter()
                .map(|id| hex(&id.0))
                .collect(),
            selected_ids: evaluation
                .next
                .selected()
                .iter()
                .map(|id| hex(&id.0))
                .collect(),
            memories: evaluation
                .memories
                .iter()
                .map(|memory| DeliveredMemory {
                    owner_id: hex(&owner.as_bytes()),
                    memory_id: hex(&memory.id.0),
                    revision: memory.revision,
                    body_id: hex(&memory.body_id.0),
                    title: &memory.body.title,
                    content: &memory.body.content,
                })
                .collect(),
        })
        .map_err(|error| GatewayError::Retrieval(error.to_string()))?;
        let digest: [u8; 32] = Sha256::digest(&payload).into();
        let use_id = stable_memory_use_id(owner, &self.principal, turn_id, digest);
        let selected_ids = evaluation
            .memories
            .iter()
            .map(|m| m.id)
            .collect::<BTreeSet<_>>();
        let body_bindings = memories_for_bindings(&memories, &selected_ids);
        state.pending_use_id = Some(use_id.clone());
        state.pending_profile = profile;
        state.handoff_checked = false;
        state.consumption_confirmed = false;
        let anchors = evaluation.anchors.clone();
        Ok(RetrievalOutcome {
            already_provided: BTreeSet::new(),
            anchors,
            preview: None,
            prepared: Some(PreparedMemoryDelivery {
                instance_id: self.instance_id,
                principal: self.principal.to_string(),
                owner,
                turn_id: turn_id.to_owned(),
                controller_id: turn.controller_id,
                generation,
                use_id,
                digest,
                payload,
                evaluation,
                preview,
                profile,
                proof: FreshnessEventProof {
                    graph_version: snapshot.graph_version,
                    community_generation: snapshot.community.as_ref().map(|c| c.generation),
                    community_graph_version: snapshot.community.as_ref().map(|c| c.graph_version),
                },
                accepted_turn: snapshot.rel_turn,
                body_bindings,
            }),
        })
    }

    pub fn preview_agentic_memory_delivery(
        &self,
        turn: &MemoryRetrievalTurn,
        prepared: &PreparedMemoryDelivery,
    ) -> Result<RetrievalPreview, GatewayError> {
        self.validate_pending(turn, prepared)?;
        self.revalidate_before_handoff(turn, prepared)?;
        self.validate_pending(turn, prepared)?;
        Ok(prepared.preview.clone())
    }

    /// Cancel unaccepted evaluation or discard a preparation before host I/O.
    pub fn cancel_memory_retrieval(&self, turn: &MemoryRetrievalTurn) -> Result<(), GatewayError> {
        validate_turn_binding(self, turn, turn.owner, &turn.turn_id)?;
        let _delivery = turn
            .delivery
            .try_lock()
            .map_err(|_| GatewayError::HostStatusUnknown)?;
        let mut state = turn
            .state
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        if state.pending_use_id.is_some() && state.handoff_checked {
            return Err(GatewayError::HostStatusUnknown);
        }
        state.pending_use_id = None;
        state.pending_profile = None;
        state.generation = state.generation.saturating_add(1);
        Ok(())
    }
}

struct AuthorizedDecision<'a> {
    instance: &'a Instance,
    turn: &'a MemoryRetrievalTurn,
    generation: u64,
    endpoint: &'a dyn DecisionEndpoint,
}

impl AuthorizedDecision<'_> {
    fn check(&self) -> Result<(), GatewayError> {
        validate_turn_binding(
            self.instance,
            self.turn,
            self.turn.owner,
            &self.turn.turn_id,
        )?;
        let state = self
            .turn
            .state
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        if state.generation != self.generation || state.pending_use_id.is_some() {
            return Err(GatewayError::StaleRetrieval);
        }
        Ok(())
    }
}

impl DecisionEndpoint for AuthorizedDecision<'_> {
    fn model(&self) -> &str {
        self.endpoint.model()
    }

    fn evaluate(
        &self,
        state: &serde_json::Value,
        shape: &serde_json::Value,
    ) -> Result<serde_json::Value, DecisionEndpointError> {
        self.check()
            .map_err(|error| DecisionEndpointError::Failure(error.to_string()))?;
        self.endpoint.evaluate(state, shape)
    }
}
