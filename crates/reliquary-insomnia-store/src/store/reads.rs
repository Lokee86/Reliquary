use super::*;

impl InsomniaStore {
    /// Admit an Archive-verified Episode order for scheduling. The authority
    /// check belongs to the caller; this projection accepts exact retries and
    /// rejects attempts to redefine an Episode's ordering evidence.
    pub fn admit_order(&mut self, order: AcceptedEpisodeOrder) -> Result<(), InsomniaError> {
        validate_order(&order)?;
        if let Some(existing) = self.accepted.get(&order.episode_id) {
            return if existing == &order {
                Ok(())
            } else {
                Err(InsomniaError::UnacceptedEpisode)
            };
        }
        if self
            .orders
            .get(&order.episode_id)
            .is_some_and(|existing| existing != &order)
        {
            return Err(InsomniaError::UnacceptedEpisode);
        }
        self.accepted.insert(order.episode_id, order);
        Ok(())
    }

    pub fn open(
        container: &Container,
        accepted: impl IntoIterator<Item = AcceptedEpisodeOrder>,
    ) -> Result<Self, InsomniaError> {
        Self::open_with_grouped_completion(container, accepted, |_| {
            Err(InsomniaError::CorruptRecord(
                "grouped completion requires owner-session replay",
            ))
        })
    }

    /// Replay grouped owner completions at their physical position in the REL
    /// stream. The callback belongs to the lower owner-session carrier codec;
    /// this store only applies the authenticated Insomnia receipt projection.
    pub fn open_with_grouped_completion(
        container: &Container,
        accepted: impl IntoIterator<Item = AcceptedEpisodeOrder>,
        mut decode_grouped: impl FnMut(&[u8]) -> Result<Option<AcceptedCompletionState>, InsomniaError>,
    ) -> Result<Self, InsomniaError> {
        let mut orders = BTreeMap::new();
        for order in accepted {
            validate_order(&order)?;
            if orders.insert(order.episode_id, order).is_some() {
                return Err(InsomniaError::InvalidField("duplicate admitted Episode"));
            }
        }
        let mut store = Self {
            owner: container.owner_id(),
            identity: container.identity(),
            accepted: orders,
            work: BTreeMap::new(),
            work_history_digests: BTreeMap::new(),
            orders: BTreeMap::new(),
            receipts: BTreeMap::new(),
        };
        for frame in container.read_frames()? {
            if frame.payload.starts_with(WORK_MAGIC) || frame.payload.starts_with(WORK_MAGIC_V2) {
                let (owner, order, work) = decode_work(&frame.payload)?;
                if owner != store.owner {
                    return Err(InsomniaError::WrongOwner);
                };
                store.accept_order(&order)?;
                store.apply_work(order, work)?;
            } else if frame.payload.starts_with(RECEIPT_MAGIC) {
                let (owner, receipt) = decode_receipt(&frame.payload)?;
                if owner != store.owner {
                    return Err(InsomniaError::WrongOwner);
                };
                store.accept_episode(receipt.episode_id)?;
                store.apply_receipt(receipt)?;
            } else if frame.payload.starts_with(b"RELINSC7") {
                if let Some(receipt) = decode_grouped(&frame.payload)? {
                    validate_receipt(&receipt)?;
                    store.accept_episode(receipt.episode_id)?;
                    store.apply_receipt(receipt)?;
                }
            }
        }
        Ok(store)
    }
    /// Decode every retained work-journal revision for one Episode. Used by
    /// verified owner maintenance to replay terminal/retry history through the
    /// same state machine, rather than copying opaque physical frames.
    pub fn work_history(
        container: &Container,
        episode_id: EpisodeId,
    ) -> Result<Vec<InsomniaWork>, InsomniaError> {
        let mut history = Vec::new();
        let mut order = None;
        for frame in container.read_frames()? {
            if !(frame.payload.starts_with(WORK_MAGIC) || frame.payload.starts_with(WORK_MAGIC_V2))
            {
                continue;
            }
            let (owner, candidate_order, work) = decode_work(&frame.payload)?;
            if owner != container.owner_id() {
                return Err(InsomniaError::WrongOwner);
            }
            if work.episode_id != episode_id {
                continue;
            }
            if order.as_ref().is_some_and(|old| old != &candidate_order)
                || work.revision != history.len() as u64 + 1
            {
                return Err(InsomniaError::CorruptRecord(
                    "historical Insomnia work order or revision changed",
                ));
            }
            order = Some(candidate_order);
            history.push(work);
        }
        Ok(history)
    }

