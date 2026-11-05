use super::*;

impl Instance {
    /// Start a host-owned controller for one authenticated external turn.
    pub fn begin_memory_retrieval_turn(
        &self,
        owner: OwnerId,
        turn_id: &str,
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
            state: std::sync::Mutex::new(TurnState {
                turn: RetrievalTurn::default(),
                generation: 0,
                pending_use_id: None,
            }),
        })
    }

    /// Read an immutable REL snapshot, evaluate the decision route without an
    /// owner lock, then recheck Instance authorization before returning a token.
    pub fn prepare_agentic_memory_delivery(
        &self,
        turn: &MemoryRetrievalTurn,
        route: &MemoryRetrievalRoute,
        action: RetrievalAction,
        profile: Option<reliquary_owner_session::model::CompatibilityProfileId>,
        query_vector: Option<&[f32]>,
    ) -> Result<RetrievalOutcome, GatewayError> {
        validate_turn_binding(self, turn, turn.owner, &turn.turn_id)?;
        let owner = turn.owner;
        let turn_id = turn.turn_id.as_str();
        let (prior, generation) = {
            let state = turn
                .state
                .lock()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            if state.pending_use_id.is_some() {
                return Err(GatewayError::Retrieval(
                    "turn has a pending confirmed-delivery receipt".into(),
                ));
            }
            (state.turn.clone(), state.generation)
        };
        let snapshot = self.retrieval_snapshot(owner, profile)?;
        if snapshot.owner_id != owner.as_bytes() {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner,
            });
        }
        let memories = reliquary_application::agentic_memories(&snapshot);
        let evaluation = evaluate_retrieval(
            route.0.as_ref(),
            owner.as_bytes(),
            &prior,
            &memories,
            &snapshot.active_relations,
            action,
            query_vector,
        )
        .map_err(|reason| GatewayError::Retrieval(reason))?;
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner,
            });
        }
        if evaluation.memories.is_empty() {
            let mut state = turn
                .state
                .lock()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            if state.pending_use_id.is_some() {
                return Err(GatewayError::Retrieval(
                    "turn has a pending confirmed-delivery receipt".into(),
                ));
            }
            if state.generation != generation {
                return Err(GatewayError::Retrieval(
                    "retrieval turn changed during evaluation".into(),
                ));
            }
            state.turn = evaluation.next.clone();
            state.generation = state.generation.saturating_add(1);
            return Ok(RetrievalOutcome {
                prepared: None,
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
        {
            let mut state = turn
                .state
                .lock()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            if state.pending_use_id.is_some() {
                return Err(GatewayError::Retrieval(
                    "turn already has a pending delivery".into(),
                ));
            }
            if state.generation != generation {
                return Err(GatewayError::Retrieval(
                    "retrieval turn changed during evaluation".into(),
                ));
            }
            state.pending_use_id = Some(use_id.clone());
        }
        let anchors = evaluation.anchors.clone();
        Ok(RetrievalOutcome {
            already_provided: BTreeSet::new(),
            anchors,
            prepared: Some(PreparedMemoryDelivery {
                instance_id: self.instance_id,
                principal: self.principal.to_string(),
                owner,
                turn_id: turn_id.to_owned(),
                controller_id: turn.controller_id,
                use_id,
                digest,
                payload,
                evaluation,
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
}
