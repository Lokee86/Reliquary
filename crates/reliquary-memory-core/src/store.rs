#[path = "dream_lifecycle.rs"]
mod dream_lifecycle;
pub use dream_lifecycle::GroupedMemoryLifecycle;
#[path = "initial_promotion.rs"]
mod initial_promotion;
use crate::routing::{MemoryRoutingMetadata, ROUTING_MARKER, decode_routing_metadata};
use crate::{codec::*, decode_grouped_creation, error::MemoryError, model::*};
pub use initial_promotion::GroupedMemoryInitialPromotion;
#[path = "payload.rs"]
mod payload;
use payload::{BodyEncoding, StoredBody};
use reliquary_container::{Container, ContainerIdentity, LeasedFrameReader, ObjectRef};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read;

pub type GroupedMemoryReplay = (Vec<(MemoryBodyId, Vec<u8>)>, Vec<Vec<u8>>);
type RetainedGroupedMemoryReplay = (
    Vec<(MemoryBodyId, Vec<u8>)>,
    Vec<Vec<u8>>,
    Vec<MemoryRoutingMetadata>,
);
type IndexedGroupedMemoryReplay = (
    Vec<RetainedGroupedBody>,
    Vec<Vec<u8>>,
    Vec<MemoryRoutingMetadata>,
);

/// The byte range validated by the owner of a grouped Memory carrier.
pub struct RetainedGroupedBody {
    pub id: MemoryBodyId,
    pub bytes: Vec<u8>,
    pub payload_offset: Option<u64>,
}

/// Start a detached replay, a leased replay, or continue beyond an admitted cut.
pub enum MemoryReplayStart {
    Detached,
    Leased,
    Checkpoint {
        store: MemoryStore,
        covered_end: u64,
    },
}

/// Owner-session proof accompanying an atomic Dream lifecycle event.
pub struct GroupedMemorySupersession {
    pub older: MemoryId,
    pub newer: MemoryId,
    pub previous_revision: u64,
    pub updated_at_ns: i64,
    pub mutation_id: String,
    pub delta: Vec<u8>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct State {
    revisions: BTreeMap<MemoryId, Vec<MemoryRecord>>,
    bodies: HashMap<MemoryBodyId, StoredBody>,
    body_memories: HashMap<MemoryBodyId, BTreeSet<MemoryId>>,
    mutations: HashMap<String, (MemoryId, usize)>,
    tombstones: HashMap<MemoryId, TerminalTombstone>,
    changes: Vec<MemoryId>,
    next_memory_version: u64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            revisions: BTreeMap::new(),
            bodies: HashMap::new(),
            body_memories: HashMap::new(),
            mutations: HashMap::new(),
            tombstones: HashMap::new(),
            changes: Vec::new(),
            next_memory_version: 1,
        }
    }
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MemoryStore {
    state: State,
    pruned_bodies: BTreeSet<MemoryBodyId>,
    routing: BTreeMap<MemoryId, MemoryRoutingMetadata>,
    initialized: bool,
    owner: Option<([u8; 16], ContainerIdentity)>,
    #[serde(skip)]
    reader: Option<LeasedFrameReader>,
}

fn memory_replay_frame(prefix: &[u8]) -> bool {
    if prefix.starts_with(b"RELPUB") {
        // The container admits the full clock stream; keep invalid envelopes
        // visible to the normal decoder and read only Memory publications.
        return match reliquary_container::decode_publication(prefix) {
            Ok(Some(publication)) => [BODY, CREATION, DELTA, TOMBSTONE]
                .iter()
                .any(|marker| publication.body.starts_with(*marker)),
            _ => true,
        };
    }
    [
        FORMAT.as_slice(),
        BODY,
        CREATION,
        DELTA,
        TOMBSTONE,
        b"RELMRTE",
        b"RELINSC7",
        b"RELINSC8",
        b"RELMCOR",
        b"RELMRET",
        b"RELDRB",
        b"RELDRF",
        b"RELDRS",
    ]
    .iter()
    .any(|marker| prefix.starts_with(*marker))
}

#[path = "management_group.rs"]
mod management_group;
pub use management_group::{GroupedManagementEffect, PreparedMemoryCorrection};

impl MemoryStore {
    pub fn create(container: &mut Container) -> Result<Self, MemoryError> {
        let mut cursor = container.physical_cursor()?;
        while let Some(mut frame) = cursor.next_frame()? {
            if frame.object().length < 8 {
                continue;
            }
            let mut prefix = [0; 40];
            let used = frame.object().length.min(prefix.len() as u64) as usize;
            frame.read_exact(&mut prefix[..used])?;
            let magic: [u8; 8] = prefix[..8].try_into().unwrap();
            if magic.starts_with(b"RELPUB") {
                let len = usize::try_from(frame.object().length)
                    .map_err(|_| MemoryError::FieldTooLarge)?;
                if len > 128 * 1024 * 1024 {
                    return Err(MemoryError::FieldTooLarge);
                }
                if let Some(envelope) = reliquary_container::decode_publication(&prefix[..used])?
                    && [BODY, CREATION, DELTA, TOMBSTONE]
                        .iter()
                        .any(|marker| envelope.body.starts_with(*marker))
                {
                    return Err(MemoryError::Corrupt(
                        "Memory owner already contains records",
                    ));
                }
            }
            if magic == *FORMAT
                || magic == *BODY
                || magic == *CREATION
                || magic == *DELTA
                || magic == *TOMBSTONE
                || magic == *ROUTING_MARKER
                || matches!(&magic, b"RELINSC7" | b"RELINSC8")
                || magic.starts_with(b"RELMCOR")
                || magic.starts_with(b"RELMRET")
            {
                return Err(MemoryError::Corrupt(
                    "Memory owner already contains records",
                ));
            }
        }
        drop(cursor);
        container.append(FORMAT)?;
        Ok(Self {
            initialized: true,
            owner: Some((container.owner_id(), container.identity())),
            ..Self::default()
        })
    }

    pub fn open(container: &Container) -> Result<Self, MemoryError> {
        Self::open_with_grouped_completion(container, |_| {
            Err(MemoryError::Corrupt(
                "grouped completion requires owner-session replay",
            ))
        })
    }

