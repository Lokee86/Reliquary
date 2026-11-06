//! Owner-local typed Graph and Community persistence.
//!
//! Endpoint IDs are supplied by the owning stores. This crate never discovers
//! foreign records; callers provide an exact admission closure for writes and replay.

use reliquary_container::{Container, ContainerError, ContainerIdentity};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

type Id = [u8; 32];
const GRAPH_MAGIC: &[u8; 8] = b"RELGRF01";
const COMMUNITY_MAGIC: &[u8; 8] = b"RELCOM01";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NodeRef {
    Memory(Id),
    Entity(Id),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RelationKind {
    Topical,
    Factual,
    Causal,
    Recurrent,
    References,
    DuplicateOf,
    Supersedes,
    StructuralParent,
    EntityAssociation,
    PrincipalAssociation,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RelationOrigin {
    Dream,
    User,
    Perception,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelationChange {
    pub source: NodeRef,
    pub target: NodeRef,
    pub kind: RelationKind,
    pub active: bool,
    pub origin: RelationOrigin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Relation {
    pub source: NodeRef,
    pub target: NodeRef,
    pub kind: RelationKind,
    pub active: bool,
    pub origin: RelationOrigin,
    pub graph_version: u64,
}

#[derive(Clone, Debug, Default)]
pub struct GraphAdmission {
    owner_id: Option<[u8; 16]>,
    memories: BTreeSet<Id>,
    historical_memories: BTreeSet<Id>,
    entities: BTreeSet<Id>,
    historical_entities: BTreeSet<Id>,
    principal_entities: BTreeSet<Id>,
}

impl GraphAdmission {
    pub fn for_changes(
        owner_id: [u8; 16],
        memories: impl IntoIterator<Item = Id>,
        entities: impl IntoIterator<Item = (Id, bool)>,
    ) -> Self {
        let mut value = Self {
            owner_id: Some(owner_id),
            memories: memories.into_iter().collect(),
            ..Self::default()
        };
        for (id, principal) in entities {
            value.entities.insert(id);
            if principal {
                value.principal_entities.insert(id);
            }
        }
        value
    }
    pub fn for_replay(
        owner_id: [u8; 16],
        memories: impl IntoIterator<Item = Id>,
        historical_memories: impl IntoIterator<Item = Id>,
        entities: impl IntoIterator<Item = (Id, bool)>,
        historical_entities: impl IntoIterator<Item = Id>,
        historical_principals: impl IntoIterator<Item = Id>,
    ) -> Self {
        let mut value = Self::for_changes(owner_id, memories, entities);
        value.historical_memories.extend(historical_memories);
        value.historical_entities.extend(historical_entities);
        value.principal_entities.extend(historical_principals);
        value
    }
    pub fn owner_id(&self) -> Option<[u8; 16]> {
        self.owner_id
    }
    fn contains(&self, node: NodeRef, replay: bool) -> bool {
        match node {
            NodeRef::Memory(id) => {
                self.memories.contains(&id) || (replay && self.historical_memories.contains(&id))
            }
            NodeRef::Entity(id) => {
                self.entities.contains(&id) || (replay && self.historical_entities.contains(&id))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CommunityId(pub Id);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Community {
    pub id: CommunityId,
    pub members: Vec<Id>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommunitySnapshot {
    pub generation: u64,
    pub graph_version: u64,
    pub communities: Vec<Community>,
}

#[derive(Debug)]
pub enum GraphError {
    Container(ContainerError),
    WrongOwner,
    CorruptRecord(&'static str),
    InvalidClosure(&'static str),
    InvalidRelation(&'static str),
    InvalidCommunity(&'static str),
    VersionExhausted,
}
impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(e) => e.fmt(f),
            Self::WrongOwner => f.write_str("Graph record belongs to another owner"),
            Self::CorruptRecord(s) => write!(f, "corrupt Graph record: {s}"),
            Self::InvalidClosure(s) => write!(f, "Graph endpoint closure rejected: {s}"),
            Self::InvalidRelation(s) => write!(f, "invalid Graph relation: {s}"),
            Self::InvalidCommunity(s) => write!(f, "invalid Community snapshot: {s}"),
            Self::VersionExhausted => f.write_str("Graph version exhausted"),
        }
    }
}
impl std::error::Error for GraphError {}
impl From<ContainerError> for GraphError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Key {
    source: NodeRef,
    target: NodeRef,
    kind: RelationKind,
}

#[derive(Clone)]
pub struct GraphStore {
    owner: [u8; 16],
    identity: ContainerIdentity,
    graph_version: u64,
    states: BTreeMap<Key, Relation>,
    history: Vec<Relation>,
    outgoing: BTreeMap<NodeRef, BTreeSet<Key>>,
    incoming: BTreeMap<NodeRef, BTreeSet<Key>>,
    duplicate_incident: BTreeMap<NodeRef, BTreeSet<Key>>,
}

impl GraphStore {
    pub fn open(container: &Container, admission: &GraphAdmission) -> Result<Self, GraphError> {
        Self::open_with_grouped_dream(container, admission, |_| Ok(None))
    }

    /// Owner-session supplies the decoder for its atomic Dream carrier. Bare
    /// Graph readers fail closed rather than silently dropping grouped effects.
    pub fn open_with_grouped_dream(
        container: &Container,
        admission: &GraphAdmission,
        grouped: impl FnMut(&[u8]) -> Result<Option<Relation>, GraphError>,
    ) -> Result<Self, GraphError> {
        Self::open_with_dream_batches(container, admission, grouped, |_, _| {
            Err(GraphError::CorruptRecord(
                "Dream batch requires owner replay",
            ))
        })
    }
    pub fn open_with_dream_batches(
        container: &Container,
        admission: &GraphAdmission,
        mut grouped: impl FnMut(&[u8]) -> Result<Option<Relation>, GraphError>,
        mut batch: impl FnMut(&[u8], &Self) -> Result<Vec<Relation>, GraphError>,
    ) -> Result<Self, GraphError> {
        if admission.owner_id != Some(container.owner_id()) {
            return Err(GraphError::InvalidClosure(
                "admission belongs to another owner",
            ));
        }
        let mut store = Self {
            owner: container.owner_id(),
            identity: container.identity(),
            graph_version: 0,
            states: BTreeMap::new(),
            history: Vec::new(),
            outgoing: BTreeMap::new(),
            incoming: BTreeMap::new(),
            duplicate_incident: BTreeMap::new(),
        };
        for frame in container.read_frames()? {
            if frame.payload.starts_with(b"RELDRB") {
                for relation in batch(&frame.payload, &store)? {
                    store.apply_dream_relation(admission, relation)?;
                }
            } else if let Some((owner, relation)) = decode_graph(&frame.payload)? {
                if owner != store.owner {
                    return Err(GraphError::WrongOwner);
                }
                store.apply_replay(relation, admission)?;
            } else if frame.payload.starts_with(b"RELDRG") || frame.payload.starts_with(b"RELDRS") {
                let relation = grouped(&frame.payload)?.ok_or(GraphError::CorruptRecord(
                    "grouped Dream requires owner replay",
                ))?;
                store.validate_grouped_relation(admission, relation)?;
                store.apply_replay(relation, admission)?;
            }
        }
        Ok(store)
    }
    /// Validate a caller-owned, already-encoded relation before its grouped
    /// physical commit; the owner-session holds the writer lease.
    pub fn validate_grouped_relation(
        &self,
        admission: &GraphAdmission,
        relation: Relation,
    ) -> Result<(), GraphError> {
        if admission.owner_id != Some(self.owner) {
            return Err(GraphError::WrongOwner);
        }
        validate_change(
            admission,
            RelationChange {
                source: relation.source,
                target: relation.target,
                kind: relation.kind,
                active: relation.active,
                origin: relation.origin,
            },
            false,
        )?;
        if !relation.active || relation.origin != RelationOrigin::Dream {
            return Err(GraphError::InvalidRelation(
                "grouped Dream effect must be active Dream",
            ));
        }
        if relation.graph_version
            != self
                .graph_version
                .checked_add(1)
                .ok_or(GraphError::VersionExhausted)?
        {
            return Err(GraphError::CorruptRecord(
                "stale grouped Dream Graph version",
            ));
        }
        if self
            .relation(relation.source, relation.target, relation.kind)
            .is_some_and(|existing| existing.origin == RelationOrigin::User)
        {
            return Err(GraphError::InvalidRelation(
                "Dream cannot replace user-origin relation",
            ));
        }
        Ok(())
    }
    /// Apply only after owner-session has synced the complete grouped carrier.
    pub fn apply_grouped_relation(
        &mut self,
        admission: &GraphAdmission,
        relation: Relation,
    ) -> Result<(), GraphError> {
        self.validate_grouped_relation(admission, relation)?;
        self.apply_relation(relation)
    }
    pub fn graph_version(&self) -> u64 {
        self.graph_version
    }
    pub fn owner_id(&self) -> [u8; 16] {
        self.owner
    }
    /// Accepted relation history for owner-session closure validation.
    pub fn history(&self) -> impl Iterator<Item = &Relation> {
        self.history.iter()
    }
    pub fn active_relations(&self) -> impl Iterator<Item = &Relation> {
        self.states.values().filter(|relation| relation.active)
    }
    pub fn relation(
        &self,
        source: NodeRef,
        target: NodeRef,
        kind: RelationKind,
    ) -> Option<&Relation> {
        self.states.get(&Key {
            source,
            target,
            kind,
        })
    }
    pub fn outgoing(&self, node: NodeRef) -> Vec<Relation> {
        self.outgoing
            .get(&node)
            .into_iter()
            .flatten()
            .filter_map(|key| self.states.get(key).copied())
            .filter(|relation| relation.kind != RelationKind::PrincipalAssociation)
            .collect()
    }
    pub fn incoming(&self, node: NodeRef) -> Vec<Relation> {
        self.incoming
            .get(&node)
            .into_iter()
            .flatten()
            .filter_map(|key| self.states.get(key).copied())
            .filter(|relation| relation.kind != RelationKind::PrincipalAssociation)
            .collect()
    }
    pub fn active_memory_nodes(&self) -> BTreeSet<Id> {
        self.states
            .values()
            .filter(|r| r.active && r.kind != RelationKind::PrincipalAssociation)
            .flat_map(|r| [r.source, r.target])
            .filter_map(|n| {
                if let NodeRef::Memory(id) = n {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }
    fn active_memory_nodes_at(&self, version: u64) -> Result<BTreeSet<Id>, GraphError> {
        if version > self.graph_version {
            return Err(GraphError::InvalidCommunity("snapshot is ahead of Graph"));
        }
        let mut state = BTreeMap::<Key, Relation>::new();
        for relation in self.history.iter().filter(|r| r.graph_version <= version) {
            state.insert(
                Key {
                    source: relation.source,
                    target: relation.target,
                    kind: relation.kind,
                },
                *relation,
            );
        }
        Ok(state
            .values()
            .filter(|r| r.active && r.kind != RelationKind::PrincipalAssociation)
            .flat_map(|r| [r.source, r.target])
            .filter_map(|n| {
                if let NodeRef::Memory(id) = n {
                    Some(id)
                } else {
                    None
                }
            })
            .collect())
    }
    pub fn put(
        &mut self,
        container: &mut Container,
        admission: &GraphAdmission,
        changes: &[RelationChange],
    ) -> Result<(), GraphError> {
        self.check_owner(container)?;
        if admission.owner_id != Some(self.owner) {
            return Err(GraphError::InvalidClosure(
                "admission belongs to another owner",
            ));
        }
        if changes.is_empty() {
            return Err(GraphError::InvalidRelation("empty transaction"));
        }
        let mut version = self.graph_version;
        for change in changes {
            validate_change(admission, *change, false)?;
            version = version.checked_add(1).ok_or(GraphError::VersionExhausted)?;
        }
        for change in changes {
            let graph_version = self
                .graph_version
                .checked_add(1)
                .ok_or(GraphError::VersionExhausted)?;
            let relation = Relation {
                source: change.source,
                target: change.target,
                kind: change.kind,
                active: change.active,
                origin: change.origin,
                graph_version,
            };
            let bytes = encode_graph(self.owner, relation);
            container.append(&bytes)?;
            container.sync()?;
            self.apply_relation(relation)?;
        }
        Ok(())
    }
    fn apply_replay(
        &mut self,
        relation: Relation,
        admission: &GraphAdmission,
    ) -> Result<(), GraphError> {
        validate_relation(
            relation.source,
            relation.target,
            relation.kind,
            relation.origin,
        )?;
        validate_endpoints(admission, relation.source, relation.target, true)?;
        let expected = self
            .graph_version
            .checked_add(1)
            .ok_or(GraphError::VersionExhausted)?;
        if relation.graph_version != expected {
            return Err(GraphError::CorruptRecord("noncontiguous graph version"));
        }
        if relation.kind == RelationKind::PrincipalAssociation {
            let NodeRef::Entity(id) = relation.target else {
                unreachable!()
            };
            if !admission.principal_entities.contains(&id) {
                return Err(GraphError::InvalidClosure(
                    "principal identity does not match endpoint",
                ));
            }
        }
        self.apply_relation(relation)
    }
    fn apply_relation(&mut self, relation: Relation) -> Result<(), GraphError> {
        let key = Key {
            source: relation.source,
            target: relation.target,
            kind: relation.kind,
        };
        if let Some(previous) = self.states.get(&key).copied() {
            self.remove_index(key, previous);
        }
        self.states.insert(key, relation);
        if relation.active {
            if relation.kind == RelationKind::DuplicateOf {
                self.duplicate_incident
                    .entry(relation.source)
                    .or_default()
                    .insert(key);
                self.duplicate_incident
                    .entry(relation.target)
                    .or_default()
                    .insert(key);
            }
            self.outgoing
                .entry(relation.source)
                .or_default()
                .insert(key);
            self.incoming
                .entry(relation.target)
                .or_default()
                .insert(key);
        }
        self.history.push(relation);
        self.graph_version = relation.graph_version;
        Ok(())
    }
    fn remove_index(&mut self, key: Key, relation: Relation) {
        if relation.active {
            if relation.kind == RelationKind::DuplicateOf {
                remove_key(&mut self.duplicate_incident, relation.source, key);
                remove_key(&mut self.duplicate_incident, relation.target, key);
            }
            remove_key(&mut self.outgoing, relation.source, key);
            remove_key(&mut self.incoming, relation.target, key);
        }
    }
    fn check_owner(&self, container: &Container) -> Result<(), GraphError> {
        if container.owner_id() == self.owner && container.identity() == self.identity {
            Ok(())
        } else {
            Err(GraphError::WrongOwner)
        }
    }
}

fn remove_key(map: &mut BTreeMap<NodeRef, BTreeSet<Key>>, node: NodeRef, key: Key) {
    if let Some(keys) = map.get_mut(&node) {
        keys.remove(&key);
        if keys.is_empty() {
            map.remove(&node);
        }
    }
}
fn validate_change(
    admission: &GraphAdmission,
    c: RelationChange,
    replay: bool,
) -> Result<(), GraphError> {
    validate_relation(c.source, c.target, c.kind, c.origin)?;
    validate_endpoints(admission, c.source, c.target, replay)?;
    if c.kind == RelationKind::PrincipalAssociation {
        let NodeRef::Entity(id) = c.target else {
            unreachable!()
        };
        if !admission.principal_entities.contains(&id) {
            return Err(GraphError::InvalidClosure(
                "principal identity does not match endpoint",
            ));
        }
    }
    Ok(())
}
fn validate_endpoints(
    a: &GraphAdmission,
    s: NodeRef,
    t: NodeRef,
    replay: bool,
) -> Result<(), GraphError> {
    for node in [s, t] {
        if !a.contains(node, replay) {
            return Err(GraphError::InvalidClosure(
                "endpoint is outside admitted owner snapshot",
            ));
        }
    }
    Ok(())
}
fn validate_relation(
    s: NodeRef,
    t: NodeRef,
    kind: RelationKind,
    origin: RelationOrigin,
) -> Result<(), GraphError> {
    if s == t {
        return Err(GraphError::InvalidRelation("self relation"));
    }
    match kind {
        RelationKind::EntityAssociation | RelationKind::PrincipalAssociation => {
            if !matches!((s, t), (NodeRef::Memory(_), NodeRef::Entity(_)))
                || origin == RelationOrigin::Dream
            {
                return Err(GraphError::InvalidRelation("association shape or origin"));
            }
        }
        _ => {
            if !matches!((s, t), (NodeRef::Memory(_), NodeRef::Memory(_)))
                || origin == RelationOrigin::Perception
            {
                return Err(GraphError::InvalidRelation(
                    "memory relation shape or origin",
                ));
            }
        }
    }
    Ok(())
}

pub struct CommunityStore {
    owner: [u8; 16],
    identity: ContainerIdentity,
    current: Option<CommunitySnapshot>,
}
impl CommunityStore {
    pub fn open(container: &Container, graph: &GraphStore) -> Result<Self, GraphError> {
        graph.check_owner(container)?;
        let mut store = Self {
            owner: container.owner_id(),
            identity: container.identity(),
            current: None,
        };
        for frame in container.read_frames()? {
            if let Some((owner, snapshot)) = decode_community(&frame.payload)? {
                if owner != store.owner {
                    return Err(GraphError::WrongOwner);
                }
                store.validate(&snapshot, graph)?;
                let expected = store.current.as_ref().map_or(1, |s: &CommunitySnapshot| {
                    s.generation.checked_add(1).unwrap_or(0)
                });
                if snapshot.generation != expected || expected == 0 {
                    return Err(GraphError::CorruptRecord("Community generation sequence"));
                }
                if store
                    .current
                    .as_ref()
                    .is_some_and(|current| snapshot.graph_version < current.graph_version)
                {
                    return Err(GraphError::CorruptRecord(
                        "Community graph version regressed",
                    ));
                }
                store.current = Some(snapshot);
            }
        }
        Ok(store)
    }
    pub fn current(&self) -> Option<&CommunitySnapshot> {
        self.current.as_ref()
    }
    pub fn publish(
        &mut self,
        container: &mut Container,
        graph: &GraphStore,
        snapshot: CommunitySnapshot,
    ) -> Result<(), GraphError> {
        if container.owner_id() != self.owner || container.identity() != self.identity {
            return Err(GraphError::WrongOwner);
        };
        graph.check_owner(container)?;
        let expected = self
            .current
            .as_ref()
            .map_or(1, |s| s.generation.checked_add(1).unwrap_or(0));
        if snapshot.generation != expected || expected == 0 {
            return Err(GraphError::InvalidCommunity("generation is not next"));
        };
        if self
            .current
            .as_ref()
            .is_some_and(|current| snapshot.graph_version < current.graph_version)
        {
            return Err(GraphError::InvalidCommunity("graph version regressed"));
        }
        self.validate(&snapshot, graph)?;
        let bytes = encode_community(self.owner, &snapshot)?;
        container.append(&bytes)?;
        container.sync()?;
        self.current = Some(snapshot);
        Ok(())
    }
    fn validate(&self, snapshot: &CommunitySnapshot, graph: &GraphStore) -> Result<(), GraphError> {
        let closure = graph.active_memory_nodes_at(snapshot.graph_version)?;
        let mut seen = BTreeSet::new();
        let mut ids = BTreeSet::new();
        let mut previous = None;
        for community in &snapshot.communities {
            if community.members.is_empty() || !ids.insert(community.id) {
                return Err(GraphError::InvalidCommunity("empty or duplicate community"));
            };
            if previous.is_some_and(|id| id >= community.id) {
                return Err(GraphError::InvalidCommunity("community order"));
            }
            previous = Some(community.id);
            if community.members.windows(2).any(|w| w[0] >= w[1]) {
                return Err(GraphError::InvalidCommunity(
                    "members must be sorted and unique",
                ));
            };
            for id in &community.members {
                if !seen.insert(*id) {
                    return Err(GraphError::InvalidCommunity(
                        "memory appears in multiple communities",
                    ));
                }
            }
            if community.id != community_id(self.owner, &community.members) {
                return Err(GraphError::InvalidCommunity("community identity"));
            }
        }
        if seen != closure {
            return Err(GraphError::InvalidCommunity(
                "snapshot does not cover active graph memory closure",
            ));
        };
        Ok(())
    }
}

pub fn community_id(owner: [u8; 16], members: &[[u8; 32]]) -> CommunityId {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-community-v1\0");
    hash.update(owner);
    hash.update((members.len() as u64).to_le_bytes());
    for id in members {
        hash.update(id);
    }
    CommunityId(hash.finalize().into())
}

fn encode_graph(owner: [u8; 16], r: Relation) -> Vec<u8> {
    let mut o = GRAPH_MAGIC.to_vec();
    o.extend_from_slice(&owner);
    o.extend_from_slice(&r.graph_version.to_le_bytes());
    put_node(&mut o, r.source);
    put_node(&mut o, r.target);
    o.extend_from_slice(&kind_code(r.kind).to_le_bytes());
    o.push(u8::from(r.active));
    o.push(origin_code(r.origin));
    o
}
fn decode_graph(b: &[u8]) -> Result<Option<([u8; 16], Relation)>, GraphError> {
    if b.get(..8) != Some(GRAPH_MAGIC.as_slice()) {
        return Ok(None);
    };
    if b.len() != 8 + 16 + 8 + 66 + 4 {
        return Err(GraphError::CorruptRecord("graph event length"));
    };
    let mut p = 8;
    let owner = take::<16>(b, &mut p)?;
    let graph_version = u64::from_le_bytes(take::<8>(b, &mut p)?);
    let source = get_node(b, &mut p)?;
    let target = get_node(b, &mut p)?;
    let kind = decode_kind(u16::from_le_bytes(take::<2>(b, &mut p)?))?;
    let active = match take::<1>(b, &mut p)?[0] {
        0 => false,
        1 => true,
        _ => return Err(GraphError::CorruptRecord("active flag")),
    };
    let origin = decode_origin(take::<1>(b, &mut p)?[0])?;
    Ok(Some((
        owner,
        Relation {
            source,
            target,
            kind,
            active,
            origin,
            graph_version,
        },
    )))
}

/// Read a Graph relation from a physical frame for owner-prefix validation.
pub fn relation_in_frame(payload: &[u8]) -> Result<Option<Relation>, GraphError> {
    Ok(decode_graph(payload)?.map(|(_, relation)| relation))
}
fn encode_community(owner: [u8; 16], s: &CommunitySnapshot) -> Result<Vec<u8>, GraphError> {
    let mut o = COMMUNITY_MAGIC.to_vec();
    o.extend_from_slice(&owner);
    o.extend_from_slice(&s.generation.to_le_bytes());
    o.extend_from_slice(&s.graph_version.to_le_bytes());
    o.extend_from_slice(
        &u32::try_from(s.communities.len())
            .map_err(|_| GraphError::InvalidCommunity("too many communities"))?
            .to_le_bytes(),
    );
    for c in &s.communities {
        o.extend_from_slice(&c.id.0);
        o.extend_from_slice(
            &u32::try_from(c.members.len())
                .map_err(|_| GraphError::InvalidCommunity("too many members"))?
                .to_le_bytes(),
        );
        for id in &c.members {
            o.extend_from_slice(id)
        }
    }
    Ok(o)
}
fn decode_community(b: &[u8]) -> Result<Option<([u8; 16], CommunitySnapshot)>, GraphError> {
    if b.get(..8) != Some(COMMUNITY_MAGIC.as_slice()) {
        return Ok(None);
    };
    let mut p = 8;
    let owner = take::<16>(b, &mut p)?;
    let generation = u64::from_le_bytes(take::<8>(b, &mut p)?);
    let graph_version = u64::from_le_bytes(take::<8>(b, &mut p)?);
    let count = u32::from_le_bytes(take::<4>(b, &mut p)?) as usize;
    let mut communities = Vec::with_capacity(count);
    for _ in 0..count {
        let id = CommunityId(take::<32>(b, &mut p)?);
        let n = u32::from_le_bytes(take::<4>(b, &mut p)?) as usize;
        let mut members = Vec::with_capacity(n);
        for _ in 0..n {
            members.push(take::<32>(b, &mut p)?)
        }
        communities.push(Community { id, members })
    }
    if p != b.len() {
        return Err(GraphError::CorruptRecord("Community trailing bytes"));
    }
    Ok(Some((
        owner,
        CommunitySnapshot {
            generation,
            graph_version,
            communities,
        },
    )))
}

/// Read a Community's source Graph cut for physical-prefix validation.
pub fn community_graph_version_in_frame(payload: &[u8]) -> Result<Option<u64>, GraphError> {
    Ok(decode_community(payload)?.map(|(_, snapshot)| snapshot.graph_version))
}
fn put_node(o: &mut Vec<u8>, n: NodeRef) {
    match n {
        NodeRef::Memory(id) => {
            o.push(1);
            o.extend_from_slice(&id)
        }
        NodeRef::Entity(id) => {
            o.push(2);
            o.extend_from_slice(&id)
        }
    }
}
fn get_node(b: &[u8], p: &mut usize) -> Result<NodeRef, GraphError> {
    let kind = take::<1>(b, p)?[0];
    let id = take::<32>(b, p)?;
    match kind {
        1 => Ok(NodeRef::Memory(id)),
        2 => Ok(NodeRef::Entity(id)),
        _ => Err(GraphError::CorruptRecord("node kind")),
    }
}
fn kind_code(k: RelationKind) -> u16 {
    match k {
        RelationKind::Topical => 1,
        RelationKind::Factual => 2,
        RelationKind::Causal => 3,
        RelationKind::Recurrent => 4,
        RelationKind::References => 5,
        RelationKind::DuplicateOf => 6,
        RelationKind::Supersedes => 7,
        RelationKind::StructuralParent => 8,
        RelationKind::EntityAssociation => 100,
        RelationKind::PrincipalAssociation => 101,
    }
}
fn decode_kind(c: u16) -> Result<RelationKind, GraphError> {
    match c {
        1 => Ok(RelationKind::Topical),
        2 => Ok(RelationKind::Factual),
        3 => Ok(RelationKind::Causal),
        4 => Ok(RelationKind::Recurrent),
        5 => Ok(RelationKind::References),
        6 => Ok(RelationKind::DuplicateOf),
        7 => Ok(RelationKind::Supersedes),
        8 => Ok(RelationKind::StructuralParent),
        100 => Ok(RelationKind::EntityAssociation),
        101 => Ok(RelationKind::PrincipalAssociation),
        _ => Err(GraphError::CorruptRecord("relation kind")),
    }
}
fn origin_code(o: RelationOrigin) -> u8 {
    match o {
        RelationOrigin::Dream => 1,
        RelationOrigin::User => 2,
        RelationOrigin::Perception => 3,
    }
}
fn decode_origin(c: u8) -> Result<RelationOrigin, GraphError> {
    match c {
        1 => Ok(RelationOrigin::Dream),
        2 => Ok(RelationOrigin::User),
        3 => Ok(RelationOrigin::Perception),
        _ => Err(GraphError::CorruptRecord("relation origin")),
    }
}
fn take<const N: usize>(b: &[u8], p: &mut usize) -> Result<[u8; N], GraphError> {
    let end = p
        .checked_add(N)
        .ok_or(GraphError::CorruptRecord("length overflow"))?;
    let s = b
        .get(*p..end)
        .ok_or(GraphError::CorruptRecord("truncated record"))?;
    *p = end;
    Ok(s.try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::{ContainerIdentity, ReliquaryScope};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".graph-test-tmp");
            fs::create_dir_all(&dir).unwrap();
            Self(dir.join(format!(
                "graph-{}-{}.rel",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
            if let Some(p) = self.0.parent() {
                let _ = fs::remove_dir(p);
            }
        }
    }
    fn container(path: &PathBuf, owner: u8) -> Container {
        let mut id = [0; 16];
        id[0] = owner;
        Container::create(
            path,
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            id,
        )
        .unwrap()
    }

    #[test]
    fn community_rejects_graph_from_another_owner_before_writing() {
        let first_path = Temp::new();
        let second_path = Temp::new();
        let mut first = container(&first_path.0, 10);
        let second = container(&second_path.0, 11);
        let first_admission = GraphAdmission::for_changes(first.owner_id(), [], []);
        let second_admission = GraphAdmission::for_changes(second.owner_id(), [], []);
        let first_graph = GraphStore::open(&first, &first_admission).unwrap();
        let second_graph = GraphStore::open(&second, &second_admission).unwrap();
        let mut communities = CommunityStore::open(&first, &first_graph).unwrap();
        let before = first.read_frames().unwrap().len();
        assert!(matches!(
            communities.publish(
                &mut first,
                &second_graph,
                CommunitySnapshot {
                    generation: 1,
                    graph_version: 0,
                    communities: vec![],
                }
            ),
            Err(GraphError::WrongOwner)
        ));
        assert_eq!(first.read_frames().unwrap().len(), before);
        assert!(
            CommunityStore::open(&first, &first_graph)
                .unwrap()
                .current()
                .is_none()
        );
        let phy_path = Temp::new();
        let phy = Container::create(
            &phy_path.0,
            ContainerIdentity::phylactery(),
            first.owner_id(),
        )
        .unwrap();
        assert!(matches!(
            CommunityStore::open(&phy, &first_graph),
            Err(GraphError::WrongOwner)
        ));
    }

    #[test]
    fn graph_replays_exact_typed_endpoints_and_rebuilds_indexes() {
        let path = Temp::new();
        let m1 = [1; 32];
        let m2 = [2; 32];
        let e = [1; 32];
        let mut c = container(&path.0, 1);
        let admission = GraphAdmission::for_changes(c.owner_id(), [m1, m2], [(e, false)]);
        {
            let mut g = GraphStore::open(&c, &admission).unwrap();
            g.put(
                &mut c,
                &admission,
                &[
                    RelationChange {
                        source: NodeRef::Memory(m1),
                        target: NodeRef::Memory(m2),
                        kind: RelationKind::Topical,
                        active: true,
                        origin: RelationOrigin::User,
                    },
                    RelationChange {
                        source: NodeRef::Memory(m1),
                        target: NodeRef::Entity(e),
                        kind: RelationKind::EntityAssociation,
                        active: true,
                        origin: RelationOrigin::Perception,
                    },
                ],
            )
            .unwrap();
            assert_eq!(g.outgoing(NodeRef::Memory(m1)).len(), 2);
        }
        drop(c);
        let c = Container::open_read_only(&path.0).unwrap();
        let g = GraphStore::open(
            &c,
            &GraphAdmission::for_replay(c.owner_id(), [m1, m2], [], [(e, false)], [], []),
        )
        .unwrap();
        assert_eq!(g.graph_version(), 2);
        assert_eq!(g.outgoing(NodeRef::Memory(m1)).len(), 2);
        assert!(g.outgoing(NodeRef::Entity(e)).is_empty());
    }

    #[test]
    fn owner_and_endpoint_closure_are_enforced() {
        let path = Temp::new();
        let mut c = container(&path.0, 2);
        let good = GraphAdmission::for_changes(c.owner_id(), [[1; 32], [2; 32]], []);
        let mut g = GraphStore::open(&c, &good).unwrap();
        assert!(matches!(
            g.put(
                &mut c,
                &good,
                &[RelationChange {
                    source: NodeRef::Memory([1; 32]),
                    target: NodeRef::Memory([9; 32]),
                    kind: RelationKind::Topical,
                    active: true,
                    origin: RelationOrigin::User
                }]
            ),
            Err(GraphError::InvalidClosure(_))
        ));
        let other_path = Temp::new();
        let mut other = container(&other_path.0, 3);
        assert!(matches!(
            g.put(
                &mut other,
                &good,
                &[RelationChange {
                    source: NodeRef::Memory([1; 32]),
                    target: NodeRef::Memory([2; 32]),
                    kind: RelationKind::Topical,
                    active: true,
                    origin: RelationOrigin::User
                }]
            ),
            Err(GraphError::WrongOwner)
        ));
    }

    #[test]
    fn community_snapshot_must_exactly_cover_active_memory_closure() {
        let path = Temp::new();
        let m1 = [1; 32];
        let m2 = [2; 32];
        let mut c = container(&path.0, 4);
        let owner = c.owner_id();
        let admission = GraphAdmission::for_changes(owner, [m1, m2], []);
        let mut graph = GraphStore::open(&c, &admission).unwrap();
        graph
            .put(
                &mut c,
                &admission,
                &[RelationChange {
                    source: NodeRef::Memory(m1),
                    target: NodeRef::Memory(m2),
                    kind: RelationKind::References,
                    active: true,
                    origin: RelationOrigin::User,
                }],
            )
            .unwrap();
        let mut communities = CommunityStore::open(&c, &graph).unwrap();
        let incomplete = CommunitySnapshot {
            generation: 1,
            graph_version: 1,
            communities: vec![Community {
                id: community_id(owner, &[m1]),
                members: vec![m1],
            }],
        };
        assert!(matches!(
            communities.publish(&mut c, &graph, incomplete),
            Err(GraphError::InvalidCommunity(_))
        ));
        let complete = CommunitySnapshot {
            generation: 1,
            graph_version: 1,
            communities: vec![Community {
                id: community_id(owner, &[m1, m2]),
                members: vec![m1, m2],
            }],
        };
        communities.publish(&mut c, &graph, complete).unwrap();
        drop(c);
        let c = Container::open_read_only(&path.0).unwrap();
        let graph = GraphStore::open(&c, &admission).unwrap();
        let communities = CommunityStore::open(&c, &graph).unwrap();
        assert_eq!(
            communities.current().unwrap().communities[0].members,
            vec![m1, m2]
        );
    }

    #[test]
    fn historical_community_snapshot_must_cover_its_graph_version_closure() {
        let path = Temp::new();
        let m1 = [1; 32];
        let m2 = [2; 32];
        let m3 = [3; 32];
        let mut c = container(&path.0, 5);
        let owner = c.owner_id();
        let admission = GraphAdmission::for_changes(owner, [m1, m2, m3], []);
        let mut graph = GraphStore::open(&c, &admission).unwrap();
        graph
            .put(
                &mut c,
                &admission,
                &[RelationChange {
                    source: NodeRef::Memory(m1),
                    target: NodeRef::Memory(m2),
                    kind: RelationKind::References,
                    active: true,
                    origin: RelationOrigin::User,
                }],
            )
            .unwrap();
        let mut communities = CommunityStore::open(&c, &graph).unwrap();
        graph
            .put(
                &mut c,
                &admission,
                &[RelationChange {
                    source: NodeRef::Memory(m2),
                    target: NodeRef::Memory(m3),
                    kind: RelationKind::References,
                    active: true,
                    origin: RelationOrigin::User,
                }],
            )
            .unwrap();

        let before = c.read_frames().unwrap().len();
        let incomplete = CommunitySnapshot {
            generation: 1,
            graph_version: 1,
            communities: vec![Community {
                id: community_id(owner, &[m1]),
                members: vec![m1],
            }],
        };
        assert!(matches!(
            communities.publish(&mut c, &graph, incomplete),
            Err(GraphError::InvalidCommunity(_))
        ));
        assert_eq!(c.read_frames().unwrap().len(), before);
    }

    #[test]
    fn community_graph_version_cannot_regress_in_generation_history() {
        let path = Temp::new();
        let m1 = [1; 32];
        let m2 = [2; 32];
        let m3 = [3; 32];
        let mut c = container(&path.0, 6);
        let owner = c.owner_id();
        let admission = GraphAdmission::for_changes(owner, [m1, m2, m3], []);
        let mut graph = GraphStore::open(&c, &admission).unwrap();
        graph
            .put(
                &mut c,
                &admission,
                &[RelationChange {
                    source: NodeRef::Memory(m1),
                    target: NodeRef::Memory(m2),
                    kind: RelationKind::References,
                    active: true,
                    origin: RelationOrigin::User,
                }],
            )
            .unwrap();
        graph
            .put(
                &mut c,
                &admission,
                &[RelationChange {
                    source: NodeRef::Memory(m2),
                    target: NodeRef::Memory(m3),
                    kind: RelationKind::References,
                    active: true,
                    origin: RelationOrigin::User,
                }],
            )
            .unwrap();
        let mut communities = CommunityStore::open(&c, &graph).unwrap();
        let current = CommunitySnapshot {
            generation: 1,
            graph_version: 2,
            communities: vec![Community {
                id: community_id(owner, &[m1, m2, m3]),
                members: vec![m1, m2, m3],
            }],
        };
        communities.publish(&mut c, &graph, current).unwrap();
        let stale = CommunitySnapshot {
            generation: 2,
            graph_version: 1,
            communities: vec![Community {
                id: community_id(owner, &[m1, m2]),
                members: vec![m1, m2],
            }],
        };
        let before = c.read_frames().unwrap().len();
        assert!(matches!(
            communities.publish(&mut c, &graph, stale.clone()),
            Err(GraphError::InvalidCommunity("graph version regressed"))
        ));
        assert_eq!(c.read_frames().unwrap().len(), before);

        c.append(&encode_community(owner, &stale).unwrap()).unwrap();
        c.sync().unwrap();
        drop(communities);
        drop(c);
        let c = Container::open_read_only(&path.0).unwrap();
        assert!(matches!(
            CommunityStore::open(&c, &graph),
            Err(GraphError::CorruptRecord(
                "Community graph version regressed"
            ))
        ));
    }
}

#[path = "dream_batch.rs"]
mod dream_batch;

pub use dream_batch::dream_relation_in_frame;

#[path = "duplicate_context.rs"]
mod duplicate_context;
