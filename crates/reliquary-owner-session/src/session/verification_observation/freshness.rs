use super::{CurrentOwnerSession, ReadOnlyCurrentOwnerObserver};
use reliquary_freshness_store::{HistoricalFreshnessSnapshot, RetainedFreshnessAuthority};

impl ReadOnlyCurrentOwnerObserver {
    /// Observe typed retained Freshness from the native replayed store.
    pub fn retained_freshness_authority(
        &self,
    ) -> Result<(&RetainedFreshnessAuthority, &HistoricalFreshnessSnapshot), String> {
        let CurrentOwnerSession::Rel(session) = &self.session else {
            return Err("freshness_observation_requires_rel".into());
        };
        session
            .retained_freshness_import()
            .ok_or_else(|| "freshness_retained_authority_absent".into())
    }
}