    /// Replay grouped Insomnia births at their physical position in the owner
    /// stream. The caller owns the grouped carrier codec; Memory owns the
    /// body and creation invariants.
    pub fn open_with_grouped_completion(
        container: &Container,
        decode_grouped: impl FnMut(&[u8]) -> Result<Option<GroupedMemoryReplay>, MemoryError>,
    ) -> Result<Self, MemoryError> {
        Self::open_with_grouped_events(container, decode_grouped, |_| {
            Err(MemoryError::Corrupt(
                "Dream lifecycle requires owner-session replay",
            ))
        })
    }

    /// Owner-session admits a grouped Dream lifecycle delta at its exact
    /// physical position. Memory reuses normal revision and clock validation.
    pub fn open_with_grouped_events(
        container: &Container,
        decode_grouped: impl FnMut(&[u8]) -> Result<Option<GroupedMemoryReplay>, MemoryError>,
        decode_dream: impl FnMut(&[u8]) -> Result<Option<GroupedMemorySupersession>, MemoryError>,
    ) -> Result<Self, MemoryError> {
        Self::open_with_grouped_initial_events(container, decode_grouped, decode_dream, |_| {
            Err(MemoryError::Corrupt(
                "initial Dream requires owner-session replay",
            ))
        })
    }

    pub fn open_with_grouped_initial_events(
        container: &Container,
        grouped: impl FnMut(&[u8]) -> Result<Option<GroupedMemoryReplay>, MemoryError>,
        dream: impl FnMut(&[u8]) -> Result<Option<GroupedMemorySupersession>, MemoryError>,
        initial: impl FnMut(&[u8]) -> Result<Option<GroupedMemoryInitialPromotion>, MemoryError>,
    ) -> Result<Self, MemoryError> {
        Self::open_with_dream_batches(container, grouped, dream, initial, |_| {
            Err(MemoryError::Corrupt("Dream batch requires owner replay"))
        })
    }
    pub fn open_with_dream_batches(
        container: &Container,
        decode_grouped: impl FnMut(&[u8]) -> Result<Option<GroupedMemoryReplay>, MemoryError>,
        decode_dream: impl FnMut(&[u8]) -> Result<Option<GroupedMemorySupersession>, MemoryError>,
        decode_initial: impl FnMut(&[u8]) -> Result<Option<GroupedMemoryInitialPromotion>, MemoryError>,
        decode_batch: impl FnMut(&[u8]) -> Result<Vec<GroupedMemoryLifecycle>, MemoryError>,
    ) -> Result<Self, MemoryError> {
        Self::open_with_management_groups(
            container,
            decode_grouped,
            decode_dream,
            decode_initial,
            decode_batch,
            |_| {
                Err(MemoryError::Corrupt(
                    "grouped management requires owner replay",
                ))
            },
        )
    }

    pub fn open_with_management_groups(
        container: &Container,
        mut decode_grouped: impl FnMut(&[u8]) -> Result<Option<GroupedMemoryReplay>, MemoryError>,
        decode_dream: impl FnMut(&[u8]) -> Result<Option<GroupedMemorySupersession>, MemoryError>,
        decode_initial: impl FnMut(&[u8]) -> Result<Option<GroupedMemoryInitialPromotion>, MemoryError>,
        decode_batch: impl FnMut(&[u8]) -> Result<Vec<GroupedMemoryLifecycle>, MemoryError>,
        decode_management: impl FnMut(&[u8]) -> Result<GroupedManagementEffect, MemoryError>,
    ) -> Result<Self, MemoryError> {
        Self::open_with_retained_dream_batches(
            container,
            |bytes| {
                Ok(decode_grouped(bytes)?.map(|(bodies, records)| (bodies, records, Vec::new())))
            },
            decode_dream,
            decode_initial,
            decode_batch,
            decode_management,
        )
    }

    pub fn open_with_retained_dream_batches(
        container: &Container,
        mut decode_grouped: impl FnMut(
            &[u8],
        )
            -> Result<Option<RetainedGroupedMemoryReplay>, MemoryError>,
        decode_dream: impl FnMut(&[u8]) -> Result<Option<GroupedMemorySupersession>, MemoryError>,
        decode_initial: impl FnMut(&[u8]) -> Result<Option<GroupedMemoryInitialPromotion>, MemoryError>,
        decode_batch: impl FnMut(&[u8]) -> Result<Vec<GroupedMemoryLifecycle>, MemoryError>,
        decode_management: impl FnMut(&[u8]) -> Result<GroupedManagementEffect, MemoryError>,
    ) -> Result<Self, MemoryError> {
        Self::replay_retained_dream_batches(
            container,
            MemoryReplayStart::Detached,
            |bytes| {
                Ok(decode_grouped(bytes)?
                    .map(|(bodies, records, routing)| (unindexed_bodies(bodies), records, routing)))
            },
            decode_dream,
            decode_initial,
            decode_batch,
            decode_management,
        )
    }

    /// Rebind the authenticated state to this file lease before lazy queries.
    pub fn bind_checkpoint_reader(&mut self, container: &Container) -> Result<(), MemoryError> {
        self.validate_owner(container)?;
        self.reader = Some(container.leased_frame_reader()?);
        Ok(())
    }

