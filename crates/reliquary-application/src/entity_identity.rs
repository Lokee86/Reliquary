//! Select cross-REL candidates and adjudicate one pair at a time. Only the recording owner publishes.
use crate::ManagementError;
use reliquary_entity_resolution::IdentityJudgment;
use reliquary_entity_resolution::{
    EntityResolverError, deterministic_identity_judgment, evaluate_identity_pair,
};
use reliquary_owner_session::model::identity::{IdentityPair, IdentityReceipt, IdentitySide};
use reliquary_switchboard::GeneralEndpoint;

pub use reliquary_entity_resolution::{
    IDENTITY_COMPARISON_POLICY, IdentityDiscoveryIndex, IdentityPairEvidence,
};

mod retained;
pub use retained::retained_identity_quotes;

pub struct IdentityComparison {
    evidence: IdentityPairEvidence,
    rule: Option<IdentityJudgment>,
    route: String,
}

impl IdentityComparison {
    pub fn prepare(evidence: IdentityPairEvidence, route_id: &str) -> Self {
        let rule = deterministic_identity_judgment(&evidence);
        let route = if rule.is_some() {
            "grounding-v1"
        } else {
            route_id
        }
        .to_owned();
        Self {
            evidence,
            rule,
            route,
        }
    }

    pub fn matches(&self, receipt: &IdentityReceipt) -> bool {
        receipt.fingerprint == self.evidence.fingerprint()
            && receipt.policy == IDENTITY_COMPARISON_POLICY
            && receipt.route == self.route
    }

    pub fn evaluate(
        self,
        pair: IdentityPair,
        predecessor: Option<u64>,
        endpoint: Option<&dyn GeneralEndpoint>,
    ) -> Result<IdentityReceipt, ManagementError> {
        let judgment = match self.rule {
            Some(judgment) => judgment,
            None => {
                let unavailable =
                    ManagementError::Unavailable("Entity identity endpoint unavailable");
                let endpoint = endpoint.ok_or(unavailable)?;
                // A provider's raw error or response can contain private evidence.
                evaluate_identity_pair(&self.evidence, endpoint).map_err(|error| match error {
                    EntityResolverError::Endpoint(_) => {
                        ManagementError::Unavailable("Entity identity endpoint unavailable")
                    }
                    EntityResolverError::InvalidOutput(_) => {
                        ManagementError::Owner("invalid Entity identity judgment".into())
                    }
                })?
            }
        };
        let revision = predecessor
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(ManagementError::Conflict)?;
        Ok(IdentityReceipt {
            pair,
            revision,
            predecessor,
            fingerprint: self.evidence.fingerprint(),
            policy: IDENTITY_COMPARISON_POLICY,
            route: self.route,
            left: self.evidence.side(IdentitySide::Left).references(),
            right: self.evidence.side(IdentitySide::Right).references(),
            outcome: judgment.outcome,
            reason: judgment.reason,
            citations: judgment.citations,
        })
    }
}
