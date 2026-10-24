use super::OwnerError;
use super::semantic_source::{SemanticSourceReadParts, fragment_position};
use crate::{CompatibilityProfile, CompatibilityProfileId, FragmentId};
pub(crate) struct SemanticArchiveRead<'a> {
    parts: SemanticSourceReadParts<'a>,
}
impl<'a> SemanticArchiveRead<'a> {
    pub(crate) fn new(parts: SemanticSourceReadParts<'a>) -> Self {
        Self { parts }
    }
    pub(crate) fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.parts.container.owner_uuid()
    }
    pub(crate) fn settlement_cut(&self) -> u64 {
        self.parts
            .freshness
            .latest_import_settlement()
            .map_or(0, |s| s.settlement_cut)
    }
    pub(crate) fn activation(&self) -> Option<&crate::freshness_storage::SemanticActivation> {
        self.parts.freshness.semantic_activation()
    }
    pub(crate) fn activity_position(&self, c: &str, n: &str) -> Option<u64> {
        self.parts.archive.activity_position_for_turn(c, n)
    }
    pub(crate) fn fragments(&self) -> Vec<crate::Fragment> {
        self.parts.archive.fragments()
    }
    pub(crate) fn fragment_position(&self, id: FragmentId) -> Result<Option<u64>, OwnerError> {
        fragment_position(self.parts.archive, id)
    }
    pub(crate) fn completed(&self, event: &str) -> Result<bool, String> {
        self.parts.freshness.contains_event(event)
    }
    pub(crate) fn archive_version(&self) -> u64 {
        self.parts.archive.archive_version()
    }
    pub(crate) fn fragment_version(&self, id: FragmentId) -> Option<u64> {
        self.parts.archive.fragments.archive_version(id)
    }
    pub(crate) fn profile(&self, id: CompatibilityProfileId) -> Option<CompatibilityProfile> {
        self.parts.compatibility_profiles.get(id).cloned()
    }
    pub(crate) fn archive_vectors(
        &self,
        id: crate::ArchiveVectorId,
    ) -> Result<crate::ArchiveVectorSet, crate::ArchiveVectorError> {
        self.parts.archive_vectors.get(self.parts.container, id)
    }
    pub(crate) fn text(&self, id: FragmentId) -> Result<String, crate::ArchiveError> {
        self.parts.archive.fragment_text(self.parts.container, id)
    }
}
