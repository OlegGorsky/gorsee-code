use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetSnapshot {
    pub tokens_limit: u64,
    pub tokens_used: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionManifest {
    pub id: String,
    pub repo: String,
    pub branch: String,
    pub started_at: DateTime<Utc>,
    pub status: String,
    pub agents: Vec<String>,
    pub budget: BudgetSnapshot,
}

impl SessionManifest {
    pub fn new(id: impl Into<String>, repo: impl Into<String>, branch: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            repo: repo.into(),
            branch: branch.into(),
            started_at: Utc::now(),
            status: "running".into(),
            agents: vec![
                "architect".into(),
                "scout".into(),
                "coder".into(),
                "validator".into(),
            ],
            budget: BudgetSnapshot {
                tokens_limit: 80_000,
                tokens_used: 0,
            },
        }
    }
}
