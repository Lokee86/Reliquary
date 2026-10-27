//! Physical owner replay coordination.
//!
//! Routing is only a dispatch hint: each receiving owner must validate its records.

use reliquary_container::{Container, ContainerError, PhysicalFrame};

/// A native header error or a mismatch with the requested owner kind.
#[derive(Debug)]
pub enum OwnerHeaderError {
    Container(ContainerError),
    WrongKind,
}

/// Inspect the durable owner identity before acquiring an owner replay session.
pub fn inspect_owner_header(
    path: impl AsRef<std::path::Path>,
    kind: reliquary_container::FileKind,
) -> Result<[u8; 16], OwnerHeaderError> {
    let header = Container::open_read_only_for_scan(path).map_err(OwnerHeaderError::Container)?;
    let owner = header
        .owner_uuid()
        .ok_or(OwnerHeaderError::Container(ContainerError::InvalidIdentity))?;
    if header.identity().map(|identity| identity.file_kind) != Some(kind) {
        return Err(OwnerHeaderError::WrongKind);
    }
    Ok(owner)
}

/// Traverse the complete accepted physical stream in order and publish its scan
/// state only after every frame has passed the caller's owner validation.
pub fn replay_open<E>(
    container: &mut Container,
    mut visit: impl FnMut(PhysicalFrame) -> Result<(), E>,
) -> Result<(), E>
where
    E: From<ContainerError>,
{
    let state = {
        let mut cursor = container.physical_cursor()?;
        while let Some(frame) = cursor.next_frame()? {
            visit(frame)?;
        }
        cursor.finish_open_scan()?
    };
    container.accept_replay_scan(state)?;
    Ok(())
}

