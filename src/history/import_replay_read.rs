//! Immutable owner-local replay reads at Import's original scan checkpoints.
use crate::owner_lifecycle::HistoryReadAccess;

pub(crate) struct ImportReplayRead<'a>(HistoryReadAccess<'a>);
impl<'a> ImportReplayRead<'a> {
    pub(crate) fn new(access: HistoryReadAccess<'a>) -> Self {
        Self(access)
    }
    pub(crate) fn archive_range(
        &self,
        archive: &crate::Archive,
        files: &crate::project_file_binding_store::ProjectFileStore,
        first: usize,
        end: usize,
    ) -> Result<crate::archive_ordered_replay::ArchiveTail, crate::ArchiveError> {
        self.0.archive_replay_range(archive, files, first, end)
    }
    pub(crate) fn memory_range(
        &self,
        memories: &crate::memory_store::MemoryStore,
        first: usize,
        end: usize,
    ) -> Result<crate::memory_ordered_replay::MemoryTail, crate::CvaReconcileError> {
        self.0.memory_replay_range(memories, first, end)
    }
    pub(crate) fn entity_range(
        &self,
        entities: &crate::entity_store::EntityStore,
        first: usize,
        end: usize,
    ) -> Result<crate::entity_ordered_replay::EntityTail, crate::EntityError> {
        self.0.entity_replay_range(entities, first, end)
    }
    pub(crate) fn relationship_range(
        &self,
        relationships: &crate::relationship_store::RelationshipStore,
        first: usize,
        end: usize,
    ) -> Result<crate::relationship_ordered_replay::RelationshipTail, crate::RelationshipError>
    {
        self.0.relationship_replay_range(relationships, first, end)
    }
    pub(crate) fn graph_range(
        &self,
        graph: &crate::graph_store::GraphStore,
        first: usize,
        end: usize,
    ) -> Result<crate::graph_ordered_replay::GraphTail, crate::CvaReconcileError> {
        self.0
            .graph_replay_range(graph, first, end)
            .map_err(|e| match e {
                crate::graph_ordered_replay::GraphTailReadError::Graph(e) => e.into(),
                crate::graph_ordered_replay::GraphTailReadError::InvalidGraphVersionRecord => {
                    crate::CvaReconcileError::InvalidGraphVersionRecord
                }
            })
    }
}

pub(crate) fn community_snapshot(
    payload: &[u8],
) -> Result<Option<crate::CommunitySnapshot>, crate::owner_lifecycle::OwnerError> {
    crate::community_codec::decode_snapshot(payload).map_err(Into::into)
}
pub(crate) fn graph_version(
    object: crate::ObjectRef,
    payload: &[u8],
) -> Result<Option<crate::graph_codec::GraphVersionPayload>, crate::GraphError> {
    crate::graph_compact_codec::publication::version_at(object, payload)
}
pub(crate) fn graph_coordinates(payload: &[u8]) -> Result<Option<(u64, u64)>, crate::GraphError> {
    crate::graph_compact_codec::publication::coordinates(payload)
}

/// Interpret accepted owner record families in their established short-circuit
/// decode order. Import retains the decision about which intervals may replay.
pub(crate) fn has_owner_record(
    chunk: crate::owner_lifecycle::HistoryObject,
    payload: &[u8],
) -> Result<bool, crate::CvaReconcileError> {
    Ok(crate::insomnia::codec::decode_record(&payload)?.is_some()
        || crate::memory_codec::decode_body(&payload)?.is_some()
        || crate::memory_compact_codec::publication::decode(&payload, 1)?.is_some()
        || crate::memory_compact_codec::publication::coordinates(&payload)?.is_some()
        || crate::memory_routing_codec::decode(&payload)?.is_some()
        || crate::entity_compact_codec::publication::decode(&payload, 1)?.is_some()
        || crate::entity_compact_codec::publication::coordinates(&payload)?.is_some()
        || crate::relationship_compact_codec::publication::decode(&payload, 1)?.is_some()
        || crate::relationship_compact_codec::publication::coordinates(&payload)?.is_some()
        || crate::entity_resolution_codec::delta::is_current(&payload)
        || ((payload.starts_with(b"CVAGCM") || payload.starts_with(b"CVAGCB"))
            && crate::graph_compact_codec::publication::decode(&payload, 1)?.is_some())
        || crate::graph_codec::decode_node(&payload)?.is_some()
        || crate::graph_compact_codec::publication::version_at(chunk, &payload)?.is_some()
        || crate::community_codec::decode_snapshot(&payload)
            .map_err(crate::CvaError::from)?
            .is_some()
        || !crate::freshness_storage::FreshnessReplayEntry::from_payload(&payload)
            .map_err(crate::CvaError::Freshness)?
            .is_empty()
        || payload.starts_with(b"CVDREAM1")
        || payload.starts_with(b"CVDREAM2")
        || payload.starts_with(b"CVAPEP01"))
}
