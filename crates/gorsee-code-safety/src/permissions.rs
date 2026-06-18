use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskClass {
    Read,
    Write,
    Command,
    Network,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Ask,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustProfile {
    Safe,
    Balanced,
    Fast,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionPolicy {
    pub profile: TrustProfile,
}

impl PermissionPolicy {
    pub fn balanced() -> Self {
        Self {
            profile: TrustProfile::Balanced,
        }
    }

    pub fn decide(&self, risk: RiskClass) -> Decision {
        match (self.profile, risk) {
            (_, RiskClass::Delete) => Decision::Deny,
            (TrustProfile::Manual, _) => Decision::Ask,
            (TrustProfile::Safe, RiskClass::Read) => Decision::Allow,
            (TrustProfile::Safe, RiskClass::Command) => Decision::Ask,
            (TrustProfile::Safe, _) => Decision::Ask,
            (TrustProfile::Balanced, RiskClass::Read) => Decision::Allow,
            (TrustProfile::Balanced, RiskClass::Command) => Decision::Ask,
            (TrustProfile::Balanced, RiskClass::Network) => Decision::Ask,
            (TrustProfile::Balanced, RiskClass::Write) => Decision::Ask,
            (TrustProfile::Fast, RiskClass::Read | RiskClass::Write) => Decision::Allow,
            (TrustProfile::Fast, RiskClass::Command | RiskClass::Network) => Decision::Ask,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_allows_reads_and_denies_deletes() {
        let policy = PermissionPolicy::balanced();
        assert_eq!(policy.decide(RiskClass::Read), Decision::Allow);
        assert_eq!(policy.decide(RiskClass::Delete), Decision::Deny);
    }
}
