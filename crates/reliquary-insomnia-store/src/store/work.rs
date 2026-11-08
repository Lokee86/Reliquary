use super::*;

impl InsomniaStore {
    pub fn queue(
        &mut self,
        c: &mut Container,
        order: AcceptedEpisodeOrder,
        priority: InsomniaPriority,
        now: i64,
    ) -> Result<(InsomniaWork, bool), InsomniaError> {
        self.check_owner(c)?;
        validate_order(&order)?;
        self.accept_order(&order)?;
        if let Some(w) = self.work.get(&order.episode_id) {
            return Ok((w.clone(), false));
        }
        let latest_receipt = self
            .receipts
            .range((order.episode_id, 0)..=(order.episode_id, u32::MAX))
            .next_back()
            .map(|(_, receipt)| receipt.clone());
        let w = InsomniaWork {
            episode_id: order.episode_id,
            priority,
            state: if latest_receipt.is_some() {
                InsomniaWorkState::Complete
            } else {
                InsomniaWorkState::Pending
            },
            attempt_count: latest_receipt.as_ref().map_or(0, |r| r.attempt),
            lease_owner: None,
            lease_token: None,
            lease_expires_ns: None,
            retry_after_ns: None,
            last_error: None,
            held_payload: None,
            updated_at_ns: latest_receipt.as_ref().map_or(now, |r| r.completed_at_ns),
            revision: 1,
        };
        self.persist_work(c, &order, w.clone())?;
        Ok((w, true))
    }
    pub fn claim_next(
        &mut self,
        c: &mut Container,
        worker: &str,
        now: i64,
        duration: i64,
    ) -> Result<Option<InsomniaWork>, InsomniaError> {
        self.check_owner(c)?;
        if worker.trim().is_empty() || duration <= 0 {
            return Err(InsomniaError::InvalidField("worker lease"));
        };
        let id = self
            .work
            .iter()
            .filter(|(_, w)| {
                matches!(w.state, InsomniaWorkState::Pending)
                    || w.state == InsomniaWorkState::Failed
                        && w.retry_after_ns.is_none_or(|t| t <= now)
                    || w.state == InsomniaWorkState::Processing
                        && w.lease_expires_ns.is_none_or(|t| t <= now)
            })
            .filter(|(id, _)| self.orders.contains_key(id))
            .min_by_key(|(id, w)| {
                (
                    w.priority,
                    self.orders[*id].source_through_ns,
                    self.orders[*id].conversation_id.clone(),
                    self.orders[*id].start_node_id.clone(),
                    **id,
                )
            })
            .map(|(id, _)| *id);
        let Some(id) = id else { return Ok(None) };
        self.claim_episode(c, id, worker, now, duration).map(Some)
    }

    /// Claim one specifically requested, previously admitted Episode.
    pub fn claim_episode(
        &mut self,
        c: &mut Container,
        id: EpisodeId,
        worker: &str,
        now: i64,
        duration: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.check_owner(c)?;
        if worker.trim().is_empty() || duration <= 0 {
            return Err(InsomniaError::InvalidField("worker lease"));
        }
        if !self.orders.contains_key(&id) {
            return Err(InsomniaError::UnacceptedEpisode);
        }
        let Some(w) = self.work.get(&id).cloned() else {
            return Err(InsomniaError::MissingWork);
        };
        let claimable = matches!(w.state, InsomniaWorkState::Pending)
            || w.state == InsomniaWorkState::Failed && w.retry_after_ns.is_none_or(|t| t <= now)
            || w.state == InsomniaWorkState::Processing
                && w.lease_expires_ns.is_none_or(|t| t <= now);
        if !claimable {
            return Err(InsomniaError::InvalidTransition);
        }
        let mut w = w;
        w.attempt_count = w
            .attempt_count
            .checked_add(1)
            .ok_or(InsomniaError::VersionExhausted)?;
        w.state = InsomniaWorkState::Processing;
        w.lease_owner = Some(worker.trim().to_owned());
        w.lease_token = Some(token(id, worker, w.attempt_count, now));
        w.lease_expires_ns = Some(
            now.checked_add(duration)
                .ok_or(InsomniaError::InvalidField("lease expiry"))?,
        );
        w.retry_after_ns = None;
        w.last_error = None;
        w.updated_at_ns = now;
        w.revision = w
            .revision
            .checked_add(1)
            .ok_or(InsomniaError::VersionExhausted)?;
        let order = self.orders[&id].clone();
        self.persist_work(c, &order, w.clone())?;
        Ok(w)
    }
    pub fn renew(
        &mut self,
        c: &mut Container,
        id: EpisodeId,
        t: InsomniaLeaseToken,
        now: i64,
        duration: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.check_owner(c)?;
        if duration <= 0 {
            return Err(InsomniaError::InvalidField("lease duration"));
        }
        let mut w = self.active(id, t, now)?;
        w.lease_expires_ns = Some(
            now.checked_add(duration)
                .ok_or(InsomniaError::InvalidField("lease expiry"))?,
        );
        w.updated_at_ns = now;
        self.persist_changed(c, w)
    }
    /// Durably checkpoint bounded model output before any publication attempt.
    /// Expired-lease recovery can reuse these exact bytes instead of invoking
    /// the provider again.
    pub fn checkpoint_payload(
        &mut self,
        c: &mut Container,
        id: EpisodeId,
        token: InsomniaLeaseToken,
        now: i64,
        payload: Vec<u8>,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.check_owner(c)?;
        if payload.is_empty() || payload.len() > MAX_HELD_PAYLOAD_BYTES {
            return Err(InsomniaError::InvalidField("checkpoint payload size"));
        }
        let mut w = self.active(id, token, now)?;
        w.held_payload = Some(payload);
        w.updated_at_ns = now;
        self.persist_changed(c, w)
    }
    pub fn fail(
        &mut self,
        c: &mut Container,
        id: EpisodeId,
        t: InsomniaLeaseToken,
        at: i64,
        retry: i64,
        reason: String,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.check_owner(c)?;
        if reason.trim().is_empty() || retry <= at {
            return Err(InsomniaError::InvalidField("retry failure"));
        }
        let mut w = self.active(id, t, at)?;
        w.state = InsomniaWorkState::Failed;
        clear_lease(&mut w);
        w.retry_after_ns = Some(retry);
        w.last_error = Some(reason);
        w.updated_at_ns = at;
        self.persist_changed(c, w)
    }
    pub fn terminal(
        &mut self,
        c: &mut Container,
        id: EpisodeId,
        t: InsomniaLeaseToken,
        at: i64,
        reason: String,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.check_owner(c)?;
        if reason.trim().is_empty() {
            return Err(InsomniaError::InvalidField("terminal reason"));
        }
        let mut w = self.active(id, t, at)?;
        w.state = InsomniaWorkState::Terminal;
        clear_lease(&mut w);
        w.retry_after_ns = None;
        w.last_error = Some(reason);
        w.updated_at_ns = at;
        self.persist_changed(c, w)
    }
    /// Persist work that cannot safely continue automatically (for example,
    /// unsupported evidence routing or an uncertain external outcome).
    pub fn hold(
        &mut self,
        c: &mut Container,
        id: EpisodeId,
        t: InsomniaLeaseToken,
        at: i64,
        reason: String,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.hold_with_payload(c, id, t, at, reason, None)
    }
    pub fn hold_with_payload(
        &mut self,
        c: &mut Container,
        id: EpisodeId,
        t: InsomniaLeaseToken,
        at: i64,
        reason: String,
        payload: Option<Vec<u8>>,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.check_owner(c)?;
        let work = self.active(id, t, at)?;
        self.hold_work(c, work, at, reason, payload)
    }

