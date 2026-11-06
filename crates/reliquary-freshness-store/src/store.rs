use crate::codec::{
    JOURNAL_MAGIC, JournalEntry, POLICY_MAGIC, decode_entry, decode_policy, encode_entry,
    encode_policy,
};
use crate::historical_snapshot::HistoricalFreshnessSnapshot;
use crate::projection::{
    FreshnessEvent, FreshnessEventCut, FreshnessEventProof, FreshnessEventProvenance,
    FreshnessReplayEntry, FreshnessUseReceipt,
};
use crate::{
    FRESHNESS_POLICY_VERSION, FreshnessMemoryId, FreshnessPolicy, FreshnessRecord, validate_policy,
};
use reliquary_container::{Container, ContainerError, ContainerIdentity};
use std::collections::BTreeMap;
use std::io::Read;

mod history_cut;
#[path = "initial_dream.rs"]
mod initial_dream;

const MAX_FRAME_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct FreshnessStore {
    owner_uuid: Option<[u8; 16]>,
    identity: Option<ContainerIdentity>,
    policy: FreshnessPolicy,
    policy_persisted: bool,
    births: BTreeMap<FreshnessMemoryId, (u64, bool)>,
    admissions: BTreeMap<FreshnessMemoryId, u64>,
    events: BTreeMap<String, FreshnessEvent>,
    uses: BTreeMap<String, (FreshnessUseReceipt, FreshnessEventProof)>,
    journal: Vec<JournalEntry>,
    historical_snapshots: Vec<HistoricalFreshnessSnapshot>,
}

impl Default for FreshnessStore {
    fn default() -> Self {
        Self::new(None)
    }
}

impl FreshnessStore {
    pub fn new(owner_uuid: Option<[u8; 16]>) -> Self {
        Self {
            owner_uuid,
            identity: None,
            policy: FreshnessPolicy::default_v1(),
            policy_persisted: false,
            births: BTreeMap::new(),
            admissions: BTreeMap::new(),
            events: BTreeMap::new(),
            uses: BTreeMap::new(),
            journal: Vec::new(),
            historical_snapshots: Vec::new(),
        }
    }

    pub fn open(container: &Container) -> Result<Self, String> {
        Self::open_with_grouped_births(container, |_| {
            Err("grouped completion requires owner-session replay".into())
        })
    }

    /// Replay grouped Insomnia Memory births at their physical frame position.
    /// The carrier codec is supplied by owner-session; Freshness keeps
    /// ownership of policy and birth admission.
    pub fn open_with_grouped_births(
        container: &Container,
        decode_grouped: impl FnMut(&[u8]) -> Result<Option<(u64, u64, Vec<FreshnessMemoryId>)>, String>,
    ) -> Result<Self, String> {
        Self::open_with_grouped_dream(container, decode_grouped, |_, _| {
            Err("grouped Dream requires owner-session replay".into())
        })
    }

    /// Owner-decoded Dream effects retain their physical ordering and use the
    /// same policy and transition validation as ordinary Freshness records.
    pub fn open_with_grouped_dream(
        container: &Container,
        mut decode_grouped: impl FnMut(
            &[u8],
        )
            -> Result<Option<(u64, u64, Vec<FreshnessMemoryId>)>, String>,
        mut decode_dream: impl FnMut(&[u8], &Self) -> Result<Vec<Vec<u8>>, String>,
    ) -> Result<Self, String> {
        let owner = container.owner_id();
        let mut store = Self::new(Some(owner));
        store.identity = Some(container.identity());
        let mut cursor = container.physical_cursor().map_err(display_error)?;
        while let Some(mut frame) = cursor.next_frame().map_err(display_error)? {
            if frame.object().length > MAX_FRAME_BYTES {
                return Err("Freshness record frame exceeds size limit".into());
            }
            let mut payload = Vec::new();
            frame
                .read_to_end(&mut payload)
                .map_err(|error| error.to_string())?;
            if payload.starts_with(b"RELDRF") {
                for effect in decode_dream(&payload, &store)? {
                    store.ingest_payload(&effect)?;
                }
                continue;
            }
            if payload.starts_with(b"RELINSC7") {
                if let Some((turn, policy, memory_ids)) = decode_grouped(&payload)? {
                    if policy != u64::from(FRESHNESS_POLICY_VERSION) {
                        return Err("unsupported grouped Freshness policy".into());
                    }
                    store.apply_grouped_births(owner, turn, memory_ids)?;
                }
                continue;
            }
            store.ingest_payload(&payload)?;
        }
        Ok(store)
    }