    /// Visit owner-accepted checkpoints without materializing unrelated frames.
    pub fn visit_completed_checkpoints(
        &self,
        container: &Container,
        mut visit: impl FnMut(EpisodeId, &[u8]),
    ) -> Result<(), InsomniaError> {
        use std::io::Read;
        self.check_owner(container)?;
        let mut cursor = container.physical_cursor()?;
        while let Some(mut frame) = cursor.next_frame()? {
            let mut prefix = [0; 8];
            if frame.object().length < 8 {
                continue;
            }
            frame.read_exact(&mut prefix).map_err(ContainerError::Io)?;
            if &prefix != WORK_MAGIC_V2 {
                continue;
            }
            const MAX_WORK_FRAME_BYTES: u64 =
                (MAX_HELD_PAYLOAD_BYTES + 4 * MAX_RECEIPT_TEXT_BYTES + 128) as u64;
            if frame.object().length > MAX_WORK_FRAME_BYTES {
                return Err(InsomniaError::CorruptRecord("work frame exceeds its bound"));
            }
            let mut bytes = prefix.to_vec();
            frame.read_to_end(&mut bytes).map_err(ContainerError::Io)?;
            let (owner, _, work) = decode_work(&bytes)?;
            if owner != self.owner {
                return Err(InsomniaError::WrongOwner);
            }
            if work.state == InsomniaWorkState::Processing
                && let Some(payload) = work.held_payload.as_deref()
                && let Some(receipt) = self.receipt(work.episode_id, work.attempt_count)
                && self.work(work.episode_id).is_some_and(|current| {
                    // Receipt projection clears the payload but preserves the
                    // exact work revision, including equal-timestamp checkpoints.
                    current.attempt_count == receipt.attempt && current.revision == work.revision
                })
                && receipt.started_at_ns == work.updated_at_ns
            {
                visit(work.episode_id, payload);
            }
        }
        Ok(())
    }

    pub fn owner_id(&self) -> [u8; 16] {
        self.owner
    }
    pub fn accepted_order(&self, id: EpisodeId) -> Option<&AcceptedEpisodeOrder> {
        self.accepted.get(&id)
    }
    /// Deterministic owner-admitted scheduling input for bounded workers.
    pub fn accepted_orders(&self) -> impl Iterator<Item = &AcceptedEpisodeOrder> {
        self.accepted.values()
    }
    /// Return a deterministic, bounded batch of admitted episodes that have
    /// never entered the work queue. Completed work is not re-enqueued.
    pub fn unqueued_orders(&self, limit: usize) -> Vec<AcceptedEpisodeOrder> {
        let mut orders = self
            .accepted
            .iter()
            .filter(|(id, _)| !self.work.contains_key(id))
            .map(|(_, order)| order.clone())
            .collect::<Vec<_>>();
        orders.sort_by(|left, right| {
            left.source_through_ns
                .cmp(&right.source_through_ns)
                .then_with(|| left.conversation_id.cmp(&right.conversation_id))
                .then_with(|| left.start_node_id.cmp(&right.start_node_id))
                .then_with(|| left.episode_id.cmp(&right.episode_id))
        });
        orders.truncate(limit);
        orders
    }
    pub fn work(&self, id: EpisodeId) -> Option<&InsomniaWork> {
        self.work.get(&id)
    }
    /// SHA-256 chain of accepted work-journal revisions in exact owner order.
    pub fn work_history_digest(&self, id: EpisodeId) -> Option<[u8; 32]> {
        self.work_history_digests.get(&id).copied()
    }
    pub fn receipt(&self, id: EpisodeId, attempt: u32) -> Option<&AcceptedCompletionState> {
        self.receipts.get(&(id, attempt))
    }
    pub fn work_items(&self) -> impl Iterator<Item = &InsomniaWork> {
        self.work.values()
    }
}