    pub fn replay_retained_dream_batches(
        container: &Container,
        start: MemoryReplayStart,
        mut decode_grouped: impl FnMut(&[u8]) -> Result<Option<IndexedGroupedMemoryReplay>, MemoryError>,
        mut decode_dream: impl FnMut(&[u8]) -> Result<Option<GroupedMemorySupersession>, MemoryError>,
        mut decode_initial: impl FnMut(
            &[u8],
        )
            -> Result<Option<GroupedMemoryInitialPromotion>, MemoryError>,
        mut decode_batch: impl FnMut(&[u8]) -> Result<Vec<GroupedMemoryLifecycle>, MemoryError>,
        mut decode_management: impl FnMut(&[u8]) -> Result<GroupedManagementEffect, MemoryError>,
    ) -> Result<Self, MemoryError> {
        let empty = || Self {
            owner: Some((container.owner_id(), container.identity())),
            ..Self::default()
        };
        let (mut store, mut cursor, cold) = match start {
            MemoryReplayStart::Detached => (
                empty(),
                container.matching_physical_cursor(memory_replay_frame)?,
                true,
            ),
            MemoryReplayStart::Leased => {
                let mut store = empty();
                store.bind_checkpoint_reader(container)?;
                (
                    store,
                    container.matching_physical_cursor(memory_replay_frame)?,
                    true,
                )
            }
            MemoryReplayStart::Checkpoint {
                mut store,
                covered_end,
            } => {
                store.bind_checkpoint_reader(container)?;
                (store, container.physical_cursor_from(covered_end)?, false)
            }
        };
        if cold && container.max_frame_length()? > 128 * 1024 * 1024 {
            return Err(MemoryError::FieldTooLarge);
        }
        while let Some(mut frame) = cursor.next_frame()? {
            let object = frame.object();
            let len = usize::try_from(object.length).map_err(|_| MemoryError::FieldTooLarge)?;
            if len > 128 * 1024 * 1024 {
                return Err(MemoryError::FieldTooLarge);
            }
            if len < 8 {
                continue;
            }
            let mut magic = [0; 8];
            frame.read_exact(&mut magic)?;
            if magic.starts_with(b"RELPUB") {
                let mut payload = Vec::with_capacity(len);
                payload.extend_from_slice(&magic);
                frame.read_to_end(&mut payload)?;
                if let Some(envelope) = reliquary_container::decode_publication(&payload)?
                    && [BODY, CREATION, DELTA, TOMBSTONE]
                        .iter()
                        .any(|marker| envelope.body.starts_with(*marker))
                {
                    if !store.initialized {
                        return Err(MemoryError::Corrupt(
                            "Memory record precedes format declaration",
                        ));
                    }
                    if envelope.body.len() > MAX_BODY_BYTES + MAX_FIELD_BYTES {
                        return Err(MemoryError::FieldTooLarge);
                    }
                    let offset = (payload.len() - envelope.body.len()) as u64;
                    store.replay_at(envelope.body, Some((object, offset)))?;
                }
                continue;
            }
            if magic == *FORMAT {
                if len != 8 || store.initialized {
                    return Err(MemoryError::Corrupt("duplicate or malformed format record"));
                }
                store.initialized = true;
                continue;
            }
            if magic.starts_with(b"RELMRTE") {
                if len > 64 * 1024 {
                    return Err(MemoryError::FieldTooLarge);
                }
                let mut payload = Vec::with_capacity(len);
                payload.extend_from_slice(&magic);
                frame.read_to_end(&mut payload)?;
                store.apply_routing(decode_routing_metadata(&payload)?)?;
                continue;
            }
            if matches!(&magic, b"RELINSC7" | b"RELINSC8") {
                let mut payload = Vec::with_capacity(len);
                payload.extend_from_slice(&magic);
                frame.read_to_end(&mut payload)?;
                if let Some((bodies, records, routing)) = decode_grouped(&payload)? {
                    // Owner replay validates any missing grouped payload only
                    // after it has admitted the exact terminal-prune manifest.
                    store = store.with_grouped_completion(
                        bodies,
                        records,
                        Some((object, &payload)),
                        true,
                    )?;
                    for route in routing {
                        store.apply_routing(route)?;
                    }
                }
                continue;
            }
            if magic.starts_with(b"RELMCOR") || magic.starts_with(b"RELMRET") {
                let mut payload = magic.to_vec();
                frame.read_to_end(&mut payload)?;
                store.apply_management_effect(decode_management(&payload)?)?;
                continue;
            }
            if magic.starts_with(b"RELDRB") {
                let mut payload = magic.to_vec();
                frame.read_to_end(&mut payload)?;
                for event in decode_batch(&payload)? {
                    store.apply_dream_lifecycle(&event)?;
                }
                continue;
            }
            if magic.starts_with(b"RELDRF") {
                let mut payload = magic.to_vec();
                frame.read_to_end(&mut payload)?;
                if let Some(record) = decode_initial(&payload)? {
                    store.apply_initial_promotion(&record)?;
                }
                continue;
            }
            if magic.starts_with(b"RELDRS") {
                let mut payload = Vec::with_capacity(len);
                payload.extend_from_slice(&magic);
                frame.read_to_end(&mut payload)?;
                let record = decode_dream(&payload)?
                    .ok_or(MemoryError::Corrupt("unrecognized grouped Dream lifecycle"))?;
                store.apply_grouped_supersession(&record)?;
                continue;
            }
            if magic != *BODY && magic != *CREATION && magic != *DELTA && magic != *TOMBSTONE {
                continue;
            }
            if !store.initialized {
                return Err(MemoryError::Corrupt(
                    "Memory record precedes format declaration",
                ));
            }
            if len > MAX_BODY_BYTES + MAX_FIELD_BYTES {
                return Err(MemoryError::FieldTooLarge);
            }
            let mut payload = Vec::with_capacity(len);
            payload.extend_from_slice(&magic);
            frame.read_to_end(&mut payload)?;
            store.replay_at(&payload, Some((object, 0)))?;
        }
        if cold {
            store.validate_backed_records()?;
        }
        Ok(store)
    }

    pub fn routing_metadata(&self, memory_id: MemoryId) -> Option<&MemoryRoutingMetadata> {
        self.routing.get(&memory_id)
    }

    fn apply_routing(&mut self, route: MemoryRoutingMetadata) -> Result<(), MemoryError> {
        let record = self
            .state
            .revisions
            .get(&route.memory_id)
            .and_then(|revisions| revisions.first())
            .ok_or(MemoryError::Corrupt(
                "Memory routing precedes its Memory identity",
            ))?;
        if record.body_id != route.body_id {
            return Err(MemoryError::Corrupt("Memory routing body identity differs"));
        }
        let body = self.load_body(route.body_id)?.ok_or(MemoryError::Corrupt(
            "Memory routing requires the original body",
        ))?;
        route.validate_body(&body)?;
        if let Some(existing) = self.routing.get(&route.memory_id) {
            if existing != &route {
                return Err(MemoryError::Corrupt("conflicting Memory routing metadata"));
            }
        } else {
            self.routing.insert(route.memory_id, route);
        }
        Ok(())
    }

    pub fn preflight_grouped_completion(
        &self,
        bodies: Vec<(MemoryBodyId, Vec<u8>)>,
        records: Vec<Vec<u8>>,
    ) -> Result<(), MemoryError> {
        self.clone()
            .with_grouped_completion(unindexed_bodies(bodies), records, None, false)
            .map(drop)
    }

    pub fn apply_grouped_completion(
        &mut self,
        bodies: Vec<(MemoryBodyId, Vec<u8>)>,
        records: Vec<Vec<u8>>,
    ) -> Result<(), MemoryError> {
        *self =
            self.clone()
                .with_grouped_completion(unindexed_bodies(bodies), records, None, false)?;
        Ok(())
    }

