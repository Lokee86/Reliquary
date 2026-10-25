//! Supported offline community naming bindings.
use crate::{
    CompatibilityProfileId, Cva, DreamCommunityNamer, DreamCommunityNamingError,
    DreamCommunityNamingResult, GeneralEndpoint, Phylactery,
};
impl<E: GeneralEndpoint> DreamCommunityNamer<E> {
    pub fn name_communities(
        &self,
        cva: &mut Cva,
        compatibility_profile_id: CompatibilityProfileId,
        limit: usize,
    ) -> Result<DreamCommunityNamingResult, DreamCommunityNamingError> {
        self.runtime_name_communities(cva, compatibility_profile_id, limit)
    }
    pub fn name_phylactery_communities(
        &self,
        phylactery: &mut Phylactery,
        compatibility_profile_id: CompatibilityProfileId,
        limit: usize,
    ) -> Result<DreamCommunityNamingResult, DreamCommunityNamingError> {
        self.runtime_name_phylactery_communities(phylactery, compatibility_profile_id, limit)
    }
}
