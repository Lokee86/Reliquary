/// Version of the immutable Freshness defaults and validation contract.
pub const FRESHNESS_POLICY_VERSION: u16 = 1;
pub const FRESHNESS_SCORE_MIN: i16 = -100;
pub const FRESHNESS_SCORE_MAX: i16 = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreshnessPolicy {
    pub decay_turns_per_point: u64,
    pub access_principal: i16,
    pub linkage_principal: i16,
    pub local_hop_cost: i16,
    pub outside_hop_cost: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FreshnessPolicyError {
    ZeroDecayInterval,
    InvalidPrincipal,
    InvalidHopCost,
    InconsistentHopRatios,
}

impl std::fmt::Display for FreshnessPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroDecayInterval => f.write_str("decay interval must be positive"),
            Self::InvalidPrincipal => f.write_str("event principals must be in 1..=100"),
            Self::InvalidHopCost => f.write_str("hop costs must be positive and outside cost must be double local cost"),
            Self::InconsistentHopRatios => f.write_str(
                "local hop cost must equal one fifth of access principal and one tenth of linkage principal",
            ),
        }
    }
}

impl std::error::Error for FreshnessPolicyError {}

impl Default for FreshnessPolicy {
    fn default() -> Self {
        Self {
            decay_turns_per_point: 10,
            access_principal: 25,
            linkage_principal: 50,
            local_hop_cost: 5,
            outside_hop_cost: 10,
        }
    }
}

impl FreshnessPolicy {
    pub const fn default_v1() -> Self {
        Self {
            decay_turns_per_point: 10,
            access_principal: 25,
            linkage_principal: 50,
            local_hop_cost: 5,
            outside_hop_cost: 10,
        }
    }

    /// Validates tunable numerical defaults while keeping score bounds fixed by contract.
    pub fn validate(&self) -> Result<(), FreshnessPolicyError> {
        if self.decay_turns_per_point == 0 {
            return Err(FreshnessPolicyError::ZeroDecayInterval);
        }
        if !(1..=FRESHNESS_SCORE_MAX).contains(&self.access_principal)
            || !(1..=FRESHNESS_SCORE_MAX).contains(&self.linkage_principal)
        {
            return Err(FreshnessPolicyError::InvalidPrincipal);
        }
        if self.local_hop_cost <= 0
            || self.outside_hop_cost <= 0
            || i32::from(self.outside_hop_cost) != i32::from(self.local_hop_cost) * 2
        {
            return Err(FreshnessPolicyError::InvalidHopCost);
        }
        if self.access_principal % 5 != 0
            || self.linkage_principal % 10 != 0
            || self.local_hop_cost != self.access_principal / 5
            || self.local_hop_cost != self.linkage_principal / 10
        {
            return Err(FreshnessPolicyError::InconsistentHopRatios);
        }
        Ok(())
    }

    pub const fn is_valid_event_principal(&self, principal: i16) -> bool {
        principal == self.access_principal || principal == self.linkage_principal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_v1_and_matches_contract() {
        let policy = FreshnessPolicy::default();
        assert_eq!(policy, FreshnessPolicy::default_v1());
        assert_eq!(FRESHNESS_POLICY_VERSION, 1);
        assert_eq!(FRESHNESS_SCORE_MIN, -100);
        assert_eq!(FRESHNESS_SCORE_MAX, 100);
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn coherent_nondefault_policy_is_valid_and_incoherent_costs_are_rejected() {
        let policy = FreshnessPolicy {
            decay_turns_per_point: 5,
            access_principal: 30,
            linkage_principal: 60,
            local_hop_cost: 6,
            outside_hop_cost: 12,
        };
        assert!(policy.validate().is_ok());
        assert!(policy.is_valid_event_principal(30));
        assert!(policy.is_valid_event_principal(60));

        let invalid = FreshnessPolicy {
            local_hop_cost: 5,
            ..policy
        };
        assert_eq!(
            invalid.validate(),
            Err(FreshnessPolicyError::InvalidHopCost)
        );
    }
}
