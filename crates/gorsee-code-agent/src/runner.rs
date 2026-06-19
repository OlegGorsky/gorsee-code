use std::path::Path;

use gorsee_code_artifacts::ArtifactRecord;
use gorsee_code_core::{default_agent_matrix, AgentProfile, EventKind, TaskSpec};
use gorsee_code_safety::Redactor;
use gorsee_code_session::{ApprovalDecision, SessionManifest, SessionStore};
use gorsee_code_tools::builtin_registry;
use serde_json::json;

use crate::{
    agent_loop::{run_agent, AgentOutcome, AgentRunContext},
    budget_events::{record_budget_status, sync_manifest_budget},
    client::ChatClient,
    events::EventSink,
    execution::{save_pending_execution, ExecutionOutput, PendingExecution},
    session_artifacts::{write_run_artifacts, write_session_snapshots},
    summary::{build_summary, session_id, TaskRunSummary},
    AgentRunError,
};

#[derive(Debug, Clone)]
pub struct TaskRunner {
    store: SessionStore,
}

impl TaskRunner {
    pub fn new(session_root: impl AsRef<Path>) -> Self {
        Self {
            store: SessionStore::new(session_root, Redactor::default()),
        }
    }

    pub fn run_sequential<C: ChatClient>(
        &self,
        spec: &TaskSpec,
        client: &C,
    ) -> Result<TaskRunSummary, AgentRunError> {
        self.run(spec, None, client, default_agent_matrix())
    }

    pub fn run_sequential_with_agents<C: ChatClient>(
        &self,
        spec: &TaskSpec,
        client: &C,
        agents: Vec<AgentProfile>,
    ) -> Result<TaskRunSummary, AgentRunError> {
        self.run(spec, None, client, agents)
    }

    pub fn run_skill<C: ChatClient>(
        &self,
        spec: &TaskSpec,
        skill_id: &str,
        client: &C,
    ) -> Result<TaskRunSummary, AgentRunError> {
        self.run(spec, Some(skill_id), client, default_agent_matrix())
    }

    pub fn run_skill_with_agents<C: ChatClient>(
        &self,
        spec: &TaskSpec,
        skill_id: &str,
        client: &C,
        agents: Vec<AgentProfile>,
    ) -> Result<TaskRunSummary, AgentRunError> {
        self.run(spec, Some(skill_id), client, agents)
    }

    pub fn resume_after_decision<C: ChatClient>(
        &self,
        session_id: &str,
        approval_id: &str,
        decision: ApprovalDecision,
        client: &C,
    ) -> Result<TaskRunSummary, AgentRunError> {
        crate::resume::resume_after_decision(&self.store, session_id, approval_id, decision, client)
    }

    fn run<C: ChatClient>(
        &self,
        spec: &TaskSpec,
        skill_id: Option<&str>,
        client: &C,
        agents: Vec<AgentProfile>,
    ) -> Result<TaskRunSummary, AgentRunError> {
        let registry = builtin_registry(&spec.repo_path)?;
        let session_id = session_id(&spec.title);
        let mut manifest = manifest_for(&session_id, spec, &agents);
        let session_dir = self.store.create(&manifest)?;
        let mut sink = EventSink::new(&self.store, session_id.clone());
        let result = self.execute(ExecuteInput {
            session_id: &session_id,
            spec,
            skill_id,
            client,
            agents: &agents,
            registry: &registry,
            sink: &mut sink,
        });
        match result {
            Ok(output) => {
                let ExecutionOutput {
                    answers,
                    tool_results,
                    usage_records,
                } = output;
                sync_manifest_budget(&mut manifest, &usage_records);
                if let Err(error) = record_budget_status(&mut sink, &manifest, &usage_records) {
                    finish_unsuccessful(&self.store, &mut sink, &mut manifest, &error)?;
                    return Err(error);
                }
                let mut artifacts = write_run_artifacts(
                    &session_dir,
                    &manifest,
                    spec,
                    skill_id,
                    &answers,
                    &tool_results,
                )?;
                finish_success(&self.store, &mut sink, &mut manifest, skill_id, &artifacts)?;
                artifacts.extend(write_session_snapshots(&session_dir)?);
                Ok(build_summary(session_id, sink.count(), agents, artifacts))
            }
            Err(error) => {
                finish_unsuccessful(&self.store, &mut sink, &mut manifest, &error)?;
                Err(error)
            }
        }
    }

