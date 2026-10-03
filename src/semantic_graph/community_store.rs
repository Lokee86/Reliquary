use crate::CommunityLineageTransition;
use crate::community_codec::{
    decode_semantic_name, decode_snapshot, encode_semantic_name, encode_snapshot,
};
use crate::community_lineage::{latest_lineage, resolve_semantic_name};
use crate::graph_store::GraphStore;
use crate::{
    COMMUNITY_ALGORITHM_VERSION, COMMUNITY_NAMING_CONTRACT_VERSION, CommunityError, CommunityId,
    CommunitySemanticName, CommunitySemanticNameSource, CommunitySnapshot, CommunityStats,
    Container, DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES, MAX_COMMUNITY_SEMANTIC_NAME_BYTES,
    MemoryId,
};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct CommunityStore {
    snapshots: Vec<CommunitySnapshot>,
    semantic_names: HashMap<CommunityId, CommunitySemanticName>,
}

impl CommunityStore {
    pub(crate) fn latest(&self) -> Option<&CommunitySnapshot> {
        self.snapshots.last()
    }

    pub(crate) fn contains_snapshot(&self, generation: u64, derived_graph_version: u64) -> bool {
        self.snapshots.iter().any(|snapshot| {
            snapshot.generation == generation
                && snapshot.derived_graph_version == derived_graph_version
        })
    }

    pub(crate) fn semantic_name(&self, community_id: CommunityId) -> Option<CommunitySemanticName> {
        resolve_semantic_name(&self.snapshots, &self.semantic_names, community_id)
    }

    pub(crate) fn current_semantic_names(&self) -> Vec<CommunitySemanticName> {
        let Some(snapshot) = self.latest() else {
            return Vec::new();
        };
        snapshot
            .communities
            .iter()
            .filter_map(|community| self.semantic_name(community.id))
            .collect()
    }

    pub(crate) fn latest_lineage(&self) -> Option<CommunityLineageTransition> {
        latest_lineage(&self.snapshots)
    }

    pub(crate) fn next_generation(&self) -> Result<u64, CommunityError> {
        self.latest().map_or(Ok(1), |snapshot| {
            snapshot
                .generation
                .checked_add(1)
                .ok_or(CommunityError::GenerationOverflow)
        })
    }

    pub(crate) fn publish(
        &mut self,
        container: &mut Container,
        graph: &GraphStore,
        owner_uuid: [u8; 16],
        snapshot: CommunitySnapshot,
    ) -> Result<(), CommunityError> {
        self.validate_next(graph, owner_uuid, &snapshot)?;
        container.append(&encode_snapshot(&snapshot)?)?;
        self.snapshots.push(snapshot);
        Ok(())
    }

    pub(crate) fn publish_semantic_name(
        &mut self,
        container: &mut Container,
        graph: &GraphStore,
        record: CommunitySemanticName,
    ) -> Result<bool, CommunityError> {
        let snapshot = self
            .latest()
            .filter(|snapshot| {
                snapshot.derived_graph_version == graph.memory_graph_version()
                    && snapshot.algorithm_version == COMMUNITY_ALGORITHM_VERSION
            })
            .ok_or(CommunityError::InvalidSemanticName(
                "semantic names require a current community snapshot",
            ))?;
        validate_semantic_name(snapshot, &record)?;
        if self.semantic_names.get(&record.community_id) == Some(&record) {
            return Ok(false);
        }
        container.append(&encode_semantic_name(&record)?)?;
        self.semantic_names.insert(record.community_id, record);
        Ok(true)
    }

    pub(crate) fn stats(&self, graph_version: u64) -> CommunityStats {
        let latest = self.latest();
        CommunityStats {
            generations: latest.map_or(0, |snapshot| snapshot.generation),
            communities: latest.map_or(0, |snapshot| snapshot.communities.len()),
            memberships: latest.map_or(0, |snapshot| {
                snapshot
                    .communities
                    .iter()
                    .map(|community| community.members.len())
                    .sum()
            }),
            derived_graph_version: latest.map(|snapshot| snapshot.derived_graph_version),
            current: latest.is_some_and(|snapshot| {
                snapshot.derived_graph_version == graph_version
                    && snapshot.algorithm_version == COMMUNITY_ALGORITHM_VERSION
            }),
        }
    }

    fn insert_rebuilt(
        &mut self,
        graph: &GraphStore,
        owner_uuid: [u8; 16],
        snapshot: CommunitySnapshot,
    ) -> Result<(), CommunityError> {
        self.validate_next(graph, owner_uuid, &snapshot)?;
        self.snapshots.push(snapshot);
        Ok(())
    }

    fn validate_next(
        &self,
        graph: &GraphStore,
        owner_uuid: [u8; 16],
        snapshot: &CommunitySnapshot,
    ) -> Result<(), CommunityError> {
        if snapshot.generation != self.next_generation()? {
            return Err(CommunityError::InvalidSnapshot("non-contiguous generation"));
        }
        validate_snapshot(graph, owner_uuid, snapshot)
    }
}

pub(crate) struct CommunityOpenState {
    snapshots: Vec<CommunitySnapshot>,
    semantic_names: Vec<CommunitySemanticName>,
}