    pub fn apply_indexed_grouped_completion(
        &mut self,
        object: ObjectRef,
        payload: &[u8],
        bodies: Vec<RetainedGroupedBody>,
        records: Vec<Vec<u8>>,
    ) -> Result<(), MemoryError> {
        *self = self.clone().with_grouped_completion(
            bodies,
            records,
            Some((object, payload)),
            false,
        )?;
        Ok(())
    }

    // Replay owns this candidate. A live caller supplies a snapshot so a
    // rejected group cannot expose partial changes to its existing store.
    fn with_grouped_completion(
        mut self,
        bodies: Vec<RetainedGroupedBody>,
        records: Vec<Vec<u8>>,
        frame: Option<(ObjectRef, &[u8])>,
        allow_missing_body_for_owner_admission: bool,
    ) -> Result<Self, MemoryError> {
        if !records.is_empty() && !self.initialized {
            return Err(MemoryError::Corrupt(
                "grouped creation precedes Memory format",
            ));
        }
        let mut seen_bodies = std::collections::BTreeSet::new();
        let mut provided_bodies = std::collections::BTreeSet::new();
        for RetainedGroupedBody {
            id,
            bytes,
            payload_offset,
        } in bodies
        {
            self.require_unpruned_body(id)?;
            if !seen_bodies.insert(id) {
                return Err(MemoryError::Corrupt("duplicate grouped body identity"));
            }
            let body = decode_grouped_body(id, &bytes)?;
            provided_bodies.insert(id);
            let location = match (self.reader.is_some(), frame, payload_offset) {
                (true, Some((object, payload)), Some(offset)) => {
                    let start = usize::try_from(offset).map_err(|_| MemoryError::FieldTooLarge)?;
                    let end = start
                        .checked_add(bytes.len())
                        .ok_or(MemoryError::FieldTooLarge)?;
                    if payload.get(start..end) != Some(bytes.as_slice()) {
                        return Err(MemoryError::Corrupt("grouped Memory body locator differs"));
                    }
                    Some((object, offset, BodyEncoding::Grouped))
                }
                _ => None,
            };
            self.insert_body(id, body, &bytes, location)?;
        }
        let mut used_provided_bodies = std::collections::BTreeSet::new();
        for bytes in records {
            let record = decode_grouped_creation(&bytes)?;
            self.require_unpruned_body(record.body_id)?;
            if record.source_episode_id.is_none() {
                return Err(MemoryError::Corrupt(
                    "grouped creation lacks Episode provenance",
                ));
            }
            if provided_bodies.contains(&record.body_id) {
                used_provided_bodies.insert(record.body_id);
            }
            self.replay(&bytes)?;
            if !allow_missing_body_for_owner_admission
                && !self.state.bodies.contains_key(&record.body_id)
            {
                return Err(MemoryError::Corrupt("grouped creation has no body backing"));
            }
        }
        if used_provided_bodies != provided_bodies {
            return Err(MemoryError::Corrupt(
                "grouped body is not referenced by a creation",
            ));
        }
        for id in &provided_bodies {
            self.validate_body_records(*id)?;
        }
        Ok(self)
    }

    /// Prepare an immutable-body metadata revision for an owner-approved
    /// supersession; all other fields retain their exact historical values.
    /// The standard delta codec and replay preflight own revision invariants.
    pub fn prepare_grouped_supersession(
        &self,
        id: MemoryId,
        expected_revision: u64,
        newer: MemoryId,
        mutation_id: String,
        updated_at_ns: i64,
    ) -> Result<Vec<u8>, MemoryError> {
        if id == newer || mutation_id.is_empty() {
            return Err(MemoryError::InvalidField("supersession_target"));
        }
        let newer_current = self
            .revision_record(newer, None)
            .map_err(|error| match error {
                MemoryError::TerminalTombstone => MemoryError::RevisionConflict,
                error => error,
            })?;
        if newer_current.archived || newer_current.superseded_by.is_some() {
            return Err(MemoryError::RevisionConflict);
        }
        let current = self.revision_record(id, None)?;
        if current.revision != expected_revision
            || current.archived
            || current.superseded_by.is_some()
            || current.updated_at_ns > updated_at_ns
        {
            return Err(MemoryError::RevisionConflict);
        }
        let mut next = current.clone();
        next.revision = next
            .revision
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        next.memory_version = self.state.next_memory_version;
        next.lifecycle_state = "archived".into();
        next.archived = true;
        next.superseded_by = Some(newer);
        next.mutation_id = mutation_id;
        next.updated_at_ns = updated_at_ns;
        let bytes = encode_delta(current, &next)?;
        self.preflight_grouped_lifecycle_delta(&bytes)?;
        Ok(bytes)
    }

    pub fn apply_grouped_supersession(
        &mut self,
        record: &GroupedMemorySupersession,
    ) -> Result<(), MemoryError> {
        let expected = self.prepare_grouped_supersession(
            record.older,
            record.previous_revision,
            record.newer,
            record.mutation_id.clone(),
            record.updated_at_ns,
        )?;
        if expected != record.delta {
            return Err(MemoryError::Corrupt(
                "Dream lifecycle delta changes unrelated metadata",
            ));
        }
        self.apply_grouped_lifecycle_delta(&record.delta)
    }

    pub fn preflight_grouped_lifecycle_delta(&self, bytes: &[u8]) -> Result<(), MemoryError> {
        let mut projection = self.clone();
        projection.apply_grouped_lifecycle_delta(bytes)
    }

    pub fn apply_grouped_lifecycle_delta(&mut self, bytes: &[u8]) -> Result<(), MemoryError> {
        if !bytes.starts_with(DELTA) || !self.initialized {
            return Err(MemoryError::Corrupt(
                "grouped Dream requires a Memory delta",
            ));
        }
        // Replay is all-or-nothing in memory, just as normal Memory revision
        // history replay. No physical append is performed by this method.
        let mut next = self.clone();
        next.replay(bytes)?;
        *self = next;
        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
    fn validate_backed_records(&self) -> Result<(), MemoryError> {
        for history in self.state.revisions.values() {
            for record in history {
                validate_replayed_record_with_body_len(record, self.body_len(record.body_id))?;
            }
        }
        Ok(())
    }
    fn validate_body_records(&self, body_id: MemoryBodyId) -> Result<(), MemoryError> {
        let body_len = self.body_len(body_id);
        for id in self.state.body_memories.get(&body_id).into_iter().flatten() {
            for record in &self.state.revisions[id] {
                validate_replayed_record_with_body_len(record, body_len)?;
            }
        }
        Ok(())
    }
    pub fn memory_version(&self) -> u64 {
        self.state.next_memory_version.saturating_sub(1)
    }
    pub fn memory_count(&self) -> usize {
        self.state.revisions.len()
    }
    pub fn memory_ids(&self) -> impl Iterator<Item = MemoryId> + '_ {
        self.state.revisions.keys().copied()
    }

