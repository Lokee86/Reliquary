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
    pub fn receipt(&self, id: EpisodeId, attempt: u32) -> Option<&AcceptedCompletionState> {
        self.receipts.get(&(id, attempt))
    }
    pub fn work_items(&self) -> impl Iterator<Item = &InsomniaWork> {
        self.work.values()
    }
}
