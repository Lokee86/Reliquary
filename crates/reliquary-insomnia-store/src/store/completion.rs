use super::*;

impl InsomniaStore {
    pub fn complete(
        &mut self,
        c: &mut Container,
        token: InsomniaLeaseToken,
        receipt: AcceptedCompletionState,
        now: i64,
    ) -> Result<CompletionCheck, InsomniaError> {
        self.check_owner(c)?;
        validate_receipt(&receipt)?;
        if let Some(old) = self.receipts.get(&(receipt.episode_id, receipt.attempt)) {
            return if old == &receipt {
                Ok(CompletionCheck::AlreadyAccepted)
            } else {
                Err(InsomniaError::ReceiptConflict)
            };
        }
        let work = self.active(receipt.episode_id, token, now)?;
        if work.attempt_count != receipt.attempt || receipt.completed_at_ns < receipt.started_at_ns
        {
            return Err(InsomniaError::InvalidTransition);
        }
        let bytes = encode_receipt(self.owner, &receipt)?;
        c.append(&bytes)?;
        c.sync()?;
        self.apply_receipt(receipt)?;
        Ok(CompletionCheck::Apply)
    }

    /// Validate an Insomnia receipt which will be carried by a cross-domain
    /// grouped frame. The caller owns the single append/sync boundary.
    pub fn preflight_grouped_completion(
        &self,
        container: &Container,
        token: InsomniaLeaseToken,
        receipt: &AcceptedCompletionState,
        now: i64,
    ) -> Result<bool, InsomniaError> {
        self.check_owner(container)?;
        validate_receipt(receipt)?;
        if let Some(old) = self.receipts.get(&(receipt.episode_id, receipt.attempt)) {
            return if old == receipt {
                Ok(false)
            } else {
                Err(InsomniaError::ReceiptConflict)
            };
        }
        let work = self.active(receipt.episode_id, token, now)?;
        if work.attempt_count != receipt.attempt || receipt.completed_at_ns < receipt.started_at_ns
        {
            return Err(InsomniaError::InvalidTransition);
        }
        let mut projected = self.clone_for_preflight();
        projected.apply_receipt(receipt.clone())?;
        Ok(true)
    }

    /// Apply a grouped receipt after the shared carrier has been synced.
    pub fn apply_grouped_completion(
        &mut self,
        container: &Container,
        receipt: AcceptedCompletionState,
    ) -> Result<CompletionCheck, InsomniaError> {
        self.check_owner(container)?;
        validate_receipt(&receipt)?;
        if let Some(old) = self.receipts.get(&(receipt.episode_id, receipt.attempt)) {
            return if old == &receipt {
                Ok(CompletionCheck::AlreadyAccepted)
            } else {
                Err(InsomniaError::ReceiptConflict)
            };
        }
        self.accept_episode(receipt.episode_id)?;
        self.apply_receipt(receipt)?;
        Ok(CompletionCheck::Apply)
    }

