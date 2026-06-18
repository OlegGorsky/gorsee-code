use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionStatus {
    Created,
    Running,
    Paused,
    Finished,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionSpec {
    pub title: String,
    pub objective: String,
    pub repo_path: String,
    pub budget_tokens: u64,
}

impl MissionSpec {
    pub fn new(objective: impl Into<String>, repo_path: impl Into<String>) -> Self {
        let objective = objective.into();
        Self {
            title: objective.chars().take(80).collect(),
            objective,
            repo_path: repo_path.into(),
            budget_tokens: 80_000,
        }
    }
}