    /// Source identities changed in an exact range of owner Memory versions.
    /// Includes terminal tombstones and never reads a body.
    pub fn changes_after(
        &self,
        after: u64,
        through: u64,
        limit: usize,
    ) -> impl Iterator<Item = (u64, MemoryId)> + '_ {
        let end = through.min(self.memory_version()) as usize;
        let start = after.min(end as u64) as usize;
        self.state.changes[start..end]
            .iter()
            .take(limit)
            .enumerate()
            .map(move |(offset, id)| ((start + offset + 1) as u64, *id))
    }
    /// Whether retained owner history names this body, even if its payload is unavailable.
    pub fn references_body(&self, body_id: MemoryBodyId) -> bool {
        self.state.body_memories.contains_key(&body_id)
    }

    /// Whether an admitted body has backing, without reading its payload.
    pub fn contains_body_payload(&self, id: MemoryBodyId) -> bool {
        self.state.bodies.contains_key(&id)
    }

    /// Bodies named by accepted history without a local payload.
    /// Transformed-copy admission uses these identities to validate pruning evidence.
    pub fn unbacked_body_ids(&self) -> impl Iterator<Item = MemoryBodyId> + '_ {
        self.state
            .body_memories
            .keys()
            .filter(|id| !self.state.bodies.contains_key(id))
            .copied()
    }
    /// Install the body identities verified by owner retention admission.
    pub fn retain_pruned_body_fence(&mut self, bodies: BTreeSet<MemoryBodyId>) {
        self.pruned_bodies.extend(bodies);
    }

    pub fn pruned_body_count(&self) -> usize {
        self.pruned_bodies.len()
    }

    fn require_unpruned_body(&self, id: MemoryBodyId) -> Result<(), MemoryError> {
        if self.pruned_bodies.contains(&id) {
            return Err(MemoryError::Corrupt("Memory body was terminally pruned"));
        }
        Ok(())
    }

    pub fn memory_ids_for_body(
        &self,
        body_id: MemoryBodyId,
    ) -> impl Iterator<Item = MemoryId> + '_ {
        self.state
            .body_memories
            .get(&body_id)
            .into_iter()
            .flatten()
            .copied()
    }
    pub fn contains_memory(&self, id: MemoryId) -> bool {
        self.state.revisions.contains_key(&id)
    }
    /// Owner-local version of the first retained creation, independent of payload availability.
    pub fn first_memory_version(&self, id: MemoryId) -> Option<u64> {
        self.state
            .revisions
            .get(&id)?
            .first()
            .map(|record| record.memory_version)
    }

    pub fn publish(
        &mut self,
        container: &mut Container,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
    ) -> Result<PublishResult, MemoryError> {
        self.publish_inner(container, id, expected_revision, draft, None, None)
    }

    /// Publish a creation with an owner global coordinate for Dream replay.
    pub fn publish_versioned_creation(
        &mut self,
        container: &mut Container,
        draft: MemoryDraft,
        transaction_time_ns: i64,
    ) -> Result<PublishResult, MemoryError> {
        self.publish_inner(container, None, 0, draft, Some(transaction_time_ns), None)
    }

    /// Write an ungrouped accepted creation or delta using its exact native coordinates.
    pub fn publish_accepted_versioned_revision(
        &mut self,
        container: &mut Container,
        global_version: u64,
        transaction_time_ns: i64,
        accepted: &MemoryRecord,
        draft: MemoryDraft,
    ) -> Result<(), MemoryError> {
        let previous = accepted
            .revision
            .checked_sub(1)
            .ok_or(MemoryError::RevisionConflict)?;
        if self.state.next_memory_version != accepted.memory_version
            || container.latest_version().checked_add(1) != Some(global_version)
            || self.state.mutations.contains_key(&accepted.mutation_id)
        {
            return Err(MemoryError::RevisionConflict);
        }
        let id = (previous != 0).then_some(accepted.id);
        self.publish_inner(
            container,
            id,
            previous,
            draft,
            Some(transaction_time_ns),
            Some(accepted),
        )?;
        Ok(())
    }

    fn publish_inner(
        &mut self,
        container: &mut Container,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
        versioned_at: Option<i64>,
        accepted: Option<&MemoryRecord>,
    ) -> Result<PublishResult, MemoryError> {
        self.validate_owner(container)?;
        validate_draft(&draft)?;
        if !self.initialized {
            return Err(MemoryError::Corrupt("Memory owner is not initialized"));
        }
        let target = id.unwrap_or_else(|| memory_id(&draft.mutation_id));
        let body_id = memory_body_id(&draft.title, &draft.content);
        self.require_unpruned_body(body_id)?;

        if let Some((seen_id, revision_index)) =
            self.state.mutations.get(&draft.mutation_id).copied()
        {
            if seen_id != target {
                return Err(MemoryError::MutationConflict);
            }
            let accepted = self
                .state
                .revisions
                .get(&seen_id)
                .and_then(|history| history.get(revision_index))
                .ok_or(MemoryError::Corrupt("mutation index is invalid"))?;
            let body = self.load_body(accepted.body_id)?;
            if accepted.body_id != body_id
                || body
                    .as_deref()
                    .is_none_or(|body| body.title != draft.title || body.content != draft.content)
                || !draft_matches(accepted, &draft)
            {
                return Err(MemoryError::MutationConflict);
            }
            return Ok(PublishResult {
                revision: revision_with_body(accepted, body.map(Cow::into_owned)),
                published: false,
            });
        }
        if self.state.tombstones.contains_key(&target) {
            return Err(MemoryError::TerminalTombstone);
        }

        let history = self.state.revisions.get(&target);
        let current = history.and_then(|history| history.last());
        let current_revision = current.map_or(0, |record| record.revision);
        if current_revision != expected_revision {
            return Err(MemoryError::RevisionConflict);
        }
        if let Some(current) = current {
            if current.body_id != body_id {
                return Err(MemoryError::ImmutableBodyChanged);
            }
            if !self.state.bodies.contains_key(&current.body_id) {
                return Err(MemoryError::Corrupt(
                    "cannot revise Memory with absent body backing",
                ));
            }
        }
        let revision = expected_revision
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        let memory_version = self.state.next_memory_version;
        let next_version = memory_version
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        let record = record_for_draft(&draft, current, target, body_id, revision, memory_version)?;
        if accepted.is_some_and(|expected| *expected != record) {
            return Err(MemoryError::RevisionConflict);
        }
        let body = MemoryBody {
            title: draft.title.clone(),
            content: draft.content.clone(),
        };
        let body_bytes = encode_body(body_id, &body)?;
        let encoded = if let Some(previous) = current {
            encode_delta(previous, &record)?
        } else {
            encode_creation(&record)?
        };
        let appended = if let Some(existing) = self.load_body(body_id)? {
            if existing.as_ref() != &body {
                return Err(MemoryError::HashCollision);
            }
            None
        } else {
            for id in self.state.body_memories.get(&body_id).into_iter().flatten() {
                for existing in &self.state.revisions[id] {
                    validate_replayed_record(existing, Some(&body))?;
                }
            }
            Some(container.append(&body_bytes)?)
        };
        if let Some(timestamp) = versioned_at {
            container.append_publication_envelope(&encoded, Some(timestamp))?;
        } else {
            container.append(&encoded)?;
        }
        if let Some(object) = appended {
            let location = self
                .reader
                .is_some()
                .then_some((object, 0, BodyEncoding::Standard));
            self.state.bodies.insert(
                body_id,
                StoredBody::new(body.clone(), &body_bytes, location),
            );
        }
        let history = self.state.revisions.entry(target).or_default();
        let revision_index = history.len();
        history.push(record.clone());
        self.state
            .body_memories
            .entry(body_id)
            .or_default()
            .insert(target);
        self.state
            .mutations
            .insert(draft.mutation_id, (target, revision_index));
        self.state.changes.push(target);
        self.state.next_memory_version = next_version;
        Ok(PublishResult {
            revision: revision_with_body(&record, Some(body)),
            published: true,
        })
    }

    fn validate_owner(&self, container: &Container) -> Result<(), MemoryError> {
        if self.owner == Some((container.owner_id(), container.identity())) {
            Ok(())
        } else {
            Err(MemoryError::WrongOwner)
        }
    }

    pub fn tombstone(
        &mut self,
        container: &mut Container,
        id: MemoryId,
        expected_revision: u64,
    ) -> Result<TerminalTombstone, MemoryError> {
        self.validate_owner(container)?;
        if let Some(tombstone) = self.state.tombstones.get(&id).copied() {
            if expected_revision.checked_add(1) == Some(tombstone.revision) {
                return Ok(tombstone);
            }
            return Err(MemoryError::RevisionConflict);
        }
        let current = self
            .state
            .revisions
            .get(&id)
            .and_then(|history| history.last())
            .ok_or(MemoryError::MissingMemory)?;
        if current.revision != expected_revision {
            return Err(MemoryError::RevisionConflict);
        }
        let revision = expected_revision
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        let memory_version = self.state.next_memory_version;
        let next = memory_version
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        container.append(&encode_tombstone(id, revision, memory_version))?;
        let tombstone = TerminalTombstone {
            revision,
            memory_version,
        };
        self.state.tombstones.insert(id, tombstone);
        self.state.changes.push(id);
        self.state.next_memory_version = next;
        Ok(tombstone)
    }

    /// Current selected revision without cloning the retained revision history.
    pub fn current(&self, id: MemoryId) -> Result<MemoryRevision, MemoryError> {
        self.revision_view(self.revision_record(id, None)?)
    }
    /// Canonical metadata at a domain-version cut, including tombstone exclusion.
    pub fn record_at_cut(&self, id: MemoryId, cut: u64) -> Option<&MemoryRecord> {
        if self
            .state
            .tombstones
            .get(&id)
            .is_some_and(|t| t.memory_version <= cut)
        {
            return None;
        }
        let history = self.state.revisions.get(&id)?;
        let at = history.partition_point(|r| r.memory_version <= cut);
        at.checked_sub(1).and_then(|at| history.get(at))
    }
    pub fn memory_ids_page(&self, after: Option<MemoryId>, limit: usize) -> Vec<MemoryId> {
        use std::ops::Bound::{Excluded, Unbounded};
        self.state
            .revisions
            .range((after.map_or(Unbounded, Excluded), Unbounded))
            .take(limit)
            .map(|(id, _)| *id)
            .collect()
    }
    pub fn history(&self, id: MemoryId) -> Result<MemoryHistory, MemoryError> {
        self.history_at_cut(id, u64::MAX)
    }

    pub fn history_at_cut(&self, id: MemoryId, cut: u64) -> Result<MemoryHistory, MemoryError> {
        let records = self
            .revision_records_at_cut(id, cut)
            .ok_or(MemoryError::MissingMemory)?;
        let first = records
            .first()
            .ok_or(MemoryError::Corrupt("empty Memory history"))?;
        let body = self.load_body(first.body_id)?;
        Ok(MemoryHistory {
            revisions: records
                .iter()
                .map(|record| revision_with_body(record, body.as_deref().cloned()))
                .collect(),
            tombstone: self.terminal_at_cut(id, cut),
        })
    }

    pub fn revision(&self, id: MemoryId, revision: u64) -> Result<MemoryRevision, MemoryError> {
        self.revision_view(self.revision_record(id, Some(revision))?)
    }

    pub fn revision_projection(
        &self,
        id: MemoryId,
        revision: Option<u64>,
    ) -> Result<(&MemoryRecord, Option<Cow<'_, MemoryBody>>), MemoryError> {
        let record = self.revision_record(id, revision)?;
        Ok((record, self.load_body(record.body_id)?))
    }

    /// Resolve revision metadata without page-loading its optional payload.
    pub fn revision_record(
        &self,
        id: MemoryId,
        revision: Option<u64>,
    ) -> Result<&MemoryRecord, MemoryError> {
        if revision.is_none() && self.state.tombstones.contains_key(&id) {
            return Err(MemoryError::TerminalTombstone);
        }
        let records = self
            .state
            .revisions
            .get(&id)
            .ok_or(MemoryError::MissingMemory)?;
        match revision {
            Some(revision) => records
                .binary_search_by_key(&revision, |record| record.revision)
                .ok()
                .and_then(|index| records.get(index)),
            None => records.last(),
        }
        .ok_or(MemoryError::MissingMemory)
    }

    /// Select retained revision metadata without cloning bodies outside the page.
    pub fn revision_records_at_cut(&self, id: MemoryId, cut: u64) -> Option<&[MemoryRecord]> {
        let records = self.state.revisions.get(&id)?;
        let end = records.partition_point(|record| record.memory_version <= cut);
        (end > 0).then_some(&records[..end])
    }

    pub fn terminal_at_cut(&self, id: MemoryId, cut: u64) -> Option<TerminalTombstone> {
        self.state
            .tombstones
            .get(&id)
            .copied()
            .filter(|tombstone| tombstone.memory_version <= cut)
    }

    pub fn retained_count(&self) -> usize {
        self.state.revisions.len()
    }

    pub fn current_body_available(&self, id: MemoryId) -> bool {
        self.record_at_cut(id, self.memory_version())
            .is_some_and(|record| self.contains_body_payload(record.body_id))
    }

    fn revision_view(&self, record: &MemoryRecord) -> Result<MemoryRevision, MemoryError> {
        let body = self.load_body(record.body_id)?.map(Cow::into_owned);
        Ok(revision_with_body(record, body))
    }

    fn replay(&mut self, bytes: &[u8]) -> Result<(), MemoryError> {
        self.replay_at(bytes, None)
    }

    fn replay_at(
        &mut self,
        bytes: &[u8],
        frame: Option<(ObjectRef, u64)>,
    ) -> Result<(), MemoryError> {
        if bytes.starts_with(BODY) {
            let (id, body) = decode_body(bytes)?;
            self.require_unpruned_body(id)?;
            if memory_body_id(&body.title, &body.content) != id {
                return Err(MemoryError::Corrupt("body digest mismatch"));
            }
            let location = if self.reader.is_some() {
                frame.map(|(object, offset)| (object, offset, BodyEncoding::Standard))
            } else {
                None
            };
            self.insert_body(id, body, bytes, location)?;
            self.validate_body_records(id)?;
            return Ok(());
        }
        if bytes.starts_with(CREATION) {
            let record = decode_creation(bytes)?;
            validate_replayed_record_with_body_len(&record, self.body_len(record.body_id))?;
            if record.revision != 1
                || record.memory_version != self.state.next_memory_version
                || record.mutation_id.is_empty()
                || memory_id(&record.mutation_id) != record.id
            {
                return Err(MemoryError::Corrupt("invalid creation coordinate"));
            }
            if self.state.revisions.contains_key(&record.id)
                || self.state.mutations.contains_key(&record.mutation_id)
            {
                return Err(MemoryError::Corrupt("duplicate Memory creation"));
            }
            self.state
                .mutations
                .insert(record.mutation_id.clone(), (record.id, 0));
            self.state
                .body_memories
                .entry(record.body_id)
                .or_default()
                .insert(record.id);
            self.state.changes.push(record.id);
            self.state.revisions.insert(record.id, vec![record]);
            self.state.next_memory_version = self
                .state
                .next_memory_version
                .checked_add(1)
                .ok_or(MemoryError::VersionExhausted)?;
            return Ok(());
        }
        if bytes.starts_with(DELTA) {
            let delta = decode_delta(bytes)?;
            let history = self
                .state
                .revisions
                .get_mut(&delta.id)
                .ok_or(MemoryError::Corrupt("delta has no creation"))?;
            let previous = history
                .last()
                .ok_or(MemoryError::Corrupt("empty Memory history"))?;
            if self.state.tombstones.contains_key(&delta.id)
                || previous.revision != delta.previous_revision
                || previous.revision.checked_add(1) != Some(delta.record.revision)
                || previous.body_id != delta.record.body_id
                || delta.record.memory_version != self.state.next_memory_version
                || delta.record.mutation_id == previous.mutation_id
                || self.state.mutations.contains_key(&delta.record.mutation_id)
            {
                return Err(MemoryError::Corrupt("invalid Memory delta predecessor"));
            }
            let mut record = delta.record;
            record.category = if delta.mask & (1 << 0) != 0 {
                record.category
            } else {
                previous.category.clone()
            };
            record.memory_type = if delta.mask & (1 << 1) != 0 {
                record.memory_type
            } else {
                previous.memory_type.clone()
            };
            record.authority_kind = if delta.mask & (1 << 2) != 0 {
                record.authority_kind
            } else {
                previous.authority_kind.clone()
            };
            record.temporal_status = if delta.mask & (1 << 3) != 0 {
                record.temporal_status
            } else {
                previous.temporal_status.clone()
            };
            record.scope = if delta.mask & (1 << 4) != 0 {
                record.scope
            } else {
                previous.scope.clone()
            };
            record.lifecycle_state = if delta.mask & (1 << 5) != 0 {
                record.lifecycle_state
            } else {
                previous.lifecycle_state.clone()
            };
            record.archived = if delta.mask & (1 << 6) != 0 {
                record.archived
            } else {
                previous.archived
            };
            record.superseded_by = if delta.mask & (1 << 7) != 0 {
                record.superseded_by
            } else {
                previous.superseded_by
            };
            record.parent_id = if delta.mask & (1 << 8) != 0 {
                record.parent_id
            } else {
                previous.parent_id
            };
            record.source_node_id = if delta.mask & (1 << 9) != 0 {
                record.source_node_id
            } else {
                previous.source_node_id.clone()
            };
            record.content_source_conversation_id = if delta.mask & (1 << 10) != 0 {
                record.content_source_conversation_id
            } else {
                previous.content_source_conversation_id.clone()
            };
            record.content_source_node_id = if delta.mask & (1 << 11) != 0 {
                record.content_source_node_id
            } else {
                previous.content_source_node_id.clone()
            };
            record.grounding_source_conversation_id = if delta.mask & (1 << 12) != 0 {
                record.grounding_source_conversation_id
            } else {
                previous.grounding_source_conversation_id.clone()
            };
            record.grounding_source_node_id = if delta.mask & (1 << 13) != 0 {
                record.grounding_source_node_id
            } else {
                previous.grounding_source_node_id.clone()
            };
            record.source_episode_id = if delta.mask & (1 << 14) != 0 {
                record.source_episode_id
            } else {
                previous.source_episode_id
            };
            record.source_time_ns = if delta.mask & (1 << 15) != 0 {
                record.source_time_ns
            } else {
                previous.source_time_ns
            };
            record.source_ref = if delta.mask & (1 << 16) != 0 {
                record.source_ref
            } else {
                previous.source_ref.clone()
            };
            record.temporal_inference = if delta.mask & (1 << 17) != 0 {
                record.temporal_inference
            } else {
                previous.temporal_inference.clone()
            };
            record.created_at_ns = if delta.mask & (1 << 18) != 0 {
                record.created_at_ns
            } else {
                previous.created_at_ns
            };
            record.updated_at_ns = if delta.mask & (1 << 19) != 0 {
                record.updated_at_ns
            } else {
                previous.updated_at_ns
            };
            let body_len = self
                .state
                .bodies
                .get(&record.body_id)
                .map(StoredBody::body_len);
            validate_replayed_record_with_body_len(&record, body_len)?;
            let index = history.len();
            self.state
                .mutations
                .insert(record.mutation_id.clone(), (delta.id, index));
            history.push(record);
            self.state.changes.push(delta.id);
            self.state.next_memory_version = self
                .state
                .next_memory_version
                .checked_add(1)
                .ok_or(MemoryError::VersionExhausted)?;
            return Ok(());
        }
        if bytes.starts_with(TOMBSTONE) {
            let (id, revision, memory_version) = decode_tombstone(bytes)?;
            let current = self
                .state
                .revisions
                .get(&id)
                .and_then(|history| history.last())
                .ok_or(MemoryError::Corrupt("tombstone has no Memory"))?;
            if self.state.tombstones.contains_key(&id)
                || current.revision.checked_add(1) != Some(revision)
                || memory_version != self.state.next_memory_version
            {
                return Err(MemoryError::Corrupt("invalid terminal tombstone"));
            }
            self.state.tombstones.insert(
                id,
                TerminalTombstone {
                    revision,
                    memory_version,
                },
            );
            self.state.changes.push(id);
            self.state.next_memory_version = self
                .state
                .next_memory_version
                .checked_add(1)
                .ok_or(MemoryError::VersionExhausted)?;
            return Ok(());
        }
        Err(MemoryError::Corrupt("unknown Memory record"))
    }
}