/// Replay an accepted physical prefix without publishing scan state or writing.
/// The Container cursor rejects cuts inside a frame.
pub fn replay_prefix<E>(
    container: &Container,
    end: u64,
    mut visit: impl FnMut(PhysicalFrame) -> Result<(), E>,
) -> Result<(), E>
where
    E: From<ContainerError>,
{
    let mut cursor = container.physical_cursor_through(end)?;
    while let Some(frame) = cursor.next_frame()? {
        visit(frame)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenRecordRoute {
    Archive,
    Memory,
    Graph,
    Publication,
    ContainerVersion,
    Fallback,
}

pub fn classify_open_record(payload: &[u8]) -> OpenRecordRoute {
    let Some(magic) = payload.get(..8) else {
        return OpenRecordRoute::Fallback;
    };

    match magic {
        b"CVAIMPT1" | b"CVACONT1" | b"CVANODE1" | b"CVABRCH1" | b"CVAFRAG1" | b"CVAFILE1"
        | b"CVAAFMT2" | b"CVAAREC1" | b"CVACONV1" | b"CVACONV2" | b"CVAEPIS1" | b"CVAFMEM1"
        | b"CVATURN1" => OpenRecordRoute::Archive,

        b"CVAMEMF2" | b"CVAMBDY1" | b"CVAMEMR2" | b"CVAMEMR3" | b"CVAMEMR4" | b"CVAMEMR5"
        | b"CVAMEMR6" | b"CVAMEMR7" | b"CVAMEMV1" | b"CVAMRTE1" | b"CVAMRTE2" => {
            OpenRecordRoute::Memory
        }

        b"CVAGFMT1" | b"CVAGNODE" | b"CVAGNOD2" | b"CVAGVER1" | b"CVAGMUT1" | b"CVAGMUT2"
        | b"CVAGBAT1" | b"CVAGBAT2" | b"CVAGBL01" | b"CVAGCM01" | b"CVAGCB01" => {
            OpenRecordRoute::Graph
        }

        b"CVAVERS1" | b"CVAVERS2" => OpenRecordRoute::ContainerVersion,
        b"CVAPUB01" => OpenRecordRoute::Publication,

        _ => OpenRecordRoute::Fallback,
    }
}

/// Remaining REL frame classes that require product-level proof handling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelSpecialFrame {
    Publication,
    Fallback,
}

#[derive(Debug)]
pub enum RelCoreReplayError {
    Archive(reliquary_archive::ArchiveError),
    Memory(reliquary_memory_core::MemoryError),
    Graph(reliquary_graph::GraphError),
}

/// Route ordinary REL frames to their concrete lower owner projections.
/// Publication and grouped-carrier proof handling remains with the caller.
pub fn ingest_rel_core_frame(
    object: reliquary_container::ObjectRef,
    payload: &[u8],
    latest_global: u64,
    archive: &mut reliquary_archive::archive_rebuild::ArchiveOpenState,
    memory: &mut reliquary_memory_core::MemoryOpenState,
    graph: &mut reliquary_graph::GraphOpenState,
) -> Result<Option<RelSpecialFrame>, RelCoreReplayError> {
    match classify_open_record(payload) {
        OpenRecordRoute::Archive => archive
            .ingest(object, payload, latest_global)
            .map_err(RelCoreReplayError::Archive)?,
        OpenRecordRoute::Memory => memory
            .ingest(object, payload, latest_global)
            .map_err(RelCoreReplayError::Memory)?,
        OpenRecordRoute::Graph => graph
            .ingest(object, payload, latest_global)
            .map_err(RelCoreReplayError::Graph)?,
        OpenRecordRoute::ContainerVersion => {}
        OpenRecordRoute::Publication => return Ok(Some(RelSpecialFrame::Publication)),
        OpenRecordRoute::Fallback => return Ok(Some(RelSpecialFrame::Fallback)),
    }
    Ok(None)
}

/// Failures from ordered replay of the REL interaction, compaction and Echo projections.
#[derive(Debug)]
pub enum RelEvidenceReplayError {
    Interaction(reliquary_archive::interaction_stream_store::ArchivePersistenceError),
    Compaction(reliquary_archive::ConversationCompactionError),
    Echo(reliquary_echo::EchoError),
}

/// Replay REL evidence in its established order, leaving product policy to the caller.
pub fn ingest_rel_evidence(
    object: reliquary_container::ObjectRef,
    payload: &[u8],
    interactions: &mut reliquary_archive::interaction_stream_store::InteractionStreamStore,
    compactions: &mut reliquary_archive::conversation_compaction_store::ConversationCompactionOpenState,
    echo: &mut reliquary_echo::EchoStore,
) -> Result<(), RelEvidenceReplayError> {
    interactions
        .ingest(payload)
        .map_err(RelEvidenceReplayError::Interaction)?;
    compactions
        .ingest(object, payload)
        .map_err(RelEvidenceReplayError::Compaction)?;
    echo.ingest(payload).map_err(RelEvidenceReplayError::Echo)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{OpenRecordRoute, classify_open_record};

    #[test]
    fn routes_known_high_volume_record_families() {
        let cases = [
            (b"CVACONT1".as_slice(), OpenRecordRoute::Archive),
            (b"CVAAREC1".as_slice(), OpenRecordRoute::Archive),
            (b"CVATURN1".as_slice(), OpenRecordRoute::Archive),
            (b"CVAMEMF2".as_slice(), OpenRecordRoute::Memory),
            (b"CVAMEMR7".as_slice(), OpenRecordRoute::Memory),
            (b"CVAMRTE2".as_slice(), OpenRecordRoute::Memory),
            (b"CVAGFMT1".as_slice(), OpenRecordRoute::Graph),
            (b"CVAGBAT2".as_slice(), OpenRecordRoute::Graph),
            (b"CVAVERS2".as_slice(), OpenRecordRoute::ContainerVersion),
        ];

        for (payload, expected) in cases {
            assert_eq!(classify_open_record(payload), expected);
        }
    }

    #[test]
    fn insomnia_completion_and_unknown_records_stay_on_fallback() {
        assert_eq!(
            classify_open_record(b"CVAINSC5payload"),
            OpenRecordRoute::Fallback
        );
        assert_eq!(
            classify_open_record(b"CVAPEP01payload"),
            OpenRecordRoute::Fallback
        );
        assert_eq!(
            classify_open_record(b"CVACMP1\0payload"),
            OpenRecordRoute::Fallback
        );
        assert_eq!(
            classify_open_record(b"UNKNOWN!payload"),
            OpenRecordRoute::Fallback
        );
        assert_eq!(classify_open_record(b"tiny"), OpenRecordRoute::Fallback);
    }
}

#[cfg(test)]
mod replay_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    #[allow(dead_code)]
    #[derive(Debug)]
    enum Error {
        Container(ContainerError),
        Rejected,
    }
    impl From<ContainerError> for Error {
        fn from(value: ContainerError) -> Self {
            Self::Container(value)
        }
    }

    #[test]
    fn visits_in_physical_order_and_does_not_admit_failed_replay() {
        let path = std::env::current_dir().unwrap().join(format!(
            "owner-lifecycle-replay-{}-{}.cva",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut writer = Container::create(&path).unwrap();
        writer.append(b"first").unwrap();
        writer.append(b"second").unwrap();
        writer.sync().unwrap();
        drop(writer);
        let mut container = Container::open_for_replay(&path).unwrap();
        let mut seen = Vec::new();
        let result = replay_open(&mut container, |frame| {
            seen.push(frame.payload);
            if seen.len() == 2 {
                return Err(Error::Rejected);
            }
            Ok(())
        });
        assert!(matches!(result, Err(Error::Rejected)));
        assert_eq!(seen, [b"first".to_vec(), b"second".to_vec()]);
        assert!(matches!(
            container.append(b"blocked"),
            Err(ContainerError::WriteAdmissionClosed)
        ));
        replay_open(&mut container, |_| Ok::<_, Error>(())).unwrap();
        container.append(b"admitted").unwrap();
        drop(container);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn bounded_replay_stops_at_frame_boundary_without_changing_file() {
        let path = std::env::current_dir().unwrap().join(format!(
            "owner-lifecycle-prefix-{}-{}.cva",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut writer = Container::create(&path).unwrap();
        writer.append(b"first").unwrap();
        let boundary = writer.physical_length().unwrap();
        writer.append(b"second").unwrap();
        writer.sync().unwrap();
        drop(writer);
        let container = Container::open_read_only_for_scan(&path).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut seen = Vec::new();
        replay_prefix(&container, boundary, |frame| {
            seen.push(frame.payload);
            Ok::<_, ContainerError>(())
        })
        .unwrap();
        assert_eq!(seen, [b"first".to_vec()]);
        assert!(replay_prefix(&container, boundary - 1, |_| Ok::<_, ContainerError>(())).is_err());
        drop(container);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::remove_file(path).unwrap();
    }
}

use reliquary_memory_core::{MemoryId, memory_store::MemoryStore};
use reliquary_perception_store::{EntityStore, RelationshipError, RelationshipStore};

pub fn validate_historical_local_references(
    store: &RelationshipStore,
    owner_id: Option<&str>,
    memories: &MemoryStore,
    entities: &EntityStore,
) -> Result<(), RelationshipError> {
    let current = store.current();
    if current.is_empty() {
        return Ok(());
    }
    let owner_id = owner_id.ok_or(RelationshipError::MissingOwnerIdentity)?;
    for relationship in current {
        for participant in &relationship.participants {
            if participant.entity.owner_id == owner_id
                && !entities.contains_historical(participant.entity.entity_id)
            {
                return Err(RelationshipError::MissingLocalEntity(
                    participant.entity.entity_id,
                ));
            }
        }
        for evidence in &relationship.evidence {
            if evidence.owner_id == owner_id
                && !memories.contains_historical_memory(MemoryId(evidence.memory_id))
            {
                return Err(RelationshipError::MissingLocalMemory(evidence.memory_id));
            }
        }
    }
    Ok(())
}

/// Finish Graph reconstruction using accepted Memory and Entity projections.
pub fn finish_graph_replay(
    state: reliquary_graph::GraphOpenState,
    m: &MemoryStore,
    e: &EntityStore,
) -> Result<reliquary_graph::GraphStore, reliquary_graph::GraphError> {
    use reliquary_graph::SemanticNodeKind;
    let mut current_memories = Vec::new();
    let mut historical_memories = Vec::new();
    let mut current_entities = Vec::new();
    let mut historical_entities = Vec::new();
    let mut historical_principals = Vec::new();
    let nodes = state.semantic_nodes();
    let referenced_entities: std::collections::HashSet<_> = nodes
        .iter()
        .filter(|n| n.kind == SemanticNodeKind::Entity)
        .map(|n| reliquary_perception_store::EntityId(n.id))
        .collect();
    let mut latest_kinds = std::collections::HashMap::new();
    if !referenced_entities.is_empty() {
        for record in e.records() {
            if referenced_entities.contains(&record.id) {
                latest_kinds.insert(
                    record.id,
                    record.kind == reliquary_perception_store::PRINCIPAL_ENTITY_KIND,
                );
            }
        }
    }
    for node in nodes {
        match node.kind {
            SemanticNodeKind::Memory => {
                let id = MemoryId(node.id);
                if m.contains_memory(id) {
                    current_memories.push(node.id);
                }
                if m.contains_historical_memory(id) {
                    historical_memories.push(node.id);
                }
            }
            SemanticNodeKind::Entity => {
                let id = reliquary_perception_store::EntityId(node.id);
                if let Ok(entity) = e.entity(id) {
                    current_entities.push((
                        node.id,
                        entity.kind == reliquary_perception_store::PRINCIPAL_ENTITY_KIND,
                    ));
                }
                if e.contains_historical(id) {
                    historical_entities.push(node.id);
                }
                if latest_kinds.get(&id).copied().unwrap_or(false) {
                    historical_principals.push(node.id);
                }
            }
            SemanticNodeKind::Observation => {}
        }
    }
    let facts = reliquary_graph::GraphAdmission::for_replay(
        current_memories,
        historical_memories,
        current_entities,
        historical_entities,
        historical_principals,
    );
    state.finish(&facts)
}

/// Require every local Archive file link to name an accepted historical Memory.
pub fn validate_archive_memory_links(
    archive: &reliquary_archive::Archive,
    memories: &MemoryStore,
) -> Result<(), reliquary_archive::ArchiveError> {
    for link in archive.all_file_memory_links() {
        if !memories.contains_historical_memory(MemoryId(link.memory_id.0)) {
            return Err(reliquary_archive::ArchiveError::MissingFileMemoryTarget);
        }
    }
    Ok(())
}

/// Native initialization failures, kept distinct for the product owner.
#[derive(Debug)]
pub enum CoreInitializationError {
    Memory(reliquary_memory_core::MemoryError),
    Graph(reliquary_graph::GraphError),
    Entity(reliquary_perception_store::EntityError),
    Relationship(RelationshipError),
}

/// Write the core owner formats in their established physical order.
/// REL initializes Archive before this shared sequence. Other owner formats
/// and the final sync remain with the caller.
pub fn initialize_core(
    container: &mut Container,
    memories: &MemoryStore,
    graph: &mut reliquary_graph::GraphStore,
    entities: &EntityStore,
    relationships: &RelationshipStore,
) -> Result<(), CoreInitializationError> {
    memories
        .initialize(container)
        .map_err(CoreInitializationError::Memory)?;
    graph
        .initialize(container)
        .map_err(CoreInitializationError::Graph)?;
    entities
        .initialize(container)
        .map_err(CoreInitializationError::Entity)?;
    relationships
        .initialize(container)
        .map_err(CoreInitializationError::Relationship)?;
    Ok(())
}

/// Return the first duplicate accepted global coordinate across owner stores.
/// REL supplies its additional Archive and vector coordinates; PHY supplies none.
pub fn conflicting_global_version(
    memories: &MemoryStore,
    entities: &EntityStore,
    relationships: &RelationshipStore,
    graph: &reliquary_graph::GraphStore,
    additional: impl IntoIterator<Item = u64>,
) -> Option<u64> {
    let mut versions: Vec<u64> = memories
        .records()
        .iter()
        .map(|r| r.global_version)
        .chain(entities.records().iter().map(|r| r.global_version))
        .chain(relationships.records().iter().map(|r| r.global_version))
        .chain(memories.tombstone_global_versions())
        .chain(entities.tombstone_global_versions())
        .chain(relationships.tombstone_global_versions())
        .chain(graph.transaction_global_versions().iter().copied())
        .chain(additional)
        .collect();
    versions.sort_unstable();
    versions
        .windows(2)
        .find(|pair| pair[0] == pair[1])
        .map(|pair| pair[0])
}
