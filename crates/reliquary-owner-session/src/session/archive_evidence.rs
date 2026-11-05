use super::*;

impl RelSession {
    /// Read one exact accepted source turn, with a hard text-size ceiling.
    pub fn archive_evidence_turn(
        &self,
        conversation_id: &str,
        node_id: &str,
    ) -> Result<Option<reliquary_archive::ResolvedTurn>, OwnerSessionError> {
        self.archive
            .bounded_resolved_turn(
                conversation_id,
                node_id,
                reliquary_archive::MAX_EVIDENCE_RESULT_BYTES,
            )
            .map_err(OwnerSessionError::Archive)
    }

    /// Read a complete accepted Episode only when it fits the bounded evidence
    /// contract. Oversized ranges fail instead of returning a misleading prefix.
    pub fn archive_evidence_episode(
        &self,
        id: reliquary_archive::EpisodeId,
    ) -> Result<Option<Vec<reliquary_archive::ResolvedTurn>>, OwnerSessionError> {
        self.archive
            .bounded_episode_turns(
                id,
                reliquary_archive::MAX_EVIDENCE_TURNS,
                reliquary_archive::MAX_EVIDENCE_RESULT_BYTES,
            )
            .map_err(OwnerSessionError::Archive)
    }

    /// Read a complete ancestor-to-descendant range in one conversation under
    /// the same turn and aggregate text ceilings used by Insomnia evidence.
    pub fn archive_evidence_range(
        &self,
        conversation_id: &str,
        start_node_id: &str,
        end_node_id: &str,
    ) -> Result<Vec<reliquary_archive::ResolvedTurn>, OwnerSessionError> {
        self.archive
            .bounded_conversation_range(
                conversation_id,
                start_node_id,
                end_node_id,
                reliquary_archive::MAX_EVIDENCE_TURNS,
                reliquary_archive::MAX_EVIDENCE_RESULT_BYTES,
            )
            .map_err(OwnerSessionError::Archive)
    }

    /// Search only this mounted REL's accepted Archive. Search scans a bounded
    /// deterministic prefix and reports when that prefix may omit later hits.
    pub fn archive_evidence_search(
        &self,
        conversation_id: Option<&str>,
        query: &str,
        limit: usize,
        through_timestamp_ns: Option<i64>,
    ) -> Result<reliquary_archive::ArchiveSearchResult, OwnerSessionError> {
        self.archive
            .bounded_search(conversation_id, query, limit, through_timestamp_ns)
            .map_err(OwnerSessionError::Archive)
    }
}
