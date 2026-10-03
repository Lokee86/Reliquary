use crate::Cva;

impl Cva {
    /// Activity at an Archive version, including non-turn publications; zero returns zero.
    /// Versions above the current Archive head are rejected.
    pub fn activity_cut_at_archive_version(
        &self,
        version: u64,
    ) -> Result<u64, crate::ArchiveError> {
        self.archive.activity_cut_at_archive_version(version)
    }

    /// Count of first accepted logical user/assistant identities in this REL.
    pub fn rel_turn_count(&self) -> u64 {
        self.archive.rel_turn_count()
    }

    /// The first REL-local activity position, or None for missing/noneligible turns.
    pub fn activity_position_for_turn(&self, conversation_id: &str, node_id: &str) -> Option<u64> {
        self.archive
            .activity_position_for_turn(conversation_id, node_id)
    }
}
