use serde::{Deserialize, Serialize};

use crate::TokenLedger;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetPolicy {
    pub session_tokens: u64,
    pub session_usd: Option<f64>,
    pub warn_at_percent: u8,
    pub stop_at_percent: u8,
}

impl Default for BudgetPolicy {
    fn default() -> Self {
        Self {
            session_tokens: 80_000,
            session_usd: Some(2.0),
            warn_at_percent: 75,
            stop_at_percent: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub used_tokens: u64,
    pub limit_tokens: u64,
    pub percent_used: f64,
    pub warning: bool,
    pub stopped: bool,
}

impl BudgetPolicy {
    pub fn evaluate(&self, ledger: &TokenLedger) -> BudgetStatus {
        let used_tokens = ledger.totals().tokens;
        let percent_used = if self.session_tokens == 0 {
            100.0
        } else {
            used_tokens as f64 * 100.0 / self.session_tokens as f64
        };
        BudgetStatus {
            used_tokens,
            limit_tokens: self.session_tokens,
            percent_used,
            warning: percent_used >= f64::from(self.warn_at_percent),
            stopped: percent_used >= f64::from(self.stop_at_percent),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UsageRecord;

    #[test]
    fn budget_warns_at_threshold() {
        let mut ledger = TokenLedger::default();
        ledger.push(UsageRecord {
            agent_id: "a".into(),
            phase: "p".into(),
            model: "m".into(),
            input_tokens: 75,
            output_tokens: 0,
            cached_tokens: 0,
            reasoning_tokens: 0,
            estimated: true,
            credit_multiplier: 1.0,
        });

        let status = BudgetPolicy {
            session_tokens: 100,
            session_usd: None,
            warn_at_percent: 75,
            stop_at_percent: 95,
        }
        .evaluate(&ledger);
        assert!(status.warning);
        assert!(!status.stopped);
    }
}
