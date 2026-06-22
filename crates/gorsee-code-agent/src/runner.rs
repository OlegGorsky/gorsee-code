use std::path::Path;

use gorsee_code_coding_core::{LocalCodingProtocol, TurnRequest};
use gorsee_code_core::{default_agent_matrix, AgentProfile, TaskSpec};
use gorsee_code_safety::Redactor;
use gorsee_code_session::{ApprovalDecision, SessionStore};
use gorsee_code_tools::builtin_registry;

use crate::{
    artifact_events::record_artifact_outcomes,
    budget_events::{
        append_token_ledger, record_budget_status, sync_manifest_budget, write_token_ledger,
    },
    client::ChatClient,
    events::{EventObserver, EventSink},
    execution::ExecutionOutput,
    runner_execute::{execute, ExecuteInput},
    runner_finish::{finish_success, finish_turn_success, finish_unsuccessful, manifest_for},
    session_artifacts::{write_run_artifacts, write_session_snapshots, RunArtifactsInput},
    summary::{build_summary, session_id, TaskRunSummary},
    turn_response::{build_lcp_response_from_store, waiting_summary, TaskTurnOutput},
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

    pub fn run_turn_with_agents<C: ChatClient>(
        &self,
        session_id: &str,
        spec: &TaskSpec,
        client: &C,
        agents: Vec<AgentProfile>,
    ) -> Result<TaskRunSummary, AgentRunError> {
        self.run_existing_session(session_id, spec, None, client, agents, None)
    }

    pub fn run_turn_with_agents_observed<C: ChatClient>(
        &self,
        session_id: &str,
        spec: &TaskSpec,
        client: &C,
        agents: Vec<AgentProfile>,
        observer: Box<EventObserver>,
    ) -> Result<TaskRunSummary, AgentRunError> {
        self.run_existing_session(session_id, spec, None, client, agents, Some(observer))
    }

    pub fn run_lcp_turn<C: ChatClient>(
        &self,
        request: TurnRequest,
        client: &C,
        profiles: Vec<AgentProfile>,
    ) -> Result<TaskTurnOutput, AgentRunError> {
        self.run_lcp_turn_inner(request, client, profiles, None)
    }

    pub fn run_lcp_turn_observed<C: ChatClient>(
        &self,
        request: TurnRequest,
        client: &C,
        profiles: Vec<AgentProfile>,
        observer: Box<EventObserver>,
    ) -> Result<TaskTurnOutput, AgentRunError> {
        self.run_lcp_turn_inner(request, client, profiles, Some(observer))
    }

    fn run_lcp_turn_inner<C: ChatClient>(
        &self,
        request: TurnRequest,
        client: &C,
        profiles: Vec<AgentProfile>,
        observer: Option<Box<EventObserver>>,
    ) -> Result<TaskTurnOutput, AgentRunError> {
        let orchestration = LocalCodingProtocol::default()
            .plan_turn(request, profiles)
            .orchestration;
        let spec = TaskSpec::new(
            orchestration.request.message.clone(),
            orchestration.request.workspace.root.clone(),
        );
        let agents = orchestration.agents.clone();
        let session_id = orchestration
            .request
            .workspace
            .session_id
            .clone()
            .unwrap_or_else(|| session_id(&spec.title));
        let summary = match orchestration.request.workspace.session_id.as_deref() {
            Some(_) => self.run_existing_session(
                &session_id,
                &spec,
                None,
                client,
                agents.clone(),
                observer,
            ),
            None => {
                self.run_new_session(&session_id, &spec, None, client, agents.clone(), observer)
            }
        };
        match summary {
            Ok(summary) => self.turn_output(orchestration, summary),
            Err(AgentRunError::WaitingApproval(_)) => {
                let summary = waiting_summary(&self.store, session_id, agents)?;
                self.turn_output(orchestration, summary)
            }
            Err(error) => Err(error),
        }
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
        let session_id = session_id(&spec.title);
        self.run_new_session(&session_id, spec, skill_id, client, agents, None)
    }

    fn run_new_session<C: ChatClient>(
        &self,
        session_id: &str,
        spec: &TaskSpec,
        skill_id: Option<&str>,
        client: &C,
        agents: Vec<AgentProfile>,
        observer: Option<Box<EventObserver>>,
    ) -> Result<TaskRunSummary, AgentRunError> {
        let registry = builtin_registry(&spec.repo_path)?;
        let mut manifest = manifest_for(session_id, spec, &agents);
        let session_dir = self.store.create(&manifest)?;
        let mut sink = EventSink::new(&self.store, session_id.to_string());
        if let Some(observer) = observer {
            sink = sink.with_observer(observer);
        }
        let result = execute(
            &self.store,
            ExecuteInput {
                session_id,
                spec,
                skill_id,
                client,
                agents: &agents,
                registry: &registry,
                sink: &mut sink,
            },
        );
        match result {
            Ok(output) => {
                let ExecutionOutput {
                    answers,
                    tool_results,
                    usage_records,
                    turn_plan,
                    execution_contract,
                } = output;
                sync_manifest_budget(&mut manifest, &usage_records);
                if let Err(error) = record_budget_status(&mut sink, &manifest, &usage_records) {
                    finish_unsuccessful(&self.store, &mut sink, &mut manifest, &error)?;
                    return Err(error);
                }
                write_token_ledger(&session_dir, &usage_records)?;
                let run_artifacts = write_run_artifacts(RunArtifactsInput {
                    session_dir: &session_dir,
                    manifest: &manifest,
                    spec,
                    skill_id,
                    answers: &answers,
                    results: &tool_results,
                    usage_records: &usage_records,
                    plan: turn_plan.as_ref(),
                    contract: &execution_contract,
                })?;
                record_artifact_outcomes(&mut sink, &run_artifacts)?;
                let mut artifacts = run_artifacts.records;
                finish_success(&self.store, &mut sink, &mut manifest, skill_id, &artifacts)?;
                artifacts.extend(write_session_snapshots(&session_dir)?);
                Ok(build_summary(
                    session_id.to_string(),
                    sink.count(),
                    agents,
                    artifacts,
                ))
            }
            Err(error) => {
                finish_unsuccessful(&self.store, &mut sink, &mut manifest, &error)?;
                Err(error)
            }
        }
    }

    fn run_existing_session<C: ChatClient>(
        &self,
        session_id: &str,
        spec: &TaskSpec,
        skill_id: Option<&str>,
        client: &C,
        agents: Vec<AgentProfile>,
        observer: Option<Box<EventObserver>>,
    ) -> Result<TaskRunSummary, AgentRunError> {
        let registry = builtin_registry(&spec.repo_path)?;
        let mut manifest = self.store.read_manifest(session_id)?;
        manifest.status = "running".into();
        manifest.repo = spec.repo_path.clone();
        manifest.agents = agents.iter().map(|agent| agent.id().to_string()).collect();
        manifest.budget.tokens_limit = spec.budget_tokens;
        self.store.write_manifest(&manifest)?;
        let session_dir = self.store.session_dir(session_id);
        let mut sink = EventSink::resume(&self.store, session_id.to_string())?;
        if let Some(observer) = observer {
            sink = sink.with_observer(observer);
        }
        let result = execute(
            &self.store,
            ExecuteInput {
                session_id,
                spec,
                skill_id,
                client,
                agents: &agents,
                registry: &registry,
                sink: &mut sink,
            },
        );
        match result {
            Ok(output) => {
                let ExecutionOutput {
                    answers,
                    tool_results,
                    usage_records,
                    turn_plan,
                    execution_contract,
                } = output;
                let usage_records = append_token_ledger(&session_dir, &usage_records)?;
                sync_manifest_budget(&mut manifest, &usage_records);
                if let Err(error) = record_budget_status(&mut sink, &manifest, &usage_records) {
                    finish_unsuccessful(&self.store, &mut sink, &mut manifest, &error)?;
                    return Err(error);
                }
                let run_artifacts = write_run_artifacts(RunArtifactsInput {
                    session_dir: &session_dir,
                    manifest: &manifest,
                    spec,
                    skill_id,
                    answers: &answers,
                    results: &tool_results,
                    usage_records: &usage_records,
                    plan: turn_plan.as_ref(),
                    contract: &execution_contract,
                })?;
                record_artifact_outcomes(&mut sink, &run_artifacts)?;
                let mut artifacts = run_artifacts.records;
                finish_turn_success(&self.store, &mut sink, &mut manifest, skill_id, &artifacts)?;
                artifacts.extend(write_session_snapshots(&session_dir)?);
                Ok(build_summary(
                    session_id.to_string(),
                    sink.count(),
                    agents,
                    artifacts,
                ))
            }
            Err(error) => {
                finish_unsuccessful(&self.store, &mut sink, &mut manifest, &error)?;
                Err(error)
            }
        }
    }

    fn turn_output(
        &self,
        orchestration: gorsee_code_coding_core::OrchestrationPlan,
        summary: TaskRunSummary,
    ) -> Result<TaskTurnOutput, AgentRunError> {
        let response =
            build_lcp_response_from_store(&self.store, orchestration.clone(), &summary.session_id)?;
        Ok(TaskTurnOutput {
            orchestration,
            response,
            summary,
        })
    }
}
