use chrono::Utc;
use gorsee_code_artifacts::ArtifactRecord;
use gorsee_code_core::AgentProfile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRunSummary {
    pub session_id: String,
    pub events: usize,
    pub agents: Vec<String>,
    pub artifacts: Vec<ArtifactRecord>,
}

pub(crate) fn build_summary(
    session_id: String,
    events: usize,
    agents: Vec<AgentProfile>,
    artifacts: Vec<ArtifactRecord>,
) -> TaskRunSummary {
    TaskRunSummary {
        session_id,
        events,
        agents: agents
            .into_iter()
            .map(|agent| agent.id().to_string())
            .collect(),
        artifacts,
    }
}

pub(crate) fn session_id(title: &str) -> String {
    let stamp = Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let slug = title_slug(title);
    format!("{stamp}_{slug}")
}

fn title_slug(title: &str) -> String {
    title
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == ' ')
        .collect::<String>()
        .split_whitespace()
        .take(4)
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase()
}
