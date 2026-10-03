use crate::freshness::{FreshnessPolicy, FreshnessRecord};
use crate::{FreshnessEventProof, MemoryId};
use std::collections::BTreeMap;

const MAGIC: &[u8; 8] = b"CVAFRS02";
const POLICY_MAGIC: &[u8; 8] = b"CVAFRP01";

#[cfg(test)]
#[path = "freshness_storage_tests.rs"]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FreshnessEventProvenance {
    pub(crate) producer_kind: u8,
    pub(crate) source_id: String,
    pub(crate) origin_owner: [u8; 16],
    pub(crate) accepted_turn: u64,
    pub(crate) policy_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum JournalEntry {
    Initialize {
        memory_id: MemoryId,
        turn: u64,
        record: FreshnessRecord,
    },
    Admit {
        memory_id: MemoryId,
        record: FreshnessRecord,
    },
    Event {
        event_id: String,
        turn: u64,
        provenance: FreshnessEventProvenance,
        proof: FreshnessEventProof,
        effects: BTreeMap<MemoryId, i16>,
        records: BTreeMap<MemoryId, FreshnessRecord>,
    },
    AcceptedUseReceipt {
        use_id: String,
        turn: u64,
        sources: Vec<MemoryId>,
        policy_version: u64,
    },
    AcceptedUseBatch {
        use_id: String,
        turn: u64,
        sources: Vec<MemoryId>,
        policy_version: u64,
        events: Vec<JournalEntry>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FreshnessReplayEntry {
    Initialize {
        memory_id: MemoryId,
        source_turn: u64,
        admitted_at_birth: bool,
    },
    Admit {
        memory_id: MemoryId,
        source_turn: u64,
    },
    Event {
        event_id: String,
        source_turn: u64,
        effects: BTreeMap<MemoryId, i16>,
        proof: FreshnessEventProof,
        provenance: Option<FreshnessEventProvenance>,
    },
    AcceptedUseReceipt {
        use_id: String,
        source_turn: u64,
        sources: Vec<MemoryId>,
        policy_version: u64,
    },
}

fn hex_id(id: &MemoryId) -> String {
    id.0.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl FreshnessReplayEntry {
    pub(crate) fn source_turn(&self) -> u64 {
        match self {
            Self::Initialize { source_turn, .. }
            | Self::Admit { source_turn, .. }
            | Self::Event { source_turn, .. }
            | Self::AcceptedUseReceipt { source_turn, .. } => *source_turn,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FreshnessStore {
    owner_uuid: Option<[u8; 16]>,
    policy: FreshnessPolicy,
    policy_persisted: bool,
    // Derived birth proofs can populate the in-memory projection on ordinary
    // reopen without having written a standalone owner policy chunk yet.
    durable_policy_seen: bool,
    records: BTreeMap<MemoryId, FreshnessRecord>,
    births: BTreeMap<MemoryId, (u64, FreshnessRecord)>,
    admissions: BTreeMap<MemoryId, FreshnessRecord>,
    recent: std::collections::VecDeque<(JournalEntry, usize)>,
    recent_bytes: usize,
    history_path: Option<std::path::PathBuf>,
    history_limit: u64,
    latest_event: Option<(u64, String)>,
    rebuild_receipts: Option<std::sync::Arc<rebuild::RebuildReceipts>>,
}

const RECENT_LIMIT: usize = 128;
const RECENT_BYTES: usize = 256 * 1024;
#[path = "freshness/freshness_storage_ordering.rs"]
mod ordering;
#[path = "freshness/freshness_storage_rebuild.rs"]
mod rebuild;
#[path = "freshness/freshness_storage_stream.rs"]
mod stream;

impl FreshnessStore {
    pub(crate) fn new(owner_uuid: Option<[u8; 16]>) -> Self {
        Self {
            owner_uuid,
            policy: FreshnessPolicy::default_v1(),
            ..Self::default()
        }
    }
    pub(crate) fn bind_history_path(&mut self, path: impl AsRef<std::path::Path>, limit: u64) {
        self.history_path = Some(path.as_ref().to_path_buf());
        self.history_limit = limit;
    }
    pub(crate) fn begin_rebuild(&mut self) {
        self.rebuild_receipts = Some(std::sync::Arc::new(rebuild::RebuildReceipts::new()));
    }
    pub(crate) fn finish_rebuild(&mut self) -> Result<(), String> {
        self.records = self.canonical_projection(u64::MAX, None, u64::MAX, &[])?;
        self.rebuild_receipts = None;
        Ok(())
    }
    pub(crate) fn policy(&self) -> FreshnessPolicy {
        self.policy
    }
    pub(crate) fn freeze_derived_birth_policy(&mut self) {
        self.policy_persisted = true;
    }
    pub(crate) fn set_policy_before_enrollment(
        &mut self,
        policy: FreshnessPolicy,
    ) -> Result<(), String> {
        policy.validate().map_err(|e| e.to_string())?;
        if self.policy_persisted || !self.records.is_empty() || !self.recent.is_empty() {
            return if self.policy == policy {
                Ok(())
            } else {
                Err("Freshness policy is immutable after owner enrollment".into())
            };
        }
        self.policy = policy;
        Ok(())
    }
    pub(crate) fn persist_policy_if_needed(
        &mut self,
        container: &mut crate::Container,
        owner: [u8; 16],
    ) -> Result<(), String> {
        if self.durable_policy_seen {
            return Ok(());
        }
        let payload = encode_policy(owner, self.policy);
        container.append(&payload).map_err(|e| e.to_string())?;
        container.sync().map_err(|e| e.to_string())?;
        self.ingest_appended(&payload)
    }
    pub(crate) fn ingest_appended(&mut self, payload: &[u8]) -> Result<(), String> {
        let path = self
            .history_path
            .as_ref()
            .ok_or("Freshness durable history is not bound")?;
        let end = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
        let before = end
            .checked_sub(8 + payload.len() as u64)
            .ok_or("Freshness appended payload has invalid extent")?;
        let previous = self.history_limit;
        self.history_limit = before;
        match self.ingest(payload) {
            Ok(()) => {
                self.history_limit = end;
                if payload.starts_with(POLICY_MAGIC) {
                    self.durable_policy_seen = true;
                }
                Ok(())
            }
            Err(e) => {
                self.history_limit = previous;
                Err(e)
            }
        }
    }
    pub(crate) fn ingest_scanned(&mut self, payload: &[u8], end: u64) -> Result<(), String> {
        self.ingest(payload)?;
        self.history_limit = end;
        if payload.starts_with(POLICY_MAGIC) {
            self.durable_policy_seen = true;
        }
        Ok(())
    }

    /// A derived birth is intentionally visible after read-only reopen, but it
    /// cannot authorize consuming its publication intent until its own durable
    /// initialization has been written. Scan the owner journal only on recovery.
    pub(crate) fn has_durable_birth(&self, id: MemoryId) -> Result<bool, String> {
        let mut found = false;
        self.scan_entries(|entry| {
            if let JournalEntry::Initialize { memory_id, .. } = entry {
                found |= memory_id == id;
            }
            Ok(())
        })?;
        Ok(found)
    }

    fn scan_entries(
        &self,
        mut visitor: impl FnMut(JournalEntry) -> Result<(), String>,
    ) -> Result<(), String> {
        if let Some(path) = &self.history_path {
            let expected = self.owner_uuid;
            stream::scan_entries(path, self.history_limit, |owner, entry| {
                if expected.is_some_and(|id| id != owner) {
                    return Err("Freshness durable scan owner mismatch".into());
                }
                visitor(entry)
            })
        } else {
            // Standalone codec tests have no durable owner. Production stores are
            // always bound to their Container before publication or replay.
            for (entry, _) in &self.recent {
                visitor(entry.clone())?;
            }
            Ok(())
        }
    }
    fn find_event(&self, id: &str) -> Result<Option<JournalEntry>, String> {
        for (entry, _) in self.recent.iter().rev() {
            if let Some(event) = event_in(entry, id) {
                return Ok(Some(event.clone()));
            }
        }
        if let Some(index) = &self.rebuild_receipts {
            return index.find("event", id);
        }
        let mut found = None;
        self.scan_entries(|entry| {
            if let Some(event) = event_in(&entry, id) {
                if found
                    .as_ref()
                    .is_some_and(|old| !event_receipts_equal(old, event))
                {
                    return Err("conflicting durable Freshness event identities".into());
                }
                found = Some(event.clone());
            }
            Ok(())
        })?;
        Ok(found)
    }
    fn find_use(&self, id: &str) -> Result<Option<JournalEntry>, String> {
        for (entry, _) in self.recent.iter().rev() {
            if use_fields(entry).is_some_and(|(key, _, _, _)| key == id) {
                return Ok(Some(entry.clone()));
            }
        }
        if let Some(index) = &self.rebuild_receipts {
            return index.find("use", id);
        }
        let mut found = None;
        self.scan_entries(|entry| {
            if use_fields(&entry).is_some_and(|(key, _, _, _)| key == id) {
                if found
                    .as_ref()
                    .is_some_and(|old| !use_receipts_equal(old, &entry))
                {
                    return Err("conflicting durable accepted-use identities".into());
                }
                found = Some(entry);
            }
            Ok(())
        })?;
        Ok(found)
    }
    fn remember(&mut self, entry: JournalEntry) -> Result<(), String> {
        if let Some(index) = &self.rebuild_receipts {
            index.remember(&entry)?;
        }
        let bytes = receipt_bytes(&entry);
        if bytes > RECENT_BYTES {
            return Ok(());
        }
        while self.recent.len() >= RECENT_LIMIT
            || self.recent_bytes.saturating_add(bytes) > RECENT_BYTES
        {
            let Some((_, old)) = self.recent.pop_front() else {
                break;
            };
            self.recent_bytes -= old;
        }
        self.recent_bytes += bytes;
        self.recent.push_back((entry, bytes));
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn recent_receipt_stats(&self) -> (usize, usize) {
        (self.recent.len(), self.recent_bytes)
    }

    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), String> {
        if payload.get(..8) == Some(POLICY_MAGIC.as_slice()) {
            let (owner, policy) = decode_policy(payload)?;
            policy.validate().map_err(|e| e.to_string())?;
            if self.owner_uuid.is_some_and(|id| id != owner) {
                return Err("Freshness policy belongs to another durable owner".into());
            }
            if self.policy_persisted && self.policy != policy {
                return Err("Freshness owner policy changed within one history".into());
            }
            self.owner_uuid.get_or_insert(owner);
            self.policy = policy;
            self.policy_persisted = true;
            return Ok(());
        }
        if payload.starts_with(b"CVAFRS") && payload.get(..8) != Some(MAGIC.as_slice()) {
            return Err(
                "unsupported Freshness prototype format; explicit migration is required".into(),
            );
        }
        if payload.get(..8) != Some(MAGIC.as_slice()) {
            return Ok(());
        }
        let (owner, entry) = decode(payload)?;
        if self.owner_uuid.is_some_and(|id| id != owner) {
            return Err("Freshness metadata belongs to another durable owner".into());
        }
        if !self.policy_persisted {
            return Err("Freshness record appeared before its pinned owner policy".into());
        }
        // Every branch validates before modifying any projection or cache.
        match &entry {
            JournalEntry::Initialize {
                memory_id,
                turn,
                record,
            } => {
                if record.score != 100
                    || record.admitted_at_turn.is_some_and(|at| at != *turn)
                    || record.accounted_turn != record.admitted_at_turn
                {
                    return Err("invalid Freshness initialization record".into());
                }
                if let Some(old) = self.births.get(memory_id) {
                    return if old == &(*turn, *record) {
                        Ok(())
                    } else {
                        Err("Freshness initialization conflicts with its accepted birth".into())
                    };
                }
                self.births.insert(*memory_id, (*turn, *record));
                self.records.insert(*memory_id, *record);
            }
            JournalEntry::Admit { memory_id, record } => {
                let at = record
                    .admitted_at_turn
                    .ok_or("invalid Freshness admission record")?;
                if record.accounted_turn != Some(at) {
                    return Err("invalid Freshness admission accounting anchor".into());
                }
                let (birth, initial) = self
                    .births
                    .get(memory_id)
                    .ok_or("Freshness admission has no accepted birth")?;
                if at < *birth || record.score != initial.score {
                    return Err("Freshness admission changed birth score or precedes birth".into());
                }
                if let Some(old) = self.admissions.get(memory_id) {
                    return if old == record {
                        Ok(())
                    } else {
                        Err("Freshness admission cannot be reanchored".into())
                    };
                }
                if let Some(born_at) = initial.admitted_at_turn {
                    return if born_at == at {
                        Ok(())
                    } else {
                        Err("Freshness admission cannot be reanchored".into())
                    };
                }
                let mut staged = self.clone();
                staged.admissions.insert(*memory_id, *record);
                if staged.rebuild_receipts.is_none() {
                    staged.records = staged.canonical_projection(u64::MAX, None, u64::MAX, &[])?;
                }
                *self = staged;
            }
            JournalEntry::Event {
                event_id,
                turn,
                effects,
                records,
                ..
            } => {
                if let Some(old) = self.find_event(event_id)? {
                    return if event_receipts_equal(&old, &entry) {
                        Ok(())
                    } else {
                        Err("Freshness event identity collides with different payload".into())
                    };
                }
                self.validate_event_entry(owner, &entry)?;
                let canonical = if self.rebuild_receipts.is_some() && records.is_empty() {
                    BTreeMap::new()
                } else {
                    self.prepare_event_postimages_for(event_id, *turn, effects)?
                };
                // The compact journal persists immutable candidate effects and
                // recomputes derived postimages on replay. An explicitly supplied
                // in-memory postimage must still match before projection changes.
                if !records.is_empty() && &canonical != records {
                    return Err(
                        "Freshness durable event postimages differ from canonical effects".into(),
                    );
                }
                let projected = self.project_with_events(std::slice::from_ref(&entry))?;
                self.records = projected;
                self.latest_event = Some(self.latest_event.clone().map_or_else(
                    || event_sort_key(&entry),
                    |old| old.max(event_sort_key(&entry)),
                ));
                self.remember(entry)?;
            }
            JournalEntry::AcceptedUseBatch {
                use_id,
                turn,
                sources,
                policy_version,
                events,
            } => {
                self.validate_use_identity(use_id, sources, *policy_version)?;
                if let Some(old) = self.find_use(use_id)? {
                    return if use_receipts_equal(&old, &entry) {
                        Ok(())
                    } else {
                        Err("accepted-use batch payload conflicts with its receipt".into())
                    };
                }
                if events.len() != sources.len() {
                    return Err("accepted-use source/event count mismatch".into());
                }
                for (source, event) in sources.iter().zip(events) {
                    let JournalEntry::Event {
                        event_id,
                        turn: event_turn,
                        provenance,
                        effects,
                        ..
                    } = event
                    else {
                        return Err("accepted-use batch contains a non-event".into());
                    };
                    if *event_turn != *turn
                        || *event_id != accepted_event_id(owner, use_id, *source)
                        || effects.get(source) != Some(&self.policy.access_principal)
                        || effects.values().any(|v| *v > self.policy.access_principal)
                        || provenance.producer_kind != 2
                        || provenance.source_id != *use_id
                        || provenance.accepted_turn != *turn
                        || provenance.policy_version != *policy_version
                    {
                        return Err("accepted-use batch has incompatible source provenance".into());
                    }
                    if self.find_event(event_id)?.is_some() {
                        return Err(
                            "accepted-use source event exists without its batch receipt".into()
                        );
                    }
                    self.validate_event_entry(owner, event)?;
                }
                let projected = self.project_with_events(events)?;
                self.records = projected;
                for event in events {
                    let key = event_sort_key(event);
                    self.latest_event = Some(
                        self.latest_event
                            .clone()
                            .map_or(key.clone(), |old| old.max(key)),
                    );
                }
                self.remember(entry)?;
            }
            JournalEntry::AcceptedUseReceipt {
                use_id,
                turn,
                sources,
                policy_version,
            } => {
                self.validate_use_identity(use_id, sources, *policy_version)?;
                if let Some(old) = self.find_use(use_id)? {
                    return if use_receipts_equal(&old, &entry) {
                        Ok(())
                    } else {
                        Err("accepted-use receipt conflicts with prior receipt".into())
                    };
                }
                for source in sources {
                    let event = self
                        .find_event(&accepted_event_id(owner, use_id, *source))?
                        .ok_or("replayed accepted-use receipt lacks constituent event")?;
                    let JournalEntry::Event {
                        turn: event_turn,
                        provenance,
                        ..
                    } = event
                    else {
                        unreachable!()
                    };
                    if event_turn != *turn
                        || provenance.producer_kind != 2
                        || provenance.source_id != *use_id
                        || provenance.origin_owner != owner
                        || provenance.policy_version != *policy_version
                    {
                        return Err(
                            "replayed accepted-use receipt differs from constituent event".into(),
                        );
                    }
                }
                self.remember(entry)?;
            }
        }
        self.owner_uuid.get_or_insert(owner);
        Ok(())
    }
    fn validate_event_entry(&self, owner: [u8; 16], event: &JournalEntry) -> Result<(), String> {
        let JournalEntry::Event {
            event_id,
            turn,
            provenance,
            proof,
            effects,
            ..
        } = event
        else {
            return Err("expected Freshness event".into());
        };
        if event_id.is_empty()
            || effects.is_empty()
            || provenance.source_id.trim().is_empty()
            || provenance.origin_owner != owner
            // The original accepted producer cut is immutable provenance, not
            // a bound on the destination event turn: reordered reconciliations
            // can move it in either direction. Local Dream/consumption producers
            // validate exact equality before committing a new source event.
            || !(1..=2).contains(&provenance.producer_kind)
            || provenance.policy_version != crate::freshness::FRESHNESS_POLICY_VERSION as u64
        {
            return Err("invalid Freshness event source provenance".into());
        }
        validate_proof(*proof)?;
        match provenance.producer_kind {
            1 => {
                // A Dream event is identified by the owner's exact initial pass,
                // never by a caller-supplied unrelated string.
                let expected_prefix = format!("dream-initial:{}:", hex_bytes(&owner));
                if provenance.source_id != *event_id
                    || !event_id.starts_with(&expected_prefix)
                    || event_id.len() != expected_prefix.len() + 64
                    || !event_id[expected_prefix.len()..]
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit())
                {
                    return Err("Freshness Dream event is not an owner-bound first-pass ID".into());
                }
                let suffix = &event_id[expected_prefix.len()..];
                let mut source = [0u8; 32];
                for (index, byte) in source.iter_mut().enumerate() {
                    *byte = u8::from_str_radix(&suffix[index * 2..index * 2 + 2], 16)
                        .map_err(|_| "invalid Dream source ID")?;
                }
                if !self
                    .births
                    .get(&MemoryId(source))
                    .is_some_and(|(birth, _)| birth <= turn)
                    || !effects
                        .values()
                        .any(|amount| *amount == self.policy.linkage_principal)
                {
                    return Err(
                        "Freshness Dream event lacks a born source or first-link principal".into(),
                    );
                }
            }
            2 => {
                // A standalone accepted-use event (during reconcile replay) must
                // still identify a genuine source root. The enclosing receipt
                // is checked at the owner boundary before replay.
                if !effects.iter().any(|(id, amount)| {
                    *amount == self.policy.access_principal
                        && accepted_event_id(owner, &provenance.source_id, *id) == *event_id
                }) || effects
                    .values()
                    .any(|amount| *amount > self.policy.access_principal)
                {
                    return Err("Freshness accepted-use event lacks its claimed source".into());
                }
            }
            _ => unreachable!(),
        }
        for (id, amount) in effects {
            if *amount <= 0 || *amount > self.policy.linkage_principal {
                return Err("Freshness event exceeds pinned principal bound".into());
            }
            if !self.births.get(id).is_some_and(|(birth, _)| birth <= turn) {
                return Err("Freshness event precedes accepted Memory birth".into());
            }
        }
        Ok(())
    }
    fn project_with_events(
        &self,
        events: &[JournalEntry],
    ) -> Result<BTreeMap<MemoryId, FreshnessRecord>, String> {
        // Validated receipts are replayed once after the accepted owner scan.
        if self.rebuild_receipts.is_some() {
            return Ok(self.records.clone());
        }
        let mut ordered = events.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|e| event_sort_key(e));
        if ordered.first().is_none_or(|e| {
            self.latest_event
                .as_ref()
                .is_none_or(|last| event_sort_key(e) > *last)
        }) && ordered.iter().all(|e| match e {
            JournalEntry::Event { turn, effects, .. } => effects.keys().all(|id| {
                self.records
                    .get(id)
                    .is_some_and(|record| record.accounted_turn.is_none_or(|at| at <= *turn))
            }),
            _ => false,
        }) {
            let mut records = self.records.clone();
            for event in ordered {
                let JournalEntry::Event { turn, effects, .. } = event else {
                    unreachable!()
                };
                for (id, amount) in effects {
                    records
                        .get_mut(id)
                        .ok_or("Freshness event references unenrolled Memory")?
                        .reinforce_with_policy(*amount, *turn, &self.policy)
                        .map_err(|e| e.to_string())?;
                }
            }
            return Ok(records);
        }
        self.canonical_projection(u64::MAX, None, u64::MAX, events)
    }
    fn canonical_projection(
        &self,
        through: u64,
        before: Option<(&str, bool)>,
        graph: u64,
        extra: &[JournalEntry],
    ) -> Result<BTreeMap<MemoryId, FreshnessRecord>, String> {
        let mut projected = self
            .births
            .iter()
            .filter(|(_, (turn, _))| *turn <= through)
            .map(|(id, (_, record))| (*id, *record))
            .collect::<BTreeMap<_, _>>();
        let mut admissions = self
            .admissions
            .iter()
            .filter_map(|(id, r)| {
                r.admitted_at_turn
                    .filter(|at| *at <= through)
                    .map(|at| (at, *id))
            })
            .collect::<Vec<_>>();
        admissions.sort();
        let mut admit_index = 0;
        ordering::ordered_events(self, through, before, graph, extra, |event| {
            let JournalEntry::Event { turn, effects, .. } = event else {
                unreachable!()
            };
            while admit_index < admissions.len() && admissions[admit_index].0 <= *turn {
                let (at, id) = admissions[admit_index];
                projected
                    .get_mut(&id)
                    .ok_or("Freshness admission precedes accepted birth")?
                    .admit(at)
                    .map_err(|e| e.to_string())?;
                admit_index += 1;
            }
            for (id, amount) in effects {
                if !self.births.get(id).is_some_and(|(birth, _)| birth <= turn) {
                    return Err("Freshness event precedes accepted Memory birth".into());
                }
                projected
                    .get_mut(id)
                    .ok_or("Freshness event references unenrolled Memory at cut")?
                    .reinforce_with_policy(*amount, *turn, &self.policy)
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        })?;
        for (at, id) in &admissions[admit_index..] {
            projected
                .get_mut(id)
                .ok_or("Freshness admission precedes accepted birth")?
                .admit(*at)
                .map_err(|e| e.to_string())?;
        }
        Ok(projected)
    }
    pub(crate) fn prepare_event_postimages_for(
        &self,
        id: &str,
        turn: u64,
        effects: &BTreeMap<MemoryId, i16>,
    ) -> Result<BTreeMap<MemoryId, FreshnessRecord>, String> {
        let in_order = self
            .latest_event
            .as_ref()
            .is_none_or(|last| (turn, id.to_owned()) > *last)
            && effects.keys().all(|memory| {
                self.births
                    .get(memory)
                    .is_some_and(|(birth, _)| *birth <= turn)
                    && self
                        .records
                        .get(memory)
                        .is_some_and(|r| r.accounted_turn.is_none_or(|at| at <= turn))
            });
        let before = if in_order {
            self.records.clone()
        } else {
            self.canonical_projection(turn, Some((id, false)), u64::MAX, &[])?
        };
        let mut result = BTreeMap::new();
        for (memory, amount) in effects {
            if *amount <= 0 || *amount > self.policy.linkage_principal {
                return Err("Freshness event exceeds pinned principal bound".into());
            }
            let mut record = *before
                .get(memory)
                .ok_or("Freshness event references unenrolled Memory at accepted cut")?;
            record
                .reinforce_with_policy(*amount, turn, &self.policy)
                .map_err(|e| e.to_string())?;
            result.insert(*memory, record);
        }
        Ok(result)
    }
    pub(crate) fn validate_event_payload(
        &self,
        id: &str,
        turn: u64,
        proof: FreshnessEventProof,
        effects: &BTreeMap<MemoryId, i16>,
        records: &BTreeMap<MemoryId, FreshnessRecord>,
    ) -> Result<(), String> {
        if self.contains_event(id)? {
            return Err("Freshness event identity already exists".into());
        }
        if id.is_empty() || effects.is_empty() {
            return Err("invalid Freshness event payload".into());
        }
        validate_proof(proof)?;
        if &self.prepare_event_postimages_for(id, turn, effects)? != records {
            return Err("Freshness event postimages differ from canonical effects".into());
        }
        Ok(())
    }
    pub(crate) fn resolve_accepted_use(
        &self,
        id: &str,
        turn: u64,
        sources: &[MemoryId],
    ) -> Result<Option<Vec<(String, BTreeMap<MemoryId, i16>, FreshnessEventProof)>>, String> {
        let Some(receipt) = self.find_use(id)? else {
            return Ok(None);
        };
        let (_, stored_turn, stored_sources, version) = use_fields(&receipt).unwrap();
        let mut canonical = sources.to_vec();
        canonical.sort();
        canonical.dedup();
        if stored_turn != turn
            || stored_sources != canonical.as_slice()
            || version != crate::freshness::FRESHNESS_POLICY_VERSION as u64
        {
            return Err("accepted-use identity collides with a different durable receipt".into());
        }
        let owner = self
            .owner_uuid
            .ok_or("accepted-use receipt lacks owner identity")?;
        let mut out = Vec::new();
        for source in stored_sources {
            let event_id = accepted_event_id(owner, id, *source);
            let event = self
                .find_event(&event_id)?
                .ok_or("accepted-use receipt has missing source event")?;
            let JournalEntry::Event {
                turn: event_turn,
                effects,
                proof,
                ..
            } = event
            else {
                unreachable!()
            };
            if event_turn != turn {
                return Err("accepted-use source event has mismatched cut".into());
            }
            out.push((event_id, effects, proof));
        }
        Ok(Some(out))
    }
    pub(crate) fn bind_owner(&mut self, owner: Option<[u8; 16]>) -> Result<(), String> {
        match (self.owner_uuid, owner) {
            (Some(stored), Some(actual)) if stored == actual => Ok(()),
            (None, None) if self.records.is_empty() => Ok(()),
            (None, Some(actual)) => {
                self.owner_uuid = Some(actual);
                Ok(())
            }
            _ => Err("Freshness journal owner does not match containing Reliquary".into()),
        }
    }
    pub(crate) fn derive_birth(
        &mut self,
        owner: [u8; 16],
        id: MemoryId,
        turn: u64,
        admitted: bool,
    ) -> Result<(), String> {
        if self.owner_uuid.is_some_and(|old| old != owner) {
            return Err("birth proof owner mismatch".into());
        }
        if !self.policy_persisted {
            self.ingest(&encode_policy(owner, self.policy))?;
        }
        let mut record = FreshnessRecord::created();
        if admitted {
            record.admit(turn).map_err(|e| e.to_string())?;
        }
        self.ingest(&self.encode_initialize(owner, id, turn, record))
    }
    pub(crate) fn records(&self) -> &BTreeMap<MemoryId, FreshnessRecord> {
        &self.records
    }
    pub(crate) fn has_any(&self) -> bool {
        self.policy_persisted || !self.records.is_empty()
    }
    pub(crate) fn contains_event(&self, id: &str) -> Result<bool, String> {
        Ok(self.find_event(id)?.is_some())
    }
    pub(crate) fn event_receipt(
        &self,
        id: &str,
    ) -> Result<Option<(u64, FreshnessEventProof, BTreeMap<MemoryId, i16>)>, String> {
        Ok(self.find_event(id)?.map(|event| {
            let JournalEntry::Event {
                turn,
                proof,
                effects,
                ..
            } = event
            else {
                unreachable!()
            };
            (turn, proof, effects)
        }))
    }
    pub(crate) fn event_matches(
        &self,
        id: &str,
        turn: u64,
        proof: FreshnessEventProof,
        effects: &BTreeMap<MemoryId, i16>,
    ) -> Result<bool, String> {
        self.event_matches_with_provenance(id, turn, proof, effects, None)
    }
    pub(crate) fn event_matches_with_provenance(
        &self,
        id: &str,
        turn: u64,
        proof: FreshnessEventProof,
        effects: &BTreeMap<MemoryId, i16>,
        provenance: Option<&FreshnessEventProvenance>,
    ) -> Result<bool, String> {
        Ok(
            matches!(self.find_event(id)?,Some(JournalEntry::Event{turn:t,proof:p,effects:e,provenance:v,..}) if t==turn && p==proof && &e==effects && provenance.is_none_or(|expected|expected==&v)),
        )
    }
    pub(crate) fn event_has_accepted_provenance(&self, id: &str) -> Result<bool, String> {
        let Some(JournalEntry::Event {
            turn,
            provenance,
            effects,
            ..
        }) = self.find_event(id)?
        else {
            return Ok(false);
        };
        let Some(owner) = self.owner_uuid else {
            return Err("Freshness receipt lacks owner binding".into());
        };
        if provenance.producer_kind != 2
            || provenance.origin_owner != owner
            || provenance.policy_version != crate::freshness::FRESHNESS_POLICY_VERSION as u64
        {
            return Ok(false);
        }
        let Some(receipt) = self.find_use(&provenance.source_id)? else {
            return Ok(false);
        };
        let (_, receipt_turn, sources, version) = use_fields(&receipt).unwrap();
        Ok(receipt_turn == turn
            && version == provenance.policy_version
            && sources.iter().any(|source| {
                accepted_event_id(owner, &provenance.source_id, *source) == id
                    && effects.get(source) == Some(&self.policy.access_principal)
            }))
    }
    pub(crate) fn record_at(
        &self,
        id: MemoryId,
        turn: u64,
    ) -> Result<Option<FreshnessRecord>, String> {
        Ok(self.records_at(turn)?.get(&id).copied())
    }
    pub(crate) fn record_at_cut(
        &self,
        id: MemoryId,
        cut: &crate::FreshnessEventCut,
    ) -> Result<Option<FreshnessRecord>, String> {
        Ok(self.records_at_cut(cut)?.get(&id).copied())
    }
    pub(crate) fn records_at(
        &self,
        turn: u64,
    ) -> Result<BTreeMap<MemoryId, FreshnessRecord>, String> {
        self.records_at_cut(&crate::FreshnessEventCut {
            rel_turn: turn,
            event_id_inclusive: None,
            graph_version: u64::MAX,
        })
    }
    pub(crate) fn records_at_cut(
        &self,
        cut: &crate::FreshnessEventCut,
    ) -> Result<BTreeMap<MemoryId, FreshnessRecord>, String> {
        let mut out = self.canonical_projection(
            cut.rel_turn,
            cut.event_id_inclusive.as_deref().map(|id| (id, true)),
            cut.graph_version,
            &[],
        )?;
        for record in out.values_mut() {
            if record.admitted_at_turn.is_some() {
                record
                    .settle_with_policy(cut.rel_turn, &self.policy)
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(out)
    }
    pub(crate) fn replay_entries(&self) -> Result<Vec<FreshnessReplayEntry>, String> {
        let mut out = Vec::new();
        for (id, (turn, record)) in &self.births {
            out.push(FreshnessReplayEntry::Initialize {
                memory_id: *id,
                source_turn: *turn,
                admitted_at_birth: record.admitted_at_turn.is_some(),
            });
        }
        for (id, record) in &self.admissions {
            out.push(FreshnessReplayEntry::Admit {
                memory_id: *id,
                source_turn: record.admitted_at_turn.unwrap(),
            });
        }
        self.scan_entries(|entry| {
            for_each_event(&entry, &mut |event| {
                if let JournalEntry::Event {
                    event_id,
                    turn,
                    provenance,
                    proof,
                    effects,
                    ..
                } = event
                {
                    out.push(FreshnessReplayEntry::Event {
                        event_id: event_id.clone(),
                        source_turn: *turn,
                        effects: effects.clone(),
                        proof: *proof,
                        provenance: Some(provenance.clone()),
                    });
                }
                Ok(())
            })?;
            if let Some((id, turn, sources, version)) = use_fields(&entry) {
                out.push(FreshnessReplayEntry::AcceptedUseReceipt {
                    use_id: id.to_owned(),
                    source_turn: turn,
                    sources: sources.to_vec(),
                    policy_version: version,
                });
            }
            Ok(())
        })?;
        Ok(out)
    }
    fn validate_use_identity(
        &self,
        id: &str,
        sources: &[MemoryId],
        version: u64,
    ) -> Result<(), String> {
        if id.trim().is_empty()
            || sources.is_empty()
            || sources.windows(2).any(|p| p[0] >= p[1])
            || version != crate::freshness::FRESHNESS_POLICY_VERSION as u64
        {
            return Err("invalid accepted-use identity or policy".into());
        }
        Ok(())
    }
    pub(crate) fn prepare_accepted_use_batch(
        &self,
        owner: [u8; 16],
        use_id: String,
        turn: u64,
        sources: Vec<MemoryId>,
        proof: FreshnessEventProof,
        effects: Vec<BTreeMap<MemoryId, i16>>,
    ) -> Result<Vec<u8>, String> {
        self.validate_use_identity(
            &use_id,
            &sources,
            crate::freshness::FRESHNESS_POLICY_VERSION as u64,
        )?;
        if self.owner_uuid != Some(owner) || effects.len() != sources.len() {
            return Err("accepted-use owner/source count mismatch".into());
        }
        validate_proof(proof)?;
        if self.find_use(&use_id)?.is_some() {
            return Err("accepted-use receipt already exists".into());
        }
        let mut events = Vec::with_capacity(sources.len());
        for (source, effects) in sources.iter().zip(effects) {
            if effects.get(source) != Some(&self.policy.access_principal)
                || effects.values().any(|v| *v > self.policy.access_principal)
            {
                return Err("accepted-use effects lack eligible source principal".into());
            }
            let event_id = accepted_event_id(owner, &use_id, *source);
            if self.find_event(&event_id)?.is_some() {
                return Err("accepted-use source event exists without receipt".into());
            }
            let provenance = FreshnessEventProvenance {
                producer_kind: 2,
                source_id: use_id.clone(),
                origin_owner: owner,
                accepted_turn: turn,
                policy_version: crate::freshness::FRESHNESS_POLICY_VERSION as u64,
            };
            let event = JournalEntry::Event {
                event_id,
                turn,
                provenance,
                proof,
                effects,
                records: BTreeMap::new(),
            };
            self.validate_event_entry(owner, &event)?;
            events.push(event);
        }
        self.project_with_events(&events)?;
        Ok(encode(
            owner,
            &JournalEntry::AcceptedUseBatch {
                use_id,
                turn,
                sources,
                policy_version: crate::freshness::FRESHNESS_POLICY_VERSION as u64,
                events,
            },
        ))
    }
    pub(crate) fn encode_initialize(
        &self,
        owner: [u8; 16],
        id: MemoryId,
        turn: u64,
        record: FreshnessRecord,
    ) -> Vec<u8> {
        encode(
            owner,
            &JournalEntry::Initialize {
                memory_id: id,
                turn,
                record,
            },
        )
    }
    pub(crate) fn encode_admit(
        &self,
        owner: [u8; 16],
        id: MemoryId,
        record: FreshnessRecord,
    ) -> Vec<u8> {
        encode(
            owner,
            &JournalEntry::Admit {
                memory_id: id,
                record,
            },
        )
    }
    pub(crate) fn encode_event(
        &self,
        owner: [u8; 16],
        event_id: String,
        turn: u64,
        proof: FreshnessEventProof,
        effects: BTreeMap<MemoryId, i16>,
        records: BTreeMap<MemoryId, FreshnessRecord>,
    ) -> Vec<u8> {
        let provenance = FreshnessEventProvenance {
            producer_kind: 1,
            source_id: event_id.clone(),
            origin_owner: owner,
            accepted_turn: turn,
            policy_version: crate::freshness::FRESHNESS_POLICY_VERSION as u64,
        };
        encode(
            owner,
            &JournalEntry::Event {
                event_id,
                turn,
                provenance,
                proof,
                effects,
                records,
            },
        )
    }
    pub(crate) fn encode_event_with_provenance(
        &self,
        owner: [u8; 16],
        event_id: String,
        turn: u64,
        proof: FreshnessEventProof,
        effects: BTreeMap<MemoryId, i16>,
        records: BTreeMap<MemoryId, FreshnessRecord>,
        provenance: Option<FreshnessEventProvenance>,
    ) -> Result<Vec<u8>, String> {
        let provenance = provenance.unwrap_or_else(|| FreshnessEventProvenance {
            producer_kind: 1,
            source_id: event_id.clone(),
            origin_owner: owner,
            accepted_turn: turn,
            policy_version: crate::freshness::FRESHNESS_POLICY_VERSION as u64,
        });
        let entry = JournalEntry::Event {
            event_id,
            turn,
            provenance,
            proof,
            effects,
            records,
        };
        self.validate_event_entry(owner, &entry)?;
        let JournalEntry::Event {
            event_id,
            turn,
            proof,
            effects,
            records,
            ..
        } = &entry
        else {
            unreachable!()
        };
        // Reconciliation must not append invalid source provenance, candidate
        // postimages or graph proof and then discover an unreopenable record.
        self.validate_event_payload(event_id, *turn, *proof, effects, records)?;
        Ok(encode(owner, &entry))
    }

    pub(crate) fn encode_accepted_use_receipt(
        &self,
        owner: [u8; 16],
        use_id: String,
        turn: u64,
        sources: Vec<MemoryId>,
        policy_version: u64,
    ) -> Vec<u8> {
        encode(
            owner,
            &JournalEntry::AcceptedUseReceipt {
                use_id,
                turn,
                sources,
                policy_version,
            },
        )
    }
}
fn encode(owner: [u8; 16], entry: &JournalEntry) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&owner);
    match entry {
        JournalEntry::Initialize {
            memory_id,
            turn,
            record,
        } => {
            out.push(0);
            out.extend_from_slice(&memory_id.0);
            out.extend_from_slice(&turn.to_le_bytes());
            put_record(&mut out, *record);
        }
        JournalEntry::Admit { memory_id, record } => {
            out.push(1);
            out.extend_from_slice(&memory_id.0);
            put_record(&mut out, *record);
        }
        JournalEntry::Event {
            event_id,
            turn,
            provenance,
            proof,
            effects,
            records,
        } => {
            out.push(2);
            put_event(
                &mut out, event_id, *turn, provenance, *proof, effects, records,
            );
        }
        JournalEntry::AcceptedUseReceipt {
            use_id,
            turn,
            sources,
            policy_version,
        } => {
            out.push(4);
            put_bytes(&mut out, use_id.as_bytes());
            out.extend_from_slice(&turn.to_le_bytes());
            out.extend_from_slice(&policy_version.to_le_bytes());
            out.extend_from_slice(&(sources.len() as u32).to_le_bytes());
            for source in sources {
                out.extend_from_slice(&source.0);
            }
        }
        JournalEntry::AcceptedUseBatch {
            use_id,
            turn,
            sources,
            policy_version,
            events,
        } => {
            out.push(3);
            put_bytes(&mut out, use_id.as_bytes());
            out.extend_from_slice(&turn.to_le_bytes());
            out.extend_from_slice(&policy_version.to_le_bytes());
            out.extend_from_slice(&(sources.len() as u32).to_le_bytes());
            for source in sources {
                out.extend_from_slice(&source.0);
            }
            out.extend_from_slice(&(events.len() as u32).to_le_bytes());
            for event in events {
                if let JournalEntry::Event {
                    event_id,
                    turn,
                    provenance,
                    proof,
                    effects,
                    records,
                } = event
                {
                    put_event(
                        &mut out, event_id, *turn, provenance, *proof, effects, records,
                    );
                }
            }
        }
    }
    out
}

fn put_event(
    out: &mut Vec<u8>,
    event_id: &str,
    turn: u64,
    provenance: &FreshnessEventProvenance,
    proof: FreshnessEventProof,
    effects: &BTreeMap<MemoryId, i16>,
    records: &BTreeMap<MemoryId, FreshnessRecord>,
) {
    put_bytes(out, event_id.as_bytes());
    out.extend_from_slice(&turn.to_le_bytes());
    out.push(provenance.producer_kind);
    put_bytes(out, provenance.source_id.as_bytes());
    out.extend_from_slice(&provenance.origin_owner);
    out.extend_from_slice(&provenance.accepted_turn.to_le_bytes());
    out.extend_from_slice(&provenance.policy_version.to_le_bytes());
    out.extend_from_slice(&proof.graph_version.to_le_bytes());
    for value in [proof.community_generation, proof.community_graph_version] {
        match value {
            Some(value) => {
                out.push(1);
                out.extend_from_slice(&value.to_le_bytes());
            }
            None => out.push(0),
        }
    }
    out.extend_from_slice(&(effects.len() as u32).to_le_bytes());
    for (id, amount) in effects {
        out.extend_from_slice(&id.0);
        out.extend_from_slice(&amount.to_le_bytes());
        // Per-event postimages are derived projection data. The canonical event
        // receipt stores only immutable effects; open/replay recomputes postimages.
        let _ = records;
    }
}

fn put_record(out: &mut Vec<u8>, record: FreshnessRecord) {
    out.extend_from_slice(&record.score.to_le_bytes());
    for value in [record.admitted_at_turn, record.accounted_turn] {
        match value {
            Some(value) => {
                out.push(1);
                out.extend_from_slice(&value.to_le_bytes());
            }
            None => out.push(0),
        }
    }
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn decode(payload: &[u8]) -> Result<([u8; 16], JournalEntry), String> {
    let mut r = Reader {
        bytes: payload,
        at: 8,
    };
    let owner = r.array::<16>()?;
    let kind = r.u8()?;
    let entry = match kind {
        0 | 1 => {
            let id = MemoryId(r.array::<32>()?);
            let turn = if kind == 0 { Some(r.u64()?) } else { None };
            let record = r.record()?;
            if kind == 0 {
                JournalEntry::Initialize {
                    memory_id: id,
                    turn: turn.unwrap(),
                    record,
                }
            } else {
                JournalEntry::Admit {
                    memory_id: id,
                    record,
                }
            }
        }
        2 => r.event()?,
        3 | 4 => {
            let use_id =
                String::from_utf8(r.bytes()?).map_err(|_| "invalid accepted-use ID UTF-8")?;
            let turn = r.u64()?;
            let policy_version = r.u64()?;
            let source_count = r.u32()? as usize;
            if source_count > payload.len() / 32 {
                return Err("invalid accepted-use source count".into());
            }
            let mut sources = Vec::with_capacity(source_count);
            for _ in 0..source_count {
                sources.push(MemoryId(r.array()?));
            }
            if kind == 4 {
                JournalEntry::AcceptedUseReceipt {
                    use_id,
                    turn,
                    sources,
                    policy_version,
                }
            } else {
                let event_count = r.u32()? as usize;
                if event_count != source_count {
                    return Err("accepted-use source/event count mismatch".into());
                }
                let mut events = Vec::with_capacity(event_count);
                for _ in 0..event_count {
                    events.push(r.event()?);
                }
                JournalEntry::AcceptedUseBatch {
                    use_id,
                    turn,
                    sources,
                    policy_version,
                    events,
                }
            }
        }
        _ => return Err("unknown Freshness journal record kind".into()),
    };
    if r.at != payload.len() {
        return Err("trailing bytes in Freshness journal record".into());
    }
    Ok((owner, entry))
}

fn encode_policy(owner: [u8; 16], policy: FreshnessPolicy) -> Vec<u8> {
    let mut out = Vec::with_capacity(34);
    out.extend_from_slice(POLICY_MAGIC);
    out.extend_from_slice(&owner);
    out.extend_from_slice(&crate::freshness::FRESHNESS_POLICY_VERSION.to_le_bytes());
    out.extend_from_slice(&policy.decay_turns_per_point.to_le_bytes());
    for value in [
        policy.access_principal,
        policy.linkage_principal,
        policy.local_hop_cost,
        policy.outside_hop_cost,
    ] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn decode_policy(payload: &[u8]) -> Result<([u8; 16], FreshnessPolicy), String> {
    if payload.len() != 8 + 16 + 2 + 8 + 8 {
        return Err("invalid Freshness policy length".into());
    }
    let mut r = Reader {
        bytes: payload,
        at: 8,
    };
    let owner = r.array::<16>()?;
    let version = u16::from_le_bytes(r.array()?);
    if version != crate::freshness::FRESHNESS_POLICY_VERSION {
        return Err("unsupported Freshness policy version".into());
    }
    let policy = FreshnessPolicy {
        decay_turns_per_point: r.u64()?,
        access_principal: r.i16()?,
        linkage_principal: r.i16()?,
        local_hop_cost: r.i16()?,
        outside_hop_cost: r.i16()?,
    };
    if r.at != payload.len() {
        return Err("trailing Freshness policy bytes".into());
    }
    Ok((owner, policy))
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self
            .at
            .checked_add(n)
            .ok_or("Freshness record length overflow")?;
        let value = self
            .bytes
            .get(self.at..end)
            .ok_or("truncated Freshness record")?;
        self.at = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_le_bytes(self.array()?))
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().expect("length checked"))
    }
    fn bytes(&mut self) -> Result<Vec<u8>, String> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
    fn optional_turn(&mut self) -> Result<Option<u64>, String> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(self.u64()?)),
            _ => Err("invalid Freshness optional turn".into()),
        }
    }
    fn event(&mut self) -> Result<JournalEntry, String> {
        let event_id =
            String::from_utf8(self.bytes()?).map_err(|_| "invalid Freshness event ID UTF-8")?;
        let turn = self.u64()?;
        let producer_kind = self.u8()?;
        let source_id = String::from_utf8(self.bytes()?)
            .map_err(|_| "invalid Freshness producer source ID UTF-8")?;
        let origin_owner = self.array::<16>()?;
        let accepted_turn = self.u64()?;
        let policy_version = self.u64()?;
        if !(1..=2).contains(&producer_kind)
            || source_id.is_empty()
            || policy_version != crate::freshness::FRESHNESS_POLICY_VERSION as u64
        {
            return Err("invalid Freshness event provenance".into());
        }
        let provenance = FreshnessEventProvenance {
            producer_kind,
            source_id,
            origin_owner,
            accepted_turn,
            policy_version,
        };
        let graph_version = self.u64()?;
        let community_generation = self.optional_turn()?;
        let community_graph_version = self.optional_turn()?;
        let proof = FreshnessEventProof {
            graph_version,
            community_generation,
            community_graph_version,
        };
        if proof.community_generation.is_some() != proof.community_graph_version.is_some()
            || proof
                .community_graph_version
                .is_some_and(|version| version != graph_version)
        {
            return Err("invalid event community proof".into());
        }
        let count = self.u32()? as usize;
        let mut effects = BTreeMap::new();
        let records = BTreeMap::new();
        for _ in 0..count {
            let id = MemoryId(self.array()?);
            let amount = self.i16()?;
            if effects.insert(id, amount).is_some() {
                return Err("duplicate Memory in Freshness event batch".into());
            }
        }
        if event_id.is_empty() || count == 0 {
            return Err("empty Freshness event batch".into());
        }
        Ok(JournalEntry::Event {
            event_id,
            turn,
            provenance,
            proof,
            effects,
            records,
        })
    }
    fn record(&mut self) -> Result<FreshnessRecord, String> {
        let score = self.i16()?;
        let record = FreshnessRecord {
            score,
            admitted_at_turn: self.optional_turn()?,
            accounted_turn: self.optional_turn()?,
        };
        if !(-100..=100).contains(&score)
            || record.admitted_at_turn.is_some() != record.accounted_turn.is_some()
            || matches!((record.admitted_at_turn, record.accounted_turn), (Some(admitted), Some(accounted)) if accounted < admitted)
        {
            return Err("invalid Freshness score or admission clock".into());
        }
        Ok(record)
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn accepted_event_id(owner: [u8; 16], use_id: &str, source: MemoryId) -> String {
    let owner: String = owner.iter().map(|byte| format!("{byte:02x}")).collect();
    let use_id: String = use_id
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("accepted-use:{owner}:{use_id}:{}", hex_id(&source))
}

fn event_sort_key(entry: &JournalEntry) -> (u64, String) {
    match entry {
        JournalEntry::Event { event_id, turn, .. } => (*turn, event_id.clone()),
        _ => (u64::MAX, String::new()),
    }
}

fn event_receipts_equal(left: &JournalEntry, right: &JournalEntry) -> bool {
    match (left, right) {
        (
            JournalEntry::Event {
                event_id: left_id,
                turn: left_turn,
                provenance: left_provenance,
                proof: left_proof,
                effects: left_effects,
                ..
            },
            JournalEntry::Event {
                event_id: right_id,
                turn: right_turn,
                provenance: right_provenance,
                proof: right_proof,
                effects: right_effects,
                ..
            },
        ) => {
            left_id == right_id
                && left_turn == right_turn
                && left_provenance == right_provenance
                && left_proof == right_proof
                && left_effects == right_effects
        }
        _ => false,
    }
}

fn validate_proof(proof: FreshnessEventProof) -> Result<(), String> {
    if proof.community_generation.is_some() != proof.community_graph_version.is_some()
        || proof
            .community_graph_version
            .is_some_and(|v| v != proof.graph_version)
    {
        return Err("invalid Freshness graph/community proof".into());
    }
    Ok(())
}
fn event_in<'a>(entry: &'a JournalEntry, id: &str) -> Option<&'a JournalEntry> {
    match entry {
        JournalEntry::Event { event_id, .. } if event_id == id => Some(entry),
        JournalEntry::AcceptedUseBatch { events, .. } => events
            .iter()
            .find(|e| matches!(e,JournalEntry::Event{event_id,..} if event_id==id)),
        _ => None,
    }
}
fn for_each_event(
    entry: &JournalEntry,
    visitor: &mut impl FnMut(&JournalEntry) -> Result<(), String>,
) -> Result<(), String> {
    match entry {
        JournalEntry::Event { .. } => visitor(entry),
        JournalEntry::AcceptedUseBatch { events, .. } => {
            for event in events {
                visitor(event)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
fn use_fields(entry: &JournalEntry) -> Option<(&str, u64, &[MemoryId], u64)> {
    match entry {
        JournalEntry::AcceptedUseBatch {
            use_id,
            turn,
            sources,
            policy_version,
            ..
        }
        | JournalEntry::AcceptedUseReceipt {
            use_id,
            turn,
            sources,
            policy_version,
        } => Some((use_id, *turn, sources, *policy_version)),
        _ => None,
    }
}
fn use_receipts_equal(a: &JournalEntry, b: &JournalEntry) -> bool {
    if use_fields(a) != use_fields(b) {
        return false;
    }
    match (a, b) {
        (
            JournalEntry::AcceptedUseBatch { events: a, .. },
            JournalEntry::AcceptedUseBatch { events: b, .. },
        ) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| event_receipts_equal(a, b)),
        (JournalEntry::AcceptedUseReceipt { .. }, JournalEntry::AcceptedUseReceipt { .. }) => true,
        _ => false,
    }
}
// Conservative heap accounting: tree nodes and vector/string capacities are
// charged above their representation sizes; the count limit bounds queue overhead.
fn receipt_bytes(entry: &JournalEntry) -> usize {
    match entry {
        JournalEntry::Event {
            event_id,
            provenance,
            effects,
            records,
            ..
        } => {
            256 + event_id.capacity()
                + provenance.source_id.capacity()
                + 128 * (effects.len() + records.len())
        }
        JournalEntry::AcceptedUseBatch {
            use_id,
            sources,
            events,
            ..
        } => {
            256 + use_id.capacity()
                + sources.capacity() * std::mem::size_of::<MemoryId>()
                + events.capacity() * std::mem::size_of::<JournalEntry>()
                + events.iter().map(receipt_bytes).sum::<usize>()
        }
        JournalEntry::AcceptedUseReceipt {
            use_id, sources, ..
        } => 256 + use_id.capacity() + sources.capacity() * std::mem::size_of::<MemoryId>(),
        _ => 256,
    }
}