fn unindexed_bodies(bodies: Vec<(MemoryBodyId, Vec<u8>)>) -> Vec<RetainedGroupedBody> {
    bodies
        .into_iter()
        .map(|(id, bytes)| RetainedGroupedBody {
            id,
            bytes,
            payload_offset: None,
        })
        .collect()
}

fn revision_with_body(record: &MemoryRecord, body: Option<MemoryBody>) -> MemoryRevision {
    MemoryRevision {
        record: record.clone(),
        payload: body.map_or(PayloadState::Absent, PayloadState::Available),
    }
}

fn decode_grouped_body(id: MemoryBodyId, bytes: &[u8]) -> Result<MemoryBody, MemoryError> {
    let mut at = 0usize;
    let title_len = read_grouped_u64(bytes, &mut at)?;
    let title_len = usize::try_from(title_len).map_err(|_| MemoryError::FieldTooLarge)?;
    let title_end = at
        .checked_add(title_len)
        .ok_or(MemoryError::FieldTooLarge)?;
    let title = String::from_utf8(
        bytes
            .get(at..title_end)
            .ok_or(MemoryError::Corrupt("truncated grouped body title"))?
            .to_vec(),
    )
    .map_err(|_| MemoryError::Corrupt("invalid grouped body title UTF-8"))?;
    at = title_end;
    let content_len = usize::try_from(read_grouped_u64(bytes, &mut at)?)
        .map_err(|_| MemoryError::FieldTooLarge)?;
    let content_end = at
        .checked_add(content_len)
        .ok_or(MemoryError::FieldTooLarge)?;
    let content = String::from_utf8(
        bytes
            .get(at..content_end)
            .ok_or(MemoryError::Corrupt("truncated grouped body content"))?
            .to_vec(),
    )
    .map_err(|_| MemoryError::Corrupt("invalid grouped body content UTF-8"))?;
    if content_end != bytes.len() || memory_body_id(&title, &content) != id {
        return Err(MemoryError::Corrupt("grouped body digest mismatch"));
    }
    Ok(MemoryBody { title, content })
}