    /// Keep an unreconciled outcome held even if this exact attempt expired.
    /// A superseded claim cannot change the currently projected work.
    pub fn hold_unreconciled_attempt(
        &mut self,
        c: &mut Container,
        id: EpisodeId,
        token: InsomniaLeaseToken,
        at: i64,
        reason: String,
        payload: Option<Vec<u8>>,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.check_owner(c)?;
        let work = self
            .work
            .get(&id)
            .filter(|work| {
                work.state == InsomniaWorkState::Processing
                    && work.lease_token == Some(token)
                    && at >= work.updated_at_ns
            })
            .cloned()
            .ok_or(InsomniaError::InvalidLease)?;
        self.hold_work(c, work, at, reason, payload)
    }

    fn hold_work(
        &mut self,
        c: &mut Container,
        mut w: InsomniaWork,
        at: i64,
        reason: String,
        payload: Option<Vec<u8>>,
    ) -> Result<InsomniaWork, InsomniaError> {
        if reason.trim().is_empty()
            || payload
                .as_ref()
                .is_some_and(|p| p.len() > MAX_HELD_PAYLOAD_BYTES)
        {
            return Err(InsomniaError::InvalidField("held reason"));
        }
        w.state = InsomniaWorkState::Held;
        clear_lease(&mut w);
        w.retry_after_ns = None;
        w.last_error = Some(reason);
        w.held_payload = payload;
        w.updated_at_ns = at;
        self.persist_changed(c, w)
    }
    /// Explicit operator action; held work is never returned to scheduling by
    /// a worker restart or automatic lease sweep.
    pub fn retry_held(
        &mut self,
        c: &mut Container,
        id: EpisodeId,
        now: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.check_owner(c)?;
        let mut w = self
            .work
            .get(&id)
            .cloned()
            .ok_or(InsomniaError::MissingWork)?;
        if w.state != InsomniaWorkState::Held {
            return Err(InsomniaError::InvalidTransition);
        }
        w.state = InsomniaWorkState::Pending;
        clear_lease(&mut w);
        w.retry_after_ns = None;
        w.last_error = None;
        w.updated_at_ns = now;
        self.persist_changed(c, w)
    }
    pub fn retry_terminal(
        &mut self,
        c: &mut Container,
        id: EpisodeId,
        now: i64,
    ) -> Result<InsomniaWork, InsomniaError> {
        self.check_owner(c)?;
        let mut w = self
            .work
            .get(&id)
            .cloned()
            .ok_or(InsomniaError::MissingWork)?;
        if w.state != InsomniaWorkState::Terminal {
            return Err(InsomniaError::InvalidTransition);
        }
        w.state = InsomniaWorkState::Pending;
        w.attempt_count = 0;
        clear_lease(&mut w);
        w.retry_after_ns = None;
        w.last_error = None;
        w.updated_at_ns = now;
        self.persist_changed(c, w)
    }
}
