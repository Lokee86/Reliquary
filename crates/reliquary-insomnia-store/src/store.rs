use reliquary_container::{Container, ContainerError};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;

const WORK_MAGIC: &[u8; 8] = b"RELINSW1";
const RECEIPT_MAGIC: &[u8; 8] = b"RELINSR1";
pub const MAX_RECEIPT_IDS: usize = 100_000;
pub const MAX_RECEIPT_TEXT_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EpisodeId(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryId(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct InsomniaLeaseToken(pub [u8; 32]);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum InsomniaPriority {
    ImmediateLive,
    Live,
    Import,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InsomniaWorkState {
    Pending,
    Processing,
    Complete,
    Failed,
    Terminal,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalMemoryRef {
    pub owner_id: String,
    pub memory_id: MemoryId,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedEpisodeOrder {
    pub episode_id: EpisodeId,
    pub source_through_ns: i64,
    pub conversation_id: String,
    pub start_node_id: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsomniaWork {
    pub episode_id: EpisodeId,
    pub priority: InsomniaPriority,
    pub state: InsomniaWorkState,
    pub attempt_count: u32,
    pub lease_owner: Option<String>,
    pub lease_token: Option<InsomniaLeaseToken>,
    pub lease_expires_ns: Option<i64>,
    pub retry_after_ns: Option<i64>,
    pub last_error: Option<String>,
    pub updated_at_ns: i64,
    pub revision: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedCompletionState {
    pub episode_id: EpisodeId,
    pub attempt: u32,
    pub started_at_ns: i64,
    pub completed_at_ns: i64,
    pub extractor_model: String,
    pub extractor_version: String,
    pub memory_ids: Vec<MemoryId>,
    pub external_memory_refs: Vec<ExternalMemoryRef>,
    pub rejected_count: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionCheck {
    Apply,
    AlreadyAccepted,
}

#[derive(Debug)]
pub enum InsomniaError {
    Container(ContainerError),
    WrongOwner,
    UnacceptedEpisode,
    CorruptRecord(&'static str),
    InvalidField(&'static str),
    MissingWork,
    InvalidLease,
    LeaseExpired,
    InvalidTransition,
    ReceiptConflict,
    VersionExhausted,
}
impl fmt::Display for InsomniaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(e) => e.fmt(f),
            Self::WrongOwner => f.write_str("Insomnia record belongs to another owner"),
            Self::UnacceptedEpisode => f.write_str("Episode was not admitted by the owner"),
            Self::CorruptRecord(s) => write!(f, "corrupt Insomnia record: {s}"),
            Self::InvalidField(s) => write!(f, "invalid Insomnia field: {s}"),
            Self::MissingWork => f.write_str("Insomnia work not found"),
            Self::InvalidLease => f.write_str("Insomnia lease is invalid"),
            Self::LeaseExpired => f.write_str("Insomnia lease expired"),
            Self::InvalidTransition => f.write_str("invalid Insomnia work transition"),
            Self::ReceiptConflict => {
                f.write_str("completion receipt conflicts with accepted evidence")
            }
            Self::VersionExhausted => f.write_str("Insomnia version exhausted"),
        }
    }
}
impl std::error::Error for InsomniaError {}
impl From<ContainerError> for InsomniaError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}

pub struct InsomniaStore {
    owner: [u8; 16],
    accepted: BTreeMap<EpisodeId, AcceptedEpisodeOrder>,
    work: BTreeMap<EpisodeId, InsomniaWork>,
    orders: BTreeMap<EpisodeId, AcceptedEpisodeOrder>,
    receipts: BTreeMap<(EpisodeId, u32), AcceptedCompletionState>,
}

impl InsomniaStore {
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
            accepted: orders,
            work: BTreeMap::new(),
            orders: BTreeMap::new(),
            receipts: BTreeMap::new(),
        };
        for frame in container.read_frames()? {
            if frame.payload.starts_with(WORK_MAGIC) {
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
    pub fn work(&self, id: EpisodeId) -> Option<&InsomniaWork> {
        self.work.get(&id)
    }
    pub fn receipt(&self, id: EpisodeId, attempt: u32) -> Option<&AcceptedCompletionState> {
        self.receipts.get(&(id, attempt))
    }
    pub fn work_items(&self) -> impl Iterator<Item = &InsomniaWork> {
        self.work.values()
    }
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

    fn clone_for_preflight(&self) -> Self {
        Self {
            owner: self.owner,
            accepted: self.accepted.clone(),
            work: self.work.clone(),
            orders: self.orders.clone(),
            receipts: self.receipts.clone(),
        }
    }
    fn persist_changed(
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
    fn persist_work(
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
    fn apply_work(
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
        self.orders.insert(w.episode_id, order);
        let id = w.episode_id;
        self.work.insert(id, w);
        self.project_receipt(id);
        Ok(())
    }
    fn apply_receipt(&mut self, r: AcceptedCompletionState) -> Result<(), InsomniaError> {
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
    fn project_receipt(&mut self, id: EpisodeId) {
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
                w.updated_at_ns = receipt.completed_at_ns;
            }
        }
    }
    fn accept_order(&mut self, o: &AcceptedEpisodeOrder) -> Result<(), InsomniaError> {
        if self.accepted.get(&o.episode_id) != Some(o) {
            return Err(InsomniaError::UnacceptedEpisode);
        }
        Ok(())
    }
    fn accept_episode(&self, id: EpisodeId) -> Result<(), InsomniaError> {
        if self.accepted.contains_key(&id) {
            Ok(())
        } else {
            Err(InsomniaError::UnacceptedEpisode)
        }
    }
    fn active(
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
    fn check_owner(&self, c: &Container) -> Result<(), InsomniaError> {
        if c.owner_id() == self.owner {
            Ok(())
        } else {
            Err(InsomniaError::WrongOwner)
        }
    }
}

fn clear_lease(w: &mut InsomniaWork) {
    w.lease_owner = None;
    w.lease_token = None;
    w.lease_expires_ns = None
}
fn validate_order(o: &AcceptedEpisodeOrder) -> Result<(), InsomniaError> {
    if o.conversation_id.trim().is_empty() || o.start_node_id.trim().is_empty() {
        Err(InsomniaError::InvalidField("accepted Episode order"))
    } else {
        Ok(())
    }
}
fn validate_work(w: &InsomniaWork) -> Result<(), InsomniaError> {
    let leased = w.lease_owner.is_some() && w.lease_token.is_some() && w.lease_expires_ns.is_some();
    if (w.state == InsomniaWorkState::Processing) != leased || w.revision == 0 {
        return Err(InsomniaError::CorruptRecord("work lease invariants"));
    }
    Ok(())
}
fn validate_receipt(r: &AcceptedCompletionState) -> Result<(), InsomniaError> {
    if r.attempt == 0
        || r.completed_at_ns < r.started_at_ns
        || r.extractor_model.trim().is_empty()
        || r.extractor_version.trim().is_empty()
        || r.memory_ids.len() > MAX_RECEIPT_IDS
        || r.external_memory_refs.len() > MAX_RECEIPT_IDS
        || r.extractor_model.len() > MAX_RECEIPT_TEXT_BYTES
        || r.extractor_version.len() > MAX_RECEIPT_TEXT_BYTES
    {
        return Err(InsomniaError::InvalidField("completion receipt"));
    }
    for r in &r.external_memory_refs {
        if r.owner_id.trim().is_empty() || r.owner_id.len() > MAX_RECEIPT_TEXT_BYTES {
            return Err(InsomniaError::InvalidField("external memory reference"));
        }
    }
    Ok(())
}
fn token(id: EpisodeId, worker: &str, attempt: u32, now: i64) -> InsomniaLeaseToken {
    let mut h = Sha256::new();
    h.update(b"continuity-insomnia-lease\0");
    h.update(id.0);
    h.update((worker.len() as u64).to_le_bytes());
    h.update(worker.as_bytes());
    h.update(attempt.to_le_bytes());
    h.update(now.to_le_bytes());
    InsomniaLeaseToken(h.finalize().into())
}

fn encode_work(
    owner: [u8; 16],
    o: &AcceptedEpisodeOrder,
    w: &InsomniaWork,
) -> Result<Vec<u8>, InsomniaError> {
    validate_work(w)?;
    let mut b = WORK_MAGIC.to_vec();
    b.extend_from_slice(&owner);
    b.extend_from_slice(&w.revision.to_le_bytes());
    b.extend_from_slice(&w.episode_id.0);
    b.extend_from_slice(&o.source_through_ns.to_le_bytes());
    put_str(&mut b, &o.conversation_id)?;
    put_str(&mut b, &o.start_node_id)?;
    b.push(w.priority as u8 + 1);
    b.push(w.state as u8 + 1);
    b.extend_from_slice(&w.attempt_count.to_le_bytes());
    b.extend_from_slice(&w.updated_at_ns.to_le_bytes());
    put_opt_str(&mut b, w.lease_owner.as_deref())?;
    put_opt_bytes(&mut b, w.lease_token.map(|t| t.0).as_ref());
    put_opt_i64(&mut b, w.lease_expires_ns);
    put_opt_i64(&mut b, w.retry_after_ns);
    put_opt_str(&mut b, w.last_error.as_deref())?;
    Ok(b)
}
fn decode_work(b: &[u8]) -> Result<([u8; 16], AcceptedEpisodeOrder, InsomniaWork), InsomniaError> {
    let mut p = 8;
    let owner = take::<16>(b, &mut p)?;
    let revision = u64::from_le_bytes(take::<8>(b, &mut p)?);
    let episode_id = EpisodeId(take::<32>(b, &mut p)?);
    let source_through_ns = i64::from_le_bytes(take::<8>(b, &mut p)?);
    let conversation_id = get_str(b, &mut p)?;
    let start_node_id = get_str(b, &mut p)?;
    let priority = match byte(b, &mut p)? {
        1 => InsomniaPriority::ImmediateLive,
        2 => InsomniaPriority::Live,
        3 => InsomniaPriority::Import,
        _ => return Err(InsomniaError::CorruptRecord("priority")),
    };
    let state = match byte(b, &mut p)? {
        1 => InsomniaWorkState::Pending,
        2 => InsomniaWorkState::Processing,
        3 => InsomniaWorkState::Complete,
        4 => InsomniaWorkState::Failed,
        5 => InsomniaWorkState::Terminal,
        _ => return Err(InsomniaError::CorruptRecord("state")),
    };
    let attempt_count = u32::from_le_bytes(take::<4>(b, &mut p)?);
    let updated_at_ns = i64::from_le_bytes(take::<8>(b, &mut p)?);
    let lease_owner = get_opt_str(b, &mut p)?;
    let lease_token = get_opt_array32(b, &mut p)?.map(InsomniaLeaseToken);
    let lease_expires_ns = get_opt_i64(b, &mut p)?;
    let retry_after_ns = get_opt_i64(b, &mut p)?;
    let last_error = get_opt_str(b, &mut p)?;
    end(b, p)?;
    let order = AcceptedEpisodeOrder {
        episode_id,
        source_through_ns,
        conversation_id,
        start_node_id,
    };
    let work = InsomniaWork {
        episode_id,
        priority,
        state,
        attempt_count,
        lease_owner,
        lease_token,
        lease_expires_ns,
        retry_after_ns,
        last_error,
        updated_at_ns,
        revision,
    };
    Ok((owner, order, work))
}
fn encode_receipt(owner: [u8; 16], r: &AcceptedCompletionState) -> Result<Vec<u8>, InsomniaError> {
    validate_receipt(r)?;
    let mut b = RECEIPT_MAGIC.to_vec();
    b.extend_from_slice(&owner);
    b.extend_from_slice(&r.episode_id.0);
    b.extend_from_slice(&r.attempt.to_le_bytes());
    b.extend_from_slice(&r.started_at_ns.to_le_bytes());
    b.extend_from_slice(&r.completed_at_ns.to_le_bytes());
    put_str(&mut b, &r.extractor_model)?;
    put_str(&mut b, &r.extractor_version)?;
    b.extend_from_slice(&r.rejected_count.to_le_bytes());
    put_ids(
        &mut b,
        &r.memory_ids.iter().map(|id| id.0).collect::<Vec<_>>(),
    )?;
    b.extend_from_slice(
        &u32::try_from(r.external_memory_refs.len())
            .map_err(|_| InsomniaError::InvalidField("external refs"))?
            .to_le_bytes(),
    );
    for x in &r.external_memory_refs {
        put_str(&mut b, &x.owner_id)?;
        b.extend_from_slice(&x.memory_id.0)
    }
    Ok(b)
}
fn decode_receipt(b: &[u8]) -> Result<([u8; 16], AcceptedCompletionState), InsomniaError> {
    let mut p = 8;
    let owner = take::<16>(b, &mut p)?;
    let episode_id = EpisodeId(take::<32>(b, &mut p)?);
    let attempt = u32::from_le_bytes(take::<4>(b, &mut p)?);
    let started_at_ns = i64::from_le_bytes(take::<8>(b, &mut p)?);
    let completed_at_ns = i64::from_le_bytes(take::<8>(b, &mut p)?);
    let extractor_model = get_str(b, &mut p)?;
    let extractor_version = get_str(b, &mut p)?;
    let rejected_count = u32::from_le_bytes(take::<4>(b, &mut p)?);
    let memory_ids = get_ids(b, &mut p)?.into_iter().map(MemoryId).collect();
    let n = u32::from_le_bytes(take::<4>(b, &mut p)?) as usize;
    if n > MAX_RECEIPT_IDS {
        return Err(InsomniaError::CorruptRecord("too many external refs"));
    }
    let mut external_memory_refs = Vec::with_capacity(n);
    for _ in 0..n {
        external_memory_refs.push(ExternalMemoryRef {
            owner_id: get_str(b, &mut p)?,
            memory_id: MemoryId(take::<32>(b, &mut p)?),
        })
    }
    end(b, p)?;
    let r = AcceptedCompletionState {
        episode_id,
        attempt,
        started_at_ns,
        completed_at_ns,
        extractor_model,
        extractor_version,
        memory_ids,
        external_memory_refs,
        rejected_count,
    };
    validate_receipt(&r)?;
    Ok((owner, r))
}
fn put_str(b: &mut Vec<u8>, s: &str) -> Result<(), InsomniaError> {
    let n = u32::try_from(s.len()).map_err(|_| InsomniaError::InvalidField("string length"))?;
    b.extend_from_slice(&n.to_le_bytes());
    b.extend_from_slice(s.as_bytes());
    Ok(())
}
fn put_opt_str(b: &mut Vec<u8>, s: Option<&str>) -> Result<(), InsomniaError> {
    match s {
        None => b.push(0),
        Some(s) => {
            b.push(1);
            put_str(b, s)?
        }
    }
    Ok(())
}
fn put_opt_bytes(b: &mut Vec<u8>, v: Option<&[u8; 32]>) {
    match v {
        None => b.push(0),
        Some(v) => {
            b.push(1);
            b.extend_from_slice(v)
        }
    }
}
fn put_opt_i64(b: &mut Vec<u8>, v: Option<i64>) {
    match v {
        None => b.push(0),
        Some(v) => {
            b.push(1);
            b.extend_from_slice(&v.to_le_bytes())
        }
    }
}
fn put_ids(b: &mut Vec<u8>, v: &[Id]) -> Result<(), InsomniaError> {
    b.extend_from_slice(
        &u32::try_from(v.len())
            .map_err(|_| InsomniaError::InvalidField("memory IDs"))?
            .to_le_bytes(),
    );
    for x in v {
        b.extend_from_slice(x)
    }
    Ok(())
}
type Id = [u8; 32];
fn get_ids(b: &[u8], p: &mut usize) -> Result<Vec<Id>, InsomniaError> {
    let n = u32::from_le_bytes(take::<4>(b, p)?) as usize;
    if n > MAX_RECEIPT_IDS || n > b.len().saturating_sub(*p) / 32 {
        return Err(InsomniaError::CorruptRecord("memory ID count"));
    }
    (0..n).map(|_| take::<32>(b, p)).collect()
}
fn get_str(b: &[u8], p: &mut usize) -> Result<String, InsomniaError> {
    let n = u32::from_le_bytes(take::<4>(b, p)?) as usize;
    if n > MAX_RECEIPT_TEXT_BYTES {
        return Err(InsomniaError::CorruptRecord("text too large"));
    }
    String::from_utf8(take_vec(b, p, n)?).map_err(|_| InsomniaError::CorruptRecord("invalid UTF-8"))
}
fn get_opt_str(b: &[u8], p: &mut usize) -> Result<Option<String>, InsomniaError> {
    match byte(b, p)? {
        0 => Ok(None),
        1 => Ok(Some(get_str(b, p)?)),
        _ => Err(InsomniaError::CorruptRecord("optional string tag")),
    }
}
fn get_opt_i64(b: &[u8], p: &mut usize) -> Result<Option<i64>, InsomniaError> {
    match byte(b, p)? {
        0 => Ok(None),
        1 => Ok(Some(i64::from_le_bytes(take::<8>(b, p)?))),
        _ => Err(InsomniaError::CorruptRecord("optional integer tag")),
    }
}
fn get_opt_array32(b: &[u8], p: &mut usize) -> Result<Option<[u8; 32]>, InsomniaError> {
    match byte(b, p)? {
        0 => Ok(None),
        1 => Ok(Some(take::<32>(b, p)?)),
        _ => Err(InsomniaError::CorruptRecord("optional token tag")),
    }
}
fn byte(b: &[u8], p: &mut usize) -> Result<u8, InsomniaError> {
    Ok(take::<1>(b, p)?[0])
}
fn take<const N: usize>(b: &[u8], p: &mut usize) -> Result<[u8; N], InsomniaError> {
    let end = p
        .checked_add(N)
        .ok_or(InsomniaError::CorruptRecord("length overflow"))?;
    let s = b
        .get(*p..end)
        .ok_or(InsomniaError::CorruptRecord("truncated record"))?;
    *p = end;
    Ok(s.try_into().unwrap())
}
fn take_vec(b: &[u8], p: &mut usize, n: usize) -> Result<Vec<u8>, InsomniaError> {
    let end = p
        .checked_add(n)
        .ok_or(InsomniaError::CorruptRecord("length overflow"))?;
    let v = b
        .get(*p..end)
        .ok_or(InsomniaError::CorruptRecord("truncated bytes"))?
        .to_vec();
    *p = end;
    Ok(v)
}
fn end(b: &[u8], p: usize) -> Result<(), InsomniaError> {
    if p == b.len() {
        Ok(())
    } else {
        Err(InsomniaError::CorruptRecord("trailing bytes"))
    }
}