    pub fn preflight_grouped_births(
        &self,
        owner: [u8; 16],
        accepted_turn: u64,
        memory_ids: Vec<FreshnessMemoryId>,
    ) -> Result<(), String> {
        let mut projected = self.clone();
        projected.apply_grouped_births(owner, accepted_turn, memory_ids)
    }

    pub fn apply_grouped_births(
        &mut self,
        owner: [u8; 16],
        accepted_turn: u64,
        memory_ids: Vec<FreshnessMemoryId>,
    ) -> Result<(), String> {
        if self.owner_uuid.is_some_and(|actual| actual != owner) {
            return Err("grouped Freshness birth belongs to another owner".into());
        }
        if self.owner_uuid.is_none() {
            return Err("grouped Freshness owner identity is required".into());
        }
        if !self.policy_persisted {
            return Err("grouped Freshness birth precedes owner policy".into());
        }
        let mut next = self.clone();
        for memory_id in memory_ids {
            let entry = JournalEntry::Birth {
                memory_id,
                accepted_turn,
                admitted_at_birth: false,
            };
            next.validate_entry(&entry)?;
            next.apply_entry(entry)?;
        }
        *self = next;
        Ok(())
    }

    pub fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.owner_uuid
    }

    pub fn policy(&self) -> FreshnessPolicy {
        self.policy
    }

    pub fn set_policy_before_enrollment(&mut self, policy: FreshnessPolicy) -> Result<(), String> {
        validate_policy(&policy).map_err(|error| error.to_string())?;
        if self.policy_persisted || !self.births.is_empty() {
            return Err("Freshness policy is immutable after owner enrollment".into());
        }
        self.policy = policy;
        Ok(())
    }

    pub fn initialize(&mut self, container: &mut Container) -> Result<bool, String> {
        self.bind_owner(container.owner_id())?;
        if self
            .identity
            .is_some_and(|identity| identity != container.identity())
        {
            return Err("Freshness container identity does not match owner".into());
        }
        if self.policy_persisted {
            return Ok(false);
        }
        let mut cursor = container.physical_cursor().map_err(display_error)?;
        while let Some(mut frame) = cursor.next_frame().map_err(display_error)? {
            if frame.object().length < 8 {
                continue;
            }
            let mut magic = [0; 8];
            frame
                .read_exact(&mut magic)
                .map_err(|error| error.to_string())?;
            if magic == *POLICY_MAGIC || magic == *JOURNAL_MAGIC || magic == *b"RELINSC7" {
                return Err(
                    "Freshness owner already contains records; reopen before writing".into(),
                );
            }
        }
        drop(cursor);
        let owner = self
            .owner_uuid
            .ok_or("Freshness owner identity is required")?;
        self.append_then_ingest(container, encode_policy(owner, self.policy))?;
        self.identity = Some(container.identity());
        Ok(true)
    }

    pub fn bind_owner(&mut self, owner: [u8; 16]) -> Result<(), String> {
        match self.owner_uuid {
            Some(existing) if existing != owner => {
                Err("Freshness journal owner does not match containing Reliquary".into())
            }
            Some(_) => Ok(()),
            None if self.births.is_empty() && self.events.is_empty() => {
                self.owner_uuid = Some(owner);
                Ok(())
            }
            None => Err("cannot bind an enrolled Freshness store to an owner".into()),
        }
    }

    pub fn create_birth(
        &mut self,
        container: &mut Container,
        memory_id: FreshnessMemoryId,
        accepted_turn: u64,
        admitted_at_birth: bool,
    ) -> Result<bool, String> {
        self.ensure_ready_for(container)?;
        if let Some((turn, admitted)) = self.births.get(&memory_id) {
            return if *turn == accepted_turn && *admitted == admitted_at_birth {
                Ok(false)
            } else {
                Err("Freshness birth identity conflicts with accepted evidence".into())
            };
        }
        let entry = JournalEntry::Birth {
            memory_id,
            accepted_turn,
            admitted_at_birth,
        };
        self.append_entry(container, entry)?;
        Ok(true)
    }

    pub fn admit(
        &mut self,
        container: &mut Container,
        memory_id: FreshnessMemoryId,
        accepted_turn: u64,
    ) -> Result<bool, String> {
        self.ensure_ready_for(container)?;
        let (birth_turn, admitted_at_birth) = self
            .births
            .get(&memory_id)
            .copied()
            .ok_or("Freshness admission requires an accepted birth")?;
        if accepted_turn < birth_turn {
            return Err("Freshness admission precedes accepted birth".into());
        }
        if admitted_at_birth {
            return if accepted_turn == birth_turn {
                Ok(false)
            } else {
                Err("Freshness birth admission cannot be reanchored".into())
            };
        }
        if let Some(turn) = self.admissions.get(&memory_id) {
            return if *turn == accepted_turn {
                Ok(false)
            } else {
                Err("Freshness admission cannot be reanchored".into())
            };
        }
        self.append_entry(
            container,
            JournalEntry::Admission {
                memory_id,
                accepted_turn,
            },
        )?;
        Ok(true)
    }

    pub fn append_event(
        &mut self,
        container: &mut Container,
        event: FreshnessEvent,
    ) -> Result<bool, String> {
        self.ensure_ready_for(container)?;
        if let Some(existing) = self.events.get(&event.event_id) {
            return if existing == &event {
                Ok(false)
            } else {
                Err("Freshness event identity collides with different accepted evidence".into())
            };
        }
        let entry = JournalEntry::Event(event);
        self.validate_entry(&entry)?;
        self.append_entry(container, entry)?;
        Ok(true)
    }

    /// Persists one accepted-use receipt and all source access events in one frame.
    /// Reopening replays the receipt and derives the same owner-scoped activity events.
    pub fn accept_use(
        &mut self,
        container: &mut Container,
        use_id: String,
        accepted_turn: u64,
        mut sources: Vec<FreshnessMemoryId>,
        proof: FreshnessEventProof,
    ) -> Result<bool, String> {
        self.ensure_ready_for(container)?;
        if use_id.trim().is_empty() {
            return Err("accepted-use identity must be nonempty".into());
        }
        proof.validate()?;
        sources.sort();
        sources.dedup();
        if sources.is_empty() {
            return Err("accepted-use receipt must name at least one source".into());
        }
        let owner = self
            .owner_uuid
            .ok_or("Freshness owner identity is required")?;
        let receipt = FreshnessUseReceipt {
            use_id: use_id.clone(),
            accepted_turn,
            sources,
            policy_version: u64::from(FRESHNESS_POLICY_VERSION),
        };
        if let Some((existing, existing_proof)) = self.uses.get(&use_id) {
            return if existing == &receipt && *existing_proof == proof {
                Ok(false)
            } else {
                Err("accepted-use identity collides with different receipt evidence".into())
            };
        }
        for source in &receipt.sources {
            let event_id = accepted_event_id(owner, &use_id, *source);
            if self.events.contains_key(&event_id) {
                return Err("accepted-use source event already exists outside its receipt".into());
            }
            let (birth_turn, _) = self
                .births
                .get(source)
                .ok_or("accepted-use receipt references unenrolled Memory")?;
            if *birth_turn > accepted_turn {
                return Err("accepted-use receipt precedes Memory birth".into());
            }
        }
        let entry = JournalEntry::AcceptedUse(receipt, proof);
        self.validate_entry(&entry)?;
        self.append_entry(container, entry)?;
        Ok(true)
    }

    pub fn ingest_payload(&mut self, payload: &[u8]) -> Result<(), String> {
        if payload.starts_with(POLICY_MAGIC) {
            let Some((owner, policy)) = decode_policy(payload)? else {
                unreachable!()
            };
            if self.owner_uuid.is_some_and(|actual| actual != owner) {
                return Err("Freshness policy belongs to another owner".into());
            }
            if self.policy_persisted {
                return Err(if self.policy == policy {
                    "duplicate Freshness owner policy in history".into()
                } else {
                    "Freshness owner policy changed within history".into()
                });
            }
            self.owner_uuid.get_or_insert(owner);
            self.policy = policy;
            self.policy_persisted = true;
            return Ok(());
        }
        if let Some(snapshot) = HistoricalFreshnessSnapshot::decode(payload, &self.policy)? {
            return self.apply_historical_snapshot(snapshot);
        }
        if !payload.starts_with(JOURNAL_MAGIC) && payload.starts_with(b"RELFRS") {
            return Err("unsupported Freshness journal version".into());
        }
        let Some((owner, entry)) = decode_entry(payload)? else {
            return Ok(());
        };
        if self.owner_uuid.is_some_and(|actual| actual != owner) {
            return Err("Freshness journal belongs to another owner".into());
        }
        if !self.policy_persisted {
            return Err("Freshness activity precedes its owner policy".into());
        }
        self.validate_entry(&entry)?;
        self.apply_entry(entry)
    }

    pub fn append_historical_snapshot(
        &mut self,
        container: &mut Container,
        mut snapshot: HistoricalFreshnessSnapshot,
    ) -> Result<bool, String> {
        self.ensure_ready_for(container)?;
        let owner = self
            .owner_uuid
            .ok_or("Freshness owner identity is required")?;
        if snapshot.owner_uuid != owner {
            return Err("historical Freshness snapshot owner mismatch".into());
        }
        if let Some(existing) = self
            .historical_snapshots
            .iter()
            .find(|known| known.import_id == snapshot.import_id)
        {
            return if existing == &snapshot {
                Ok(false)
            } else {
                Err("conflicting historical Freshness snapshot retry".into())
            };
        }
        snapshot.journal_entries_included = self.journal.len() as u64;
        let current = self.project(None)?;
        if snapshot.records.len() != current.len()
            || snapshot.records.iter().any(|(id, record)| {
                current
                    .get(id)
                    .is_none_or(|known| known.admitted_at_turn != record.admitted_at_turn)
            })
        {
            return Err("historical Freshness snapshot changes owner enrollment".into());
        }
        let payload = snapshot.encode(&self.policy)?;
        self.append_then_ingest(container, payload)?;
        Ok(true)
    }

    pub fn historical_snapshot(&self, import_id: &str) -> Option<&HistoricalFreshnessSnapshot> {
        self.historical_snapshots
            .iter()
            .find(|snapshot| snapshot.import_id == import_id)
    }

    pub fn records(&self) -> Result<BTreeMap<FreshnessMemoryId, FreshnessRecord>, String> {
        self.project(None)
    }

    pub fn records_at(
        &self,
        cut: &FreshnessEventCut,
    ) -> Result<BTreeMap<FreshnessMemoryId, FreshnessRecord>, String> {
        self.project(Some(cut))
    }

    /// Project all accepted events through the given turn, including events
    /// accepted on that turn. An event cut without an ID instead selects the
    /// boundary before that turn's events.
    pub fn records_through_turn(
        &self,
        rel_turn: u64,
    ) -> Result<BTreeMap<FreshnessMemoryId, FreshnessRecord>, String> {
        self.records_at(&self.cut_through_turn(rel_turn))
    }

    pub fn record_at(
        &self,
        memory_id: FreshnessMemoryId,
        turn: u64,
    ) -> Result<Option<FreshnessRecord>, String> {
        Ok(self
            .records_at(&FreshnessEventCut {
                rel_turn: turn,
                event_id_inclusive: None,
            })?
            .get(&memory_id)
            .copied())
    }

    pub fn event(&self, event_id: &str) -> Option<&FreshnessEvent> {
        self.events.get(event_id)
    }

    pub fn accepted_use(&self, use_id: &str) -> Option<&FreshnessUseReceipt> {
        self.uses.get(use_id).map(|(receipt, _)| receipt)
    }

    pub fn replay_entries(&self) -> Vec<FreshnessReplayEntry> {
        let mut entries = Vec::new();
        for entry in &self.journal {
            match entry {
                JournalEntry::Birth {
                    memory_id,
                    accepted_turn,
                    admitted_at_birth,
                } => entries.push(FreshnessReplayEntry::Birth {
                    memory_id: *memory_id,
                    accepted_turn: *accepted_turn,
                    admitted_at_birth: *admitted_at_birth,
                }),
                JournalEntry::Admission {
                    memory_id,
                    accepted_turn,
                } => entries.push(FreshnessReplayEntry::Admission {
                    memory_id: *memory_id,
                    accepted_turn: *accepted_turn,
                }),
                JournalEntry::Event(event) => {
                    entries.push(FreshnessReplayEntry::Event(event.clone()))
                }
                JournalEntry::AcceptedUse(receipt, proof) => {
                    entries.push(FreshnessReplayEntry::AcceptedUse(receipt.clone(), *proof));
                }
            }
        }
        entries
    }

    /// Return the accepted Freshness evidence that is authoritative at an
    /// exact historical event cut.
    pub fn replay_entries_at(&self, cut: &FreshnessEventCut) -> Vec<FreshnessReplayEntry> {
        let owner = self.owner_uuid.unwrap_or([0; 16]);
        self.replay_entries()
            .into_iter()
            .filter(|entry| match entry {
                FreshnessReplayEntry::Birth { accepted_turn, .. }
                | FreshnessReplayEntry::Admission { accepted_turn, .. } => {
                    *accepted_turn <= cut.rel_turn
                }
                FreshnessReplayEntry::Event(event) => event_in_cut(event, Some(cut)),
                FreshnessReplayEntry::AcceptedUse(receipt, proof) => {
                    accepted_use_events(owner, receipt, *proof, &self.policy)
                        .iter()
                        .all(|event| event_in_cut(event, Some(cut)))
                }
            })
            .collect()
    }

    pub fn accepted_events(&self) -> Vec<FreshnessEvent> {
        self.events.values().cloned().collect()
    }

    pub fn has_durable_birth(&self, memory_id: FreshnessMemoryId) -> bool {
        self.births.contains_key(&memory_id)
    }

    pub fn is_initialized(&self) -> bool {
        self.policy_persisted
    }

    fn ensure_ready_for(&mut self, container: &Container) -> Result<(), String> {
        self.bind_owner(container.owner_id())?;
        if self.identity != Some(container.identity()) {
            return Err("Freshness container identity does not match owner".into());
        }
        if !self.policy_persisted {
            return Err("Freshness owner policy must be persisted before activities".into());
        }
        Ok(())
    }

    fn append_entry(
        &mut self,
        container: &mut Container,
        entry: JournalEntry,
    ) -> Result<(), String> {
        let owner = self
            .owner_uuid
            .ok_or("Freshness owner identity is required")?;
        let payload = encode_entry(owner, &entry)?;
        self.append_then_ingest(container, payload)
    }

    fn append_then_ingest(
        &mut self,
        container: &mut Container,
        payload: Vec<u8>,
    ) -> Result<(), String> {
        container.append(&payload).map_err(display_error)?;
        container.sync().map_err(display_error)?;
        self.ingest_payload(&payload)
    }

    fn validate_entry(&self, entry: &JournalEntry) -> Result<(), String> {
        match entry {
            JournalEntry::Birth {
                memory_id,
                accepted_turn,
                admitted_at_birth: _,
            } => {
                if self.births.contains_key(memory_id) {
                    return Err("Freshness birth already exists".into());
                }
                let _ = accepted_turn;
            }
            JournalEntry::Admission {
                memory_id,
                accepted_turn,
            } => {
                let (birth_turn, admitted_at_birth) = self
                    .births
                    .get(memory_id)
                    .copied()
                    .ok_or("Freshness admission has no accepted birth")?;
                if *accepted_turn < birth_turn {
                    return Err("Freshness admission precedes accepted birth".into());
                }
                if admitted_at_birth || self.admissions.contains_key(memory_id) {
                    return Err("Freshness admission cannot be reanchored".into());
                }
            }
            JournalEntry::Event(event) => self.validate_event(event)?,
            JournalEntry::AcceptedUse(receipt, proof) => {
                proof.validate()?;
                if receipt.use_id.trim().is_empty()
                    || receipt.policy_version != u64::from(FRESHNESS_POLICY_VERSION)
                    || receipt.sources.is_empty()
                    || receipt.sources.windows(2).any(|pair| pair[0] >= pair[1])
                    || self.uses.contains_key(&receipt.use_id)
                {
                    return Err("invalid or duplicate accepted-use receipt".into());
                }
                let owner = self
                    .owner_uuid
                    .ok_or("Freshness owner identity is required")?;
                for source in &receipt.sources {
                    let (birth, _) = self
                        .births
                        .get(source)
                        .ok_or("accepted-use receipt references unenrolled Memory")?;
                    if *birth > receipt.accepted_turn {
                        return Err("accepted-use receipt precedes Memory birth".into());
                    }
                    let id = accepted_event_id(owner, &receipt.use_id, *source);
                    if self.events.contains_key(&id) {
                        return Err("accepted-use source event collides with existing event".into());
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_event(&self, event: &FreshnessEvent) -> Result<(), String> {
        event.proof.validate()?;
        let owner = self
            .owner_uuid
            .ok_or("Freshness owner identity is required")?;
        if event.event_id.trim().is_empty()
            || event.provenance.source_id.trim().is_empty()
            || event.provenance.origin_owner != owner
            || event.provenance.accepted_turn != event.accepted_turn
            || event.provenance.policy_version != u64::from(FRESHNESS_POLICY_VERSION)
            || event.provenance.producer_kind != 1
            || event.effects.is_empty()
            || event
                .effects
                .values()
                .any(|amount| *amount <= 0 || *amount > self.policy.linkage_principal)
        {
            return Err("invalid owner-scoped Freshness event evidence".into());
        }
        let source_prefix = format!("dream-initial:{}:", hex(&owner));
        if event.provenance.source_id != event.event_id
            || !event.event_id.starts_with(&source_prefix)
            || event.event_id.len() != source_prefix.len() + 64
            || !event.event_id[source_prefix.len()..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("Freshness activity lacks an owner-bound Dream source ID".into());
        }
        let source_id = decode_hex_32(&event.event_id[source_prefix.len()..])?;
        if !self
            .births
            .get(&FreshnessMemoryId(source_id))
            .is_some_and(|(birth, _)| *birth <= event.accepted_turn)
            || !event
                .effects
                .values()
                .any(|amount| *amount == self.policy.linkage_principal)
        {
            return Err("Freshness activity lacks its enrolled Dream source principal".into());
        }
        for memory_id in event.effects.keys() {
            let (birth_turn, _) = self
                .births
                .get(memory_id)
                .ok_or("Freshness event references unenrolled Memory")?;
            if *birth_turn > event.accepted_turn {
                return Err("Freshness event precedes Memory birth".into());
            }
        }
        if let Some(existing) = self.events.get(&event.event_id) {
            return Err(if existing == event {
                "duplicate Freshness event in owner history".into()
            } else {
                "Freshness event identity conflict".into()
            });
        }
        Ok(())
    }

    fn apply_entry(&mut self, entry: JournalEntry) -> Result<(), String> {
        match &entry {
            JournalEntry::Birth {
                memory_id,
                accepted_turn,
                admitted_at_birth,
            } => {
                if let Some(old) = self.births.get(memory_id) {
                    return if old == &(*accepted_turn, *admitted_at_birth) {
                        Ok(())
                    } else {
                        Err("conflicting Freshness birth".into())
                    };
                }
                self.births
                    .insert(*memory_id, (*accepted_turn, *admitted_at_birth));
            }
            JournalEntry::Admission {
                memory_id,
                accepted_turn,
            } => {
                if let Some(old) = self.admissions.get(memory_id) {
                    return if old == accepted_turn {
                        Ok(())
                    } else {
                        Err("conflicting Freshness admission".into())
                    };
                }
                self.admissions.insert(*memory_id, *accepted_turn);
            }
            JournalEntry::Event(event) => {
                if let Some(old) = self.events.get(&event.event_id) {
                    return if old == event {
                        Ok(())
                    } else {
                        Err("conflicting Freshness event".into())
                    };
                }
                self.events.insert(event.event_id.clone(), event.clone());
            }
            JournalEntry::AcceptedUse(receipt, proof) => {
                if let Some((old, old_proof)) = self.uses.get(&receipt.use_id) {
                    return if old == receipt && old_proof == proof {
                        Ok(())
                    } else {
                        Err("conflicting accepted-use receipt".into())
                    };
                }
                self.uses
                    .insert(receipt.use_id.clone(), (receipt.clone(), *proof));
                let owner = self
                    .owner_uuid
                    .ok_or("Freshness owner identity is required")?;
                for source in &receipt.sources {
                    let event = FreshnessEvent {
                        event_id: accepted_event_id(owner, &receipt.use_id, *source),
                        accepted_turn: receipt.accepted_turn,
                        provenance: FreshnessEventProvenance {
                            producer_kind: 2,
                            source_id: receipt.use_id.clone(),
                            origin_owner: owner,
                            accepted_turn: receipt.accepted_turn,
                            policy_version: receipt.policy_version,
                        },
                        proof: *proof,
                        effects: [(*source, self.policy.access_principal)]
                            .into_iter()
                            .collect(),
                    };
                    self.events.insert(event.event_id.clone(), event);
                }
            }
        }
        self.journal.push(entry);
        Ok(())
    }

    fn apply_historical_snapshot(
        &mut self,
        snapshot: HistoricalFreshnessSnapshot,
    ) -> Result<(), String> {
        let owner = self
            .owner_uuid
            .ok_or("historical Freshness snapshot precedes owner policy")?;
        if snapshot.owner_uuid != owner
            || snapshot.journal_entries_included != self.journal.len() as u64
            || self
                .historical_snapshots
                .last()
                .is_some_and(|previous| previous.settlement_cut > snapshot.settlement_cut)
        {
            return Err("historical Freshness snapshot owner or journal boundary mismatch".into());
        }
        if let Some(existing) = self
            .historical_snapshots
            .iter()
            .find(|known| known.import_id == snapshot.import_id)
        {
            return if existing == &snapshot {
                Ok(())
            } else {
                Err("conflicting historical Freshness snapshot in owner history".into())
            };
        }
        let current = self.project(None)?;
        if snapshot.records.len() != current.len()
            || snapshot.records.iter().any(|(id, record)| {
                current
                    .get(id)
                    .is_none_or(|known| known.admitted_at_turn != record.admitted_at_turn)
            })
        {
            return Err("historical Freshness snapshot changes owner enrollment".into());
        }
        self.historical_snapshots.push(snapshot);
        Ok(())
    }

    fn project(
        &self,
        cut: Option<&FreshnessEventCut>,
    ) -> Result<BTreeMap<FreshnessMemoryId, FreshnessRecord>, String> {
        #[derive(Clone)]
        enum Action {
            Birth(FreshnessMemoryId, bool),
            Admission(FreshnessMemoryId),
            Event(FreshnessEvent),
        }
        let selected = self.historical_snapshots.iter().rev().find(|snapshot| {
            cut.is_none_or(|cut| {
                snapshot.settlement_cut < cut.rel_turn
                    || (snapshot.settlement_cut == cut.rel_turn && cut.event_id_inclusive.is_some())
            })
        });
        let mut records = selected
            .map(|snapshot| snapshot.records.clone())
            .unwrap_or_default();
        let journal_start = selected
            .map(|snapshot| snapshot.journal_entries_included as usize)
            .unwrap_or(0);
        let owner = self
            .owner_uuid
            .ok_or("Freshness owner identity is required")?;
        let mut actions: Vec<(u64, u8, String, Action)> = Vec::new();
        for entry in &self.journal[journal_start..] {
            match entry {
                JournalEntry::Birth {
                    memory_id,
                    accepted_turn,
                    admitted_at_birth,
                } if cut.is_none_or(|cut| *accepted_turn <= cut.rel_turn) => actions.push((
                    *accepted_turn,
                    0,
                    String::new(),
                    Action::Birth(*memory_id, *admitted_at_birth),
                )),
                JournalEntry::Admission {
                    memory_id,
                    accepted_turn,
                } if cut.is_none_or(|cut| *accepted_turn <= cut.rel_turn) => actions.push((
                    *accepted_turn,
                    1,
                    String::new(),
                    Action::Admission(*memory_id),
                )),
                JournalEntry::Event(event) if event_in_cut(event, cut) => actions.push((
                    event.accepted_turn,
                    2,
                    event.event_id.clone(),
                    Action::Event(event.clone()),
                )),
                JournalEntry::AcceptedUse(receipt, proof) => {
                    for event in accepted_use_events(owner, receipt, *proof, &self.policy) {
                        if event_in_cut(&event, cut) {
                            actions.push((
                                event.accepted_turn,
                                2,
                                event.event_id.clone(),
                                Action::Event(event),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        actions.sort_by(|a, b| (&a.0, &a.1, &a.2).cmp(&(&b.0, &b.1, &b.2)));
        for (_, _, _, action) in actions {
            match action {
                Action::Birth(id, admitted) => {
                    let mut record = FreshnessRecord::created();
                    if admitted {
                        let birth_turn = self.births[&id].0;
                        record
                            .admit(birth_turn)
                            .map_err(|error| error.to_string())?;
                    }
                    records.insert(id, record);
                }
                Action::Admission(id) => {
                    let turn = self.admissions[&id];
                    records
                        .get_mut(&id)
                        .ok_or("Freshness admission precedes birth")?
                        .admit(turn)
                        .map_err(|error| error.to_string())?;
                }
                Action::Event(event) => {
                    for (id, amount) in &event.effects {
                        let record = records
                            .get_mut(id)
                            .ok_or("Freshness event precedes accepted birth")?;
                        record
                            .reinforce_with_policy(*amount, event.accepted_turn, &self.policy)
                            .map_err(|error| error.to_string())?;
                    }
                }
            }
        }
        if let Some(cut) = cut {
            for record in records.values_mut() {
                record
                    .settle_with_policy(cut.rel_turn, &self.policy)
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(records)
    }
}

fn accepted_event_id(owner: [u8; 16], use_id: &str, source: FreshnessMemoryId) -> String {
    let owner_hex = hex(&owner);
    let use_hex = hex(use_id.as_bytes());
    format!("accepted-use:{owner_hex}:{use_hex}:{}", hex(&source.0))
}

fn accepted_use_events(
    owner: [u8; 16],
    receipt: &FreshnessUseReceipt,
    proof: FreshnessEventProof,
    policy: &FreshnessPolicy,
) -> Vec<FreshnessEvent> {
    receipt
        .sources
        .iter()
        .map(|source| FreshnessEvent {
            event_id: accepted_event_id(owner, &receipt.use_id, *source),
            accepted_turn: receipt.accepted_turn,
            provenance: FreshnessEventProvenance {
                producer_kind: 2,
                source_id: receipt.use_id.clone(),
                origin_owner: owner,
                accepted_turn: receipt.accepted_turn,
                policy_version: receipt.policy_version,
            },
            proof,
            effects: [(*source, policy.access_principal)].into_iter().collect(),
        })
        .collect()
}

fn event_in_cut(event: &FreshnessEvent, cut: Option<&FreshnessEventCut>) -> bool {
    cut.is_none_or(|cut| {
        event.accepted_turn < cut.rel_turn
            || (event.accepted_turn == cut.rel_turn
                && cut
                    .event_id_inclusive
                    .as_ref()
                    .is_some_and(|limit| event.event_id <= *limit))
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hex_32(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64 {
        return Err("invalid Dream source identity length".into());
    }
    let mut bytes = [0; 32];
    for (index, output) in bytes.iter_mut().enumerate() {
        *output = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| "invalid Dream source identity")?;
    }
    Ok(bytes)
}

fn display_error(error: ContainerError) -> String {
    error.to_string()
}
