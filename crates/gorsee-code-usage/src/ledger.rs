use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageRecord {
    pub agent_id: String,
    pub phase: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    pub reasoning_tokens: u64,
    pub estimated: bool,
    pub credit_multiplier: f64,
}

impl UsageRecord {
    pub fn used_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.reasoning_tokens
    }

    pub fn total_tokens(&self) -> u64 {
        self.used_tokens() + self.cached_tokens
    }

    pub fn weighted_credits(&self) -> f64 {
        self.used_tokens() as f64 * self.credit_multiplier
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TokenTotals {
    pub tokens: u64,
    pub cached_tokens: u64,
    pub weighted_credits: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TokenLedger {
    pub records: Vec<UsageRecord>,
}

impl TokenLedger {
    pub fn push(&mut self, record: UsageRecord) {
        self.records.push(record);
    }

    pub fn totals(&self) -> TokenTotals {
        self.records
            .iter()
            .fold(TokenTotals::default(), |mut totals, record| {
                totals.tokens += record.used_tokens();
                totals.cached_tokens += record.cached_tokens;
                totals.weighted_credits += record.weighted_credits();
                totals
            })
    }

    pub fn by_agent(&self) -> BTreeMap<String, TokenTotals> {
        let mut map = BTreeMap::new();
        for record in &self.records {
            let entry = map
                .entry(record.agent_id.clone())
                .or_insert_with(TokenTotals::default);
            entry.tokens += record.used_tokens();
            entry.cached_tokens += record.cached_tokens;
            entry.weighted_credits += record.weighted_credits();
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_totals_tokens_and_weighted_credits() {
        let mut ledger = TokenLedger::default();
        ledger.push(UsageRecord {
            agent_id: "coder".into(),
            phase: "patch".into(),
            model: "m".into(),
            input_tokens: 10,
            output_tokens: 5,
            cached_tokens: 2,
            reasoning_tokens: 3,
            estimated: false,
            credit_multiplier: 1.5,
        });

        let totals = ledger.totals();
        assert_eq!(totals.tokens, 18);
        assert_eq!(totals.cached_tokens, 2);
        assert_eq!(totals.weighted_credits, 27.0);
    }
}