    pub(super) fn clone_for_preflight(&self) -> Self {
        Self {
            owner: self.owner,
            identity: self.identity,
            accepted: self.accepted.clone(),
            work: self.work.clone(),
            work_history_digests: self.work_history_digests.clone(),
            orders: self.orders.clone(),
            receipts: self.receipts.clone(),
        }
    }
    pub(super) fn persist_changed(
        &mut self,
        c: &mut Container,
        mut w: InsomniaWork,
    ) -> Result<InsomniaWork, InsomniaError> {
        w.revision = w
            .revision
            .checked_add(1)
            .ok_or(InsomniaError::VersionExhausted)?;
        let order = self
            .orders
            .get(&w.episode_id)
            .cloned()
            .ok_or(InsomniaError::UnacceptedEpisode)?;
        self.persist_work(c, &order, w.clone())?;
        Ok(w)
    }
    pub(super) fn persist_work(
        &mut self,
        c: &mut Container,
        order: &AcceptedEpisodeOrder,
        w: InsomniaWork,
    ) -> Result<(), InsomniaError> {
        self.check_owner(c)?;
        let bytes = encode_work(self.owner, order, &w)?;
        c.append(&bytes)?;
        c.sync()?;
        self.apply_work(order.clone(), w)
    }
    pub(super) fn apply_work(
        &mut self,
        order: AcceptedEpisodeOrder,
        w: InsomniaWork,
    ) -> Result<(), InsomniaError> {
        validate_work(&w)?;
        let expected = self
            .work
            .get(&w.episode_id)
            .map_or(1, |v| v.revision.checked_add(1).unwrap_or(0));
        if expected == 0 || w.revision != expected {
            return Err(InsomniaError::CorruptRecord("work revision sequence"));
        }
        if self
            .orders
            .get(&w.episode_id)
            .is_some_and(|old| old != &order)
        {
            return Err(InsomniaError::CorruptRecord("Episode order changed"));
        }
        let bytes = encode_work(self.owner, &order, &w)?;
        let mut hasher = Sha256::new();
        hasher.update(b"reliquary:insomnia-work-history:v1\\0");
        hasher.update(
            self.work_history_digests
                .get(&w.episode_id)
                .copied()
                .unwrap_or([0; 32]),
        );
        hasher.update(bytes);
        let digest: [u8; 32] = hasher.finalize().into();
        self.orders.insert(w.episode_id, order);
        let id = w.episode_id;
        self.work_history_digests.insert(id, digest);
        self.work.insert(id, w);
        self.project_receipt(id);
        Ok(())
    }
    pub(super) fn apply_receipt(
        &mut self,
        r: AcceptedCompletionState,
    ) -> Result<(), InsomniaError> {
        if let Some(old) = self.receipts.get(&(r.episode_id, r.attempt)) {
            return if old == &r {
                Ok(())
            } else {
                Err(InsomniaError::ReceiptConflict)
            };
        }
        if self
            .receipts
            .keys()
            .any(|(id, a)| *id == r.episode_id && *a >= r.attempt)
        {
            return Err(InsomniaError::CorruptRecord("receipt attempt order"));
        }
        if self
            .work
            .get(&r.episode_id)
            .is_some_and(|w| r.attempt < w.attempt_count)
        {
            return Err(InsomniaError::CorruptRecord(
                "receipt predates current attempt",
            ));
        }
        self.receipts.insert((r.episode_id, r.attempt), r.clone());
        self.project_receipt(r.episode_id);
        Ok(())
    }
    pub(super) fn project_receipt(&mut self, id: EpisodeId) {
        let latest = self
            .receipts
            .range((id, 0)..=(id, u32::MAX))
            .next_back()
            .map(|(_, receipt)| receipt);
        if let (Some(receipt), Some(w)) = (latest, self.work.get_mut(&id)) {
            if receipt.attempt >= w.attempt_count {
                w.state = InsomniaWorkState::Complete;
                w.attempt_count = receipt.attempt;
                clear_lease(w);
                w.retry_after_ns = None;
                w.last_error = None;
                w.held_payload = None;
                w.updated_at_ns = receipt.completed_at_ns;
            }
        }
    }
    pub(super) fn accept_order(&mut self, o: &AcceptedEpisodeOrder) -> Result<(), InsomniaError> {
        if self.accepted.get(&o.episode_id) != Some(o) {
            return Err(InsomniaError::UnacceptedEpisode);
        }
        Ok(())
    }
    pub(super) fn accept_episode(&self, id: EpisodeId) -> Result<(), InsomniaError> {
        if self.accepted.contains_key(&id) {
            Ok(())
        } else {
            Err(InsomniaError::UnacceptedEpisode)
        }
    }
    pub(super) fn active(
        &self,
        id: EpisodeId,
        t: InsomniaLeaseToken,
        now: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        let w = self.work.get(&id).ok_or(InsomniaError::MissingWork)?;
        if w.state != InsomniaWorkState::Processing || w.lease_token != Some(t) {
            return Err(InsomniaError::InvalidLease);
        }
        if w.lease_expires_ns.is_none_or(|x| x <= now) {
            return Err(InsomniaError::LeaseExpired);
        }
        Ok(w.clone())
    }
    pub(super) fn check_owner(&self, c: &Container) -> Result<(), InsomniaError> {
        if c.owner_id() == self.owner && c.identity() == self.identity {
            Ok(())
        } else {
            Err(InsomniaError::WrongOwner)
        }
    }
}
