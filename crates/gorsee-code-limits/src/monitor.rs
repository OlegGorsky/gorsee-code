use serde::{Deserialize, Serialize};

use crate::UsageWindow;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LimitPolicy {
    pub warn_credit_percent: f64,
    pub warn_request_percent: f64,
    pub stop_credit_percent: f64,
    pub stop_request_percent: f64,
}

impl Default for LimitPolicy {
    fn default() -> Self {
        Self {
            warn_credit_percent: 75.0,
            warn_request_percent: 75.0,
            stop_credit_percent: 95.0,
            stop_request_percent: 95.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LimitDecision {
    Continue,
    Warn(String),
    Stop(String),
}

impl LimitPolicy {
    pub fn evaluate(&self, windows: &[UsageWindow]) -> LimitDecision {
        let mut warning = None;
        for window in windows {
            let credit = window.credit_percent();
            let request = window.request_percent();
            if credit >= self.stop_credit_percent || request >= self.stop_request_percent {
                return LimitDecision::Stop(window.label.clone());
            }
            if credit >= self.warn_credit_percent || request >= self.warn_request_percent {
                warning = Some(window.label.clone());
            }
        }
        warning
            .map(LimitDecision::Warn)
            .unwrap_or(LimitDecision::Continue)
    }
}
