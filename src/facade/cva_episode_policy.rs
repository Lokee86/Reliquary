use crate::{
    ArchiveError, Cva, EpisodeBoundary, EpisodeBuildResult, EpisodeConfig, EpisodeOrigin,
    EpisodePolicy,
};

impl Cva {
    pub fn finalize_inactive_path_episodes(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        policy: EpisodePolicy,
        origin: EpisodeOrigin,
        now_ns: i64,
    ) -> Result<Option<EpisodeBuildResult>, ArchiveError> {
        let policy = policy.validate()?;
        let last_source_ns = self.runtime_required_node_timestamp(conversation_id, leaf_node_id)?;
        if !policy.inactive(last_source_ns, now_ns) {
            return Ok(None);
        }
        self.materialize_path_episodes(
            conversation_id,
            leaf_node_id,
            policy.episode,
            origin,
            Some((EpisodeBoundary::Inactivity, now_ns)),
        )
        .map(Some)
    }

    pub fn finalize_explicit_path(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        config: EpisodeConfig,
        now_ns: i64,
    ) -> Result<EpisodeBuildResult, ArchiveError> {
        self.materialize_path_episodes(
            conversation_id,
            leaf_node_id,
            config,
            EpisodeOrigin::Live,
            Some((EpisodeBoundary::Explicit, now_ns)),
        )
    }

    pub fn finalize_create_memory_path(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        config: EpisodeConfig,
        now_ns: i64,
    ) -> Result<EpisodeBuildResult, ArchiveError> {
        self.materialize_path_episodes(
            conversation_id,
            leaf_node_id,
            config,
            EpisodeOrigin::Live,
            Some((EpisodeBoundary::CreateMemory, now_ns)),
        )
    }
}
