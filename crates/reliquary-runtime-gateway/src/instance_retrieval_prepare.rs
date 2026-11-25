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
                provided_observations: BTreeSet::new(),
                pending_use_id: None,
                pending_profile: None,
                handoff_checked: false,
                consumption_confirmed: false,
                profile: None,
            }),
            mount: RelMount::capture(self, owner)?,
            mode,
            delivery: Mutex::new(()),
        })
    }

    /// Confirm the exact Observation revisions disclosed by an inspecting host.
    /// This advances turn-local recall state without publishing accepted Memory use.
    pub fn accept_inspected_observations(
        &self,
        turn: &MemoryRetrievalTurn,
        observation_version: u64,
        citations: &[ObservationCitation],
    ) -> Result<(), GatewayError> {
        if turn.mode != RetrievalMode::Inspect || citations.len() > MAX_AGENTIC_OBSERVATIONS {
            return Err(GatewayError::InvalidPreparedRetrieval);
        }
        let _delivery = turn
            .delivery
            .try_lock()
            .map_err(|_| GatewayError::HostStatusUnknown)?;
        let mut state = turn
            .state
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        validate_turn_binding(self, turn, turn.owner, &turn.turn_id)?;
        self.with_reliquary(turn.owner, |rel| {
            let current = ObservationReadCut::capture(rel);
            if current.publication != observation_version
                || citations.iter().any(|cite| {
                    !current.contains(*cite) || rel.observation_citation_text(cite).is_err()
                })
            {
                return Err(GatewayError::StaleRetrieval);
            }
            Ok(())
        })??;
        self.ensure_credential_active()?;
        state
            .provided_observations
            .extend(citations.iter().copied());
        state.generation = state.generation.saturating_add(1);
        Ok(())
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
        self.evaluate_agentic_memory_with_filter(turn, route, action, None, embed_query)
    }

    pub fn evaluate_agentic_memory_with_filter(
        &self,
        turn: &MemoryRetrievalTurn,
        route: &MemoryRetrievalRoute,
        action: RetrievalAction,
        query_filter: Option<TemporalFilter>,
        embed_query: impl FnOnce() -> Result<Option<(CompatibilityProfileId, Vec<f32>)>, GatewayError>,
    ) -> Result<RetrievalOutcome, GatewayError> {
        if query_filter.is_some_and(|filter| {
            !matches!(&action, RetrievalAction::Query(_))
                || filter.start_ns >= filter.end_exclusive_ns
        }) {
            return Err(GatewayError::InvalidPreparedRetrieval);
        }
        let owner = turn.owner;
        let turn_id = turn.turn_id.as_str();
        validate_turn_binding(self, turn, owner, turn_id)?;
        let (prior, generation, prior_profile, provided_observations) = {
            let state = turn
                .state
                .lock()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            if state.pending_use_id.is_some() {
                return Err(GatewayError::HostStatusUnknown);
            }
            (
                state.turn.clone(),
                state.generation,
                state.profile,
                state.provided_observations.clone(),
            )
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
        let (snapshot, observation_cut) =
            self.with_reliquary(owner, |rel| -> Result<_, GatewayError> {
                Ok((
                    capture_memory_snapshot(rel, profile, include_vectors)?,
                    ObservationReadCut::capture(rel),
                ))
            })??;
        if query_vector.is_some() && snapshot.vectors.is_empty() {
            return Err(GatewayError::RetrievalUnavailable(
                "no current Memory vectors for the embedding profile",
            ));
        }
        let authorized_route = AuthorizedDecision {
            instance: self,
            turn,
            generation,
            endpoint: route.0.as_ref(),
        };
        let offer_observations = !matches!(action, RetrievalAction::Traverse(_));
        let evaluation = evaluate_snapshot_retrieval(
            &authorized_route,
            &prior,
            &snapshot,
            action,
            query_vector.as_deref(),
            query_filter,
        );
        authorized_route.check()?;
        let evaluation = evaluation.map_err(GatewayError::Retrieval)?;
        let observations = self.with_reliquary(owner, |rel| {
            let current = capture_memory_snapshot(rel, profile, include_vectors).map_err(
                |error| match error {
                    GatewayError::RetrievalUnavailable(_) => GatewayError::StaleRetrieval,
                    other => other,
                },
            )?;
            let current_cut = ObservationReadCut::capture(rel);
            if snapshot != current
                || (current_cut.publication.max(observation_cut.publication) > 0
                    && observation_cut != current_cut)
            {
                return Err(GatewayError::StaleRetrieval);
            }
            Ok(if offer_observations {
                evaluation.next.query().map_or_else(Vec::new, |query| {
                    // The ranked shortlist is the exact set offered for delivery.
                    rank_current_observations(
                        rel,
                        query,
                        evaluation.next.temporal_filter(),
                        &provided_observations,
                        MAX_AGENTIC_OBSERVATIONS,
                    )
                    .0
                })
            } else {
                Vec::new()
            })
        })??;
        let preview = RetrievalPreview::from_evaluation(
            &snapshot,
            profile,
            &evaluation,
            observation_cut.publication,
            observations,
        )?;
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
        if turn.mode == RetrievalMode::Inspect
            || (evaluation.memories.is_empty() && preview.observations.is_empty())
        {
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
            temporal_filter: evaluation.next.temporal_filter(),
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
            selected_observations: provided_observations.into_iter().collect(),
            observations: preview.observations.iter().map(|hit| &hit.claim).collect(),
        })
        .map_err(|error| GatewayError::Retrieval(error.to_string()))?;
        let digest: [u8; 32] = Sha256::digest(&payload).into();
        let use_id = stable_memory_use_id(owner, &self.principal, turn_id, digest);
        let body_bindings = evaluation
            .memories
            .iter()
            .map(|m| (m.id, m.revision, m.body_id))
            .collect();
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
