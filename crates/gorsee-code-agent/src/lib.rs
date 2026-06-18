use std::path::Path;

use chrono::Utc;
use gorsee_code_artifacts::{ArtifactError, ArtifactRecord, ArtifactStore};
use gorsee_code_core::{default_agent_matrix, Event, EventKind, MissionSpec};
use gorsee_code_safety::Redactor;
use gorsee_code_session::{SessionManifest, SessionStore, SessionStoreError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentRunError {
    #[error("session failed: {0}")]
    Session(#[from] SessionStoreError),
    #[error("artifact failed: {0}")]
    Artifact(#[from] ArtifactError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionRunSummary {
    pub session_id: String,
    pub events: usize,
    pub agents: Vec<String>,
    pub artifacts: Vec<ArtifactRecord>,
}

#[derive(Debug, Clone)]
pub struct MissionRunner {
    store: SessionStore,
}

impl MissionRunner {
    pub fn new(session_root: impl AsRef<Path>) -> Self {
        Self {
            store: SessionStore::new(session_root, Redactor::default()),
        }
    }

    pub fn run_sequential(&self, spec: &MissionSpec) -> Result<MissionRunSummary, AgentRunError> {
        self.run(spec, None)
    }

    pub fn run_skill(
        &self,
        spec: &MissionSpec,
        skill_id: &str,
    ) -> Result<MissionRunSummary, AgentRunError> {
        self.run(spec, Some(skill_id))
    }

    fn run(
        &self,
        spec: &MissionSpec,
        skill_id: Option<&str>,
    ) -> Result<MissionRunSummary, AgentRunError> {
        let session_id = session_id(&spec.title);
        let mut manifest = SessionManifest::new(&session_id, &spec.repo_path, "unknown");
        let session_dir = self.store.create(&manifest)?;
        let artifact = write_report(&session_dir, spec, skill_id)?;
        let events = mission_events(&session_id, spec, skill_id, &artifact);
        for event in &events {
            self.store.append_event(event)?;
        }
        manifest.status = "finished".into();
        self.store.write_manifest(&manifest)?;
        Ok(MissionRunSummary {
            session_id,
            events: events.len(),
            agents: default_agent_matrix()
                .into_iter()
                .map(|agent| agent.id().into())
                .collect(),
            artifacts: vec![artifact],
        })
    }
}

fn mission_events(
    session_id: &str,
    spec: &MissionSpec,
    skill_id: Option<&str>,
    artifact: &ArtifactRecord,
) -> Vec<Event> {
    let mut events = Vec::new();
    push_event(
        &mut events,
        session_id,
        None,
        EventKind::MissionStarted,
        json!({ "message": spec.objective }),
    );
    push_skill_started(&mut events, session_id, skill_id);
    append_agent_events(session_id, &mut events);
    push_artifact_event(&mut events, session_id, artifact);
    push_skill_finished(&mut events, session_id, skill_id);
    push_event(
        &mut events,
        session_id,
        None,
        EventKind::MissionFinished,
        json!({ "message": "mission scaffold finished" }),
    );
    events
}

fn append_agent_events(session_id: &str, events: &mut Vec<Event>) {
    for profile in default_agent_matrix() {
        push_event(
            events,
            session_id,
            Some(profile.id().into()),
            EventKind::AgentStarted,
            json!({ "message": format!("{} ready", profile.id()), "model": profile.model }),
        );
    }
}

fn push_skill_started(events: &mut Vec<Event>, session_id: &str, skill_id: Option<&str>) {
    if let Some(skill_id) = skill_id {
        push_event(
            events,
            session_id,
            None,
            EventKind::SkillStarted,
            json!({ "skill": skill_id }),
        );
    }
}

fn push_artifact_event(events: &mut Vec<Event>, session_id: &str, artifact: &ArtifactRecord) {
    push_event(
        events,
        session_id,
        None,
        EventKind::ArtifactCreated,
        json!({
            "id": artifact.id.clone(),
            "path": artifact.path.clone(),
            "mime": artifact.mime.clone(),
        }),
    );
}

fn push_skill_finished(events: &mut Vec<Event>, session_id: &str, skill_id: Option<&str>) {
    if let Some(skill_id) = skill_id {
        push_event(
            events,
            session_id,
            None,
            EventKind::SkillFinished,
            json!({ "skill": skill_id, "status": "finished" }),
        );
    }
}

fn push_event(
    events: &mut Vec<Event>,
    session_id: &str,
    agent_id: Option<String>,
    kind: EventKind,
    payload: serde_json::Value,
) {
    let sequence = events.len() as u64 + 1;
    events.push(Event::new(sequence, session_id, agent_id, kind, payload));
}

fn write_report(
    session_dir: &Path,
    spec: &MissionSpec,
    skill_id: Option<&str>,
) -> Result<ArtifactRecord, ArtifactError> {
    let skill = skill_id.unwrap_or("none");
    let text = format!(
        "# Gorsee Code Mission Report\n\n- Objective: {}\n- Skill: {}\n- Budget tokens: {}\n",
        spec.objective, skill, spec.budget_tokens
    );
    ArtifactStore::new(session_dir.join("artifacts")).write_text("text/markdown", &text)
}

fn session_id(title: &str) -> String {
    let stamp = Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let slug = title
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == ' ')
        .collect::<String>()
        .split_whitespace()
        .take(4)
        .collect::<Vec<_>>()
        .join("-");
    format!("{stamp}_{}", slug.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequential_runner_records_events() {
        let temp = tempfile::tempdir().unwrap();
        let runner = MissionRunner::new(temp.path());
        let spec = MissionSpec::new("fix auth", temp.path().display().to_string());
        let summary = runner.run_sequential(&spec).unwrap();
        assert!(summary.events >= 7);
        assert_eq!(summary.artifacts.len(), 1);
        assert!(Path::new(&summary.artifacts[0].path).exists());
        assert!(temp
            .path()
            .join("sessions")
            .join(summary.session_id)
            .exists());
    }
}
