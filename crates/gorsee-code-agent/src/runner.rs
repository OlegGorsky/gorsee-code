use std::path::Path;

use chrono::Utc;
use gorsee_code_artifacts::ArtifactRecord;
use gorsee_code_core::{default_agent_matrix, AgentProfile, EventKind, MissionSpec};
use gorsee_code_safety::Redactor;
use gorsee_code_session::{SessionManifest, SessionStore};
use gorsee_code_tools::builtin_registry;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    agent_loop::{run_agent, AgentRunContext},
    client::ChatClient,
    events::EventSink,
    protocol::{AgentAnswer, ToolResult},
    report::write_report,
    AgentRunError,
};

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

    pub fn run_sequential<C: ChatClient>(
        &self,
        spec: &MissionSpec,
        client: &C,
    ) -> Result<MissionRunSummary, AgentRunError> {
        self.run(spec, None, client)
    }

    pub fn run_skill<C: ChatClient>(
        &self,
        spec: &MissionSpec,
        skill_id: &str,
        client: &C,
    ) -> Result<MissionRunSummary, AgentRunError> {
        self.run(spec, Some(skill_id), client)
    }

    fn run<C: ChatClient>(
        &self,
        spec: &MissionSpec,
        skill_id: Option<&str>,
        client: &C,
    ) -> Result<MissionRunSummary, AgentRunError> {
        let agents = default_agent_matrix();
        let registry = builtin_registry(&spec.repo_path)?;
        let session_id = session_id(&spec.title);
        let mut manifest = manifest_for(&session_id, spec, &agents);
        let session_dir = self.store.create(&manifest)?;
        let mut sink = EventSink::new(&self.store, session_id.clone());
        let result = self.execute(spec, skill_id, client, &agents, &registry, &mut sink);
        match result {
            Ok((answers, tool_results)) => {
                let artifact = write_report(&session_dir, spec, skill_id, &answers, &tool_results)?;
                finish_success(&self.store, &mut sink, &mut manifest, skill_id, &artifact)?;
                Ok(summary(session_id, sink.count(), agents, vec![artifact]))
            }
            Err(error) => {
                finish_failure(&self.store, &mut sink, &mut manifest, &error)?;
                Err(error)
            }
        }
    }

    fn execute<C: ChatClient>(
        &self,
        spec: &MissionSpec,
        skill_id: Option<&str>,
        client: &C,
        agents: &[AgentProfile],
        registry: &gorsee_code_tool_runtime::ToolRegistry,
        sink: &mut EventSink<'_>,
    ) -> Result<(Vec<AgentAnswer>, Vec<ToolResult>), AgentRunError> {
        start_events(sink, spec, skill_id)?;
        let mut answers = Vec::new();
        let mut tool_results = Vec::new();
        for agent in agents {
            let outcome = run_agent(
                AgentRunContext {
                    spec,
                    skill_id,
                    client,
                    agent,
                    registry,
                    previous_answers: &answers,
                    previous_tool_results: &tool_results,
                },
                sink,
            )?;
            tool_results.extend(outcome.tool_results);
            answers.push(outcome.answer);
            record_context_update(sink, agent, answers.len(), tool_results.len())?;
        }
        Ok((answers, tool_results))
    }
}

fn start_events(
    sink: &mut EventSink<'_>,
    spec: &MissionSpec,
    skill_id: Option<&str>,
) -> Result<(), AgentRunError> {
    sink.push(
        None,
        EventKind::MissionStarted,
        json!({ "objective": spec.objective }),
    )?;
    if let Some(skill_id) = skill_id {
        sink.push(None, EventKind::SkillStarted, json!({ "skill": skill_id }))?;
    }
    Ok(())
}

fn record_context_update(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    answers: usize,
    tool_results: usize,
) -> Result<(), AgentRunError> {
    sink.push(
        Some(agent.id()),
        EventKind::ContextUpdated,
        json!({ "answers": answers, "tool_results": tool_results }),
    )?;
    Ok(())
}

fn finish_success(
    store: &SessionStore,
    sink: &mut EventSink<'_>,
    manifest: &mut SessionManifest,
    skill_id: Option<&str>,
    artifact: &ArtifactRecord,
) -> Result<(), AgentRunError> {
    sink.push(
        None,
        EventKind::ArtifactCreated,
        json!({ "artifact": artifact }),
    )?;
    if let Some(skill_id) = skill_id {
        sink.push(
            None,
            EventKind::SkillFinished,
            json!({ "skill": skill_id, "status": "finished" }),
        )?;
    }
    sink.push(
        None,
        EventKind::MissionFinished,
        json!({ "status": "finished" }),
    )?;
    manifest.status = "finished".into();
    store.write_manifest(manifest)?;
    Ok(())
}

fn finish_failure(
    store: &SessionStore,
    sink: &mut EventSink<'_>,
    manifest: &mut SessionManifest,
    error: &AgentRunError,
) -> Result<(), AgentRunError> {
    sink.push(
        None,
        EventKind::Error,
        json!({ "error": error.to_string() }),
    )?;
    manifest.status = "failed".into();
    store.write_manifest(manifest)?;
    Ok(())
}

fn manifest_for(session_id: &str, spec: &MissionSpec, agents: &[AgentProfile]) -> SessionManifest {
    let mut manifest = SessionManifest::new(session_id, &spec.repo_path, "unknown");
    manifest.agents = agents.iter().map(|agent| agent.id().to_string()).collect();
    manifest.budget.tokens_limit = spec.budget_tokens;
    manifest
}

fn summary(
    session_id: String,
    events: usize,
    agents: Vec<AgentProfile>,
    artifacts: Vec<ArtifactRecord>,
) -> MissionRunSummary {
    MissionRunSummary {
        session_id,
        events,
        agents: agents
            .into_iter()
            .map(|agent| agent.id().to_string())
            .collect(),
        artifacts,
    }
}

fn session_id(title: &str) -> String {
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