impl CommunityOpenState {
    pub(crate) fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            semantic_names: Vec::new(),
        }
    }

    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), CommunityError> {
        if let Some(snapshot) = decode_snapshot(payload)? {
            self.snapshots.push(snapshot);
        }
        if let Some(record) = decode_semantic_name(payload)? {
            self.semantic_names.push(record);
        }
        Ok(())
    }

    pub(crate) fn finish(
        self,
        graph: &GraphStore,
        owner_uuid: Option<[u8; 16]>,
    ) -> Result<CommunityStore, CommunityError> {
        if self.snapshots.is_empty() && self.semantic_names.is_empty() {
            return Ok(CommunityStore::default());
        }
        let owner_uuid = owner_uuid.ok_or(CommunityError::MissingOwnerIdentity)?;
        let mut store = CommunityStore::default();
        for snapshot in self.snapshots {
            store.insert_rebuilt(graph, owner_uuid, snapshot)?;
        }
        for record in self.semantic_names {
            let snapshot = store
                .snapshots
                .iter()
                .rev()
                .find(|snapshot| {
                    snapshot
                        .communities
                        .iter()
                        .any(|community| community.id == record.community_id)
                })
                .ok_or(CommunityError::InvalidSemanticName(
                    "semantic name references an unknown community",
                ))?;
            validate_semantic_name(snapshot, &record)?;
            store.semantic_names.insert(record.community_id, record);
        }
        Ok(store)
    }
}

pub(crate) fn community_id(owner_uuid: [u8; 16], members: &[MemoryId]) -> CommunityId {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-community-v1\0");
    hash.update(owner_uuid);
    hash.update((members.len() as u64).to_le_bytes());
    for member in members {
        hash.update(member.0);
    }
    CommunityId(hash.finalize().into())
}

fn validate_snapshot(
    graph: &GraphStore,
    owner_uuid: [u8; 16],
    snapshot: &CommunitySnapshot,
) -> Result<(), CommunityError> {
    if snapshot.algorithm_version == 0 || snapshot.algorithm_version > COMMUNITY_ALGORITHM_VERSION {
        return Err(CommunityError::InvalidSnapshot(
            "unsupported algorithm version",
        ));
    }
    if !snapshot.resolution.is_finite() || snapshot.resolution <= 0.0 {
        return Err(CommunityError::InvalidSnapshot("invalid resolution"));
    }
    if !snapshot.quality.is_finite() {
        return Err(CommunityError::InvalidSnapshot("invalid quality"));
    }
    if snapshot.derived_graph_version > graph.memory_graph_version() {
        return Err(CommunityError::InvalidSnapshot("future graph version"));
    }

    let mut previous_community = None;
    let mut seen_members = HashSet::new();
    for community in &snapshot.communities {
        if community.members.is_empty() {
            return Err(CommunityError::InvalidSnapshot("empty community"));
        }
        if previous_community.is_some_and(|previous| previous >= community.id) {
            return Err(CommunityError::InvalidSnapshot("community order"));
        }
        previous_community = Some(community.id);

        let mut previous_member: Option<[u8; 32]> = None;
        for member in &community.members {
            if previous_member.is_some_and(|previous| previous >= member.0) {
                return Err(CommunityError::InvalidSnapshot("member order"));
            }
            previous_member = Some(member.0);
            graph.node_id(*member)?;
            if !seen_members.insert(*member) {
                return Err(CommunityError::InvalidSnapshot("duplicate member"));
            }
        }
        if community.id != community_id(owner_uuid, &community.members) {
            return Err(CommunityError::InvalidSnapshot("community identity"));
        }
    }

    if snapshot.derived_graph_version == graph.memory_graph_version()
        && seen_members.len() != graph.memory_node_count()
    {
        return Err(CommunityError::InvalidSnapshot(
            "current snapshot does not cover graph nodes",
        ));
    }
    Ok(())
}

fn validate_semantic_name(
    snapshot: &CommunitySnapshot,
    record: &CommunitySemanticName,
) -> Result<(), CommunityError> {
    if record.baseline_community_id != record.community_id {
        return Err(CommunityError::InvalidSemanticName(
            "persisted name baseline must match its community",
        ));
    }
    match record.source {
        CommunitySemanticNameSource::Dream => {
            if record.contract_version == 0
                || record.contract_version > COMMUNITY_NAMING_CONTRACT_VERSION
            {
                return Err(CommunityError::InvalidSemanticName(
                    "unsupported naming contract version",
                ));
            }
        }
        CommunitySemanticNameSource::User => {
            if record.contract_version != 0 || !record.representative_memories.is_empty() {
                return Err(CommunityError::InvalidSemanticName(
                    "user names cannot carry Dream contract evidence",
                ));
            }
        }
    }
    if record.name.is_empty()
        || record.name.len() > MAX_COMMUNITY_SEMANTIC_NAME_BYTES
        || record.name.trim() != record.name
        || record.name.chars().any(char::is_control)
    {
        return Err(CommunityError::InvalidSemanticName("invalid name text"));
    }
    if record.source == CommunitySemanticNameSource::Dream
        && (record.representative_memories.is_empty()
            || record.representative_memories.len() > DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES)
    {
        return Err(CommunityError::InvalidSemanticName(
            "invalid representative count",
        ));
    }
    let community = snapshot
        .communities
        .iter()
        .find(|community| community.id == record.community_id)
        .ok_or(CommunityError::InvalidSemanticName(
            "semantic name community is absent from snapshot",
        ))?;
    let mut previous: Option<[u8; 32]> = None;
    for memory_id in &record.representative_memories {
        if previous.is_some_and(|previous| previous >= memory_id.0) {
            return Err(CommunityError::InvalidSemanticName(
                "representatives must be sorted and unique",
            ));
        }
        if !community.members.contains(memory_id) {
            return Err(CommunityError::InvalidSemanticName(
                "representative is not a community member",
            ));
        }
        previous = Some(memory_id.0);
    }
    Ok(())
}