fn read_grouped_u64(bytes: &[u8], at: &mut usize) -> Result<u64, MemoryError> {
    let end = at.checked_add(8).ok_or(MemoryError::FieldTooLarge)?;
    let raw = bytes
        .get(*at..end)
        .ok_or(MemoryError::Corrupt("truncated grouped body length"))?;
    *at = end;
    Ok(u64::from_le_bytes(raw.try_into().unwrap()))
}

fn record_for_draft(
    draft: &MemoryDraft,
    current: Option<&MemoryRecord>,
    target: MemoryId,
    body_id: MemoryBodyId,
    revision: u64,
    memory_version: u64,
) -> Result<MemoryRecord, MemoryError> {
    let source_ref = draft
        .source_ref
        .clone()
        .or_else(|| current.and_then(|record| record.source_ref.clone()));
    let temporal_inference = draft.temporal_inference.clone().or_else(|| {
        current.and_then(|record| {
            record.temporal_inference.clone().filter(|proof| {
                proof.body_id == body_id && proof.source_time_ns == draft.source_time_ns
            })
        })
    });
    if let Some(proof) = &temporal_inference {
        validate_temporal_inference(
            proof,
            body_id,
            draft.source_time_ns,
            draft.title.len() + draft.content.len(),
        )?;
    }
    Ok(MemoryRecord {
        id: target,
        revision,
        body_id,
        category: draft.category.clone(),
        memory_type: draft.memory_type.clone(),
        authority_kind: draft.authority_kind.clone(),
        temporal_status: draft.temporal_status.clone(),
        scope: draft.scope.clone(),
        lifecycle_state: draft.lifecycle_state.clone(),
        archived: draft.archived,
        superseded_by: draft.superseded_by,
        parent_id: draft.parent_id,
        source_node_id: draft.source_node_id.clone(),
        content_source_conversation_id: draft.content_source_conversation_id.clone(),
        content_source_node_id: draft.content_source_node_id.clone(),
        grounding_source_conversation_id: draft.grounding_source_conversation_id.clone(),
        grounding_source_node_id: draft.grounding_source_node_id.clone(),
        source_episode_id: draft.source_episode_id,
        source_ref,
        source_time_ns: draft.source_time_ns,
        temporal_inference,
        mutation_id: draft.mutation_id.clone(),
        created_at_ns: draft.created_at_ns,
        updated_at_ns: draft.updated_at_ns,
        memory_version,
    })
}