    fn execute<C: ChatClient>(
        &self,
        input: ExecuteInput<'_, '_, C>,
    ) -> Result<ExecutionOutput, AgentRunError> {
        let ExecuteInput {
            session_id,
            spec,
            skill_id,
            client,
            agents,
            registry,
            sink,
        } = input;

        start_events(sink, spec, skill_id)?;
        let mut answers = Vec::new();
        let mut tool_results = Vec::new();
        let mut usage_records = Vec::new();
        for (agent_index, agent) in agents.iter().enumerate() {
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
            match outcome {
                AgentOutcome::Finished {
                    answer,
                    tool_results: agent_tool_results,
                    usage_records: agent_usage_records,
                } => {
                    tool_results.extend(agent_tool_results);
                    usage_records.extend(agent_usage_records);
                    answers.push(answer);
                    record_context_update(sink, agent, answers.len(), tool_results.len())?;
                }
                AgentOutcome::Waiting(pending) => {
                    let approval_id = pending.approval_id.clone();
                    let snapshot =
                        PendingExecution::from_parts(crate::execution::PendingExecutionParts {
                            session_id: session_id.to_string(),
                            spec,
                            skill_id,
                            agents,
                            agent_index,
                            answers: &answers,
                            global_tool_results: &tool_results,
                            global_usage_records: &usage_records,
                            pending,
                        });
                    save_pending_execution(&self.store, &snapshot)?;
                    return Err(AgentRunError::WaitingApproval(approval_id));
                }
            }
        }
        Ok(ExecutionOutput {
            answers,
            tool_results,
            usage_records,
        })
    }
}

struct ExecuteInput<'a, 'sink, C: ChatClient> {
    session_id: &'a str,
    spec: &'a TaskSpec,
    skill_id: Option<&'a str>,
    client: &'a C,
    agents: &'a [AgentProfile],
    registry: &'a gorsee_code_tool_runtime::ToolRegistry,
    sink: &'a mut EventSink<'sink>,
}

fn start_events(
    sink: &mut EventSink<'_>,
    spec: &TaskSpec,
    skill_id: Option<&str>,
) -> Result<(), AgentRunError> {
    sink.push(
        None,
        EventKind::SessionStarted,
        json!({ "objective": spec.objective }),
    )?;
    if let Some(skill_id) = skill_id {
        sink.push(None, EventKind::SkillStarted, json!({ "skill": skill_id }))?;
    }
    Ok(())
}

pub(crate) fn record_context_update(
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

pub(crate) fn finish_success(
    store: &SessionStore,
    sink: &mut EventSink<'_>,
    manifest: &mut SessionManifest,
    skill_id: Option<&str>,
    artifacts: &[ArtifactRecord],
) -> Result<(), AgentRunError> {
    for artifact in artifacts {
        sink.push(
            None,
            EventKind::ArtifactCreated,
            json!({ "artifact": artifact }),
        )?;
    }
    if let Some(skill_id) = skill_id {
        sink.push(
            None,
            EventKind::SkillFinished,
            json!({ "skill": skill_id, "status": "finished" }),
        )?;
    }
    sink.push(
        None,
        EventKind::SessionFinished,
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

pub(crate) fn finish_unsuccessful(
    store: &SessionStore,
    sink: &mut EventSink<'_>,
    manifest: &mut SessionManifest,
    error: &AgentRunError,
) -> Result<(), AgentRunError> {
    match error {
        AgentRunError::WaitingApproval(_) => finish_waiting_approval(store, manifest),
        _ => finish_failure(store, sink, manifest, error),
    }
}

fn finish_waiting_approval(
    store: &SessionStore,
    manifest: &mut SessionManifest,
) -> Result<(), AgentRunError> {
    manifest.status = "waiting_approval".into();
    store.write_manifest(manifest)?;
    Ok(())
}

fn manifest_for(session_id: &str, spec: &TaskSpec, agents: &[AgentProfile]) -> SessionManifest {
    let mut manifest = SessionManifest::new(session_id, &spec.repo_path, "unknown");
    manifest.agents = agents.iter().map(|agent| agent.id().to_string()).collect();
    manifest.budget.tokens_limit = spec.budget_tokens;
    manifest
}
