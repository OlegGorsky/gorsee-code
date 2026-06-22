use gorsee_code_core::AgentProfile;
use gorsee_code_session::{ApprovalDecision, SessionStore};
use gorsee_code_tool_runtime::ToolRegistry;
use gorsee_code_tools::builtin_registry;
use gorsee_code_usage::UsageRecord;

use crate::{
    agent_loop::{resume_agent, run_agent, AgentOutcome, AgentRunContext, PendingApproval},
    artifact_events::record_artifact_outcomes,
    budget_events::{record_budget_status, sync_manifest_budget, write_token_ledger},
    client::ChatClient,
    events::EventSink,
    execution::{clear_pending_execution, load_pending_execution, PendingExecution},
    protocol::{AgentAnswer, ToolResult},
    resume_pending::{save_waiting, validate_pending},
    resume_types::{
        AgentResumeInput, FlowSuccess, PendingSaveInput, RemainingAgentsInput, ResumeState,
    },
    runner_execute::record_context_update,
    runner_finish::{finish_success, finish_unsuccessful},
    session_artifacts::{write_run_artifacts, write_session_snapshots, RunArtifactsInput},
    summary::{build_summary, TaskRunSummary},
    turn_contract::turn_execution_context,
    AgentRunError,
};

pub(crate) fn resume_after_decision<C: ChatClient>(
    store: &SessionStore,
    session_id: &str,
    approval_id: &str,
    decision: ApprovalDecision,
    client: &C,
) -> Result<TaskRunSummary, AgentRunError> {
    let pending = load_pending_execution(store, session_id)?;
    validate_pending(&pending, session_id, approval_id)?;
    store.decide_approval(session_id, approval_id, decision)?;

    let mut manifest = store.read_manifest(session_id)?;
    let registry = builtin_registry(&pending.spec.repo_path)?;
    let mut sink = EventSink::resume(store, session_id.to_string())?;
    let result = resume_flow(store, pending, decision, client, &registry, &mut sink);

    match result {
        Ok((spec, skill_id, agents, answers, tool_results, usage_records)) => {
            let session_dir = store.session_dir(session_id);
            sync_manifest_budget(&mut manifest, &usage_records);
            if let Err(error) = record_budget_status(&mut sink, &manifest, &usage_records) {
                finish_unsuccessful(store, &mut sink, &mut manifest, &error)?;
                return Err(error);
            }
            write_token_ledger(&session_dir, &usage_records)?;
            let turn_context = turn_execution_context(&spec, &agents);
            let run_artifacts = write_run_artifacts(RunArtifactsInput {
                session_dir: &session_dir,
                manifest: &manifest,
                spec: &spec,
                skill_id: skill_id.as_deref(),
                answers: &answers,
                results: &tool_results,
                usage_records: &usage_records,
                plan: turn_context.plan.as_ref(),
                contract: &turn_context.contract,
            })?;
            record_artifact_outcomes(&mut sink, &run_artifacts)?;
            let mut artifacts = run_artifacts.records;
            finish_success(
                store,
                &mut sink,
                &mut manifest,
                skill_id.as_deref(),
                &artifacts,
            )?;
            artifacts.extend(write_session_snapshots(&session_dir)?);
            clear_pending_execution(store, session_id)?;
            Ok(build_summary(
                session_id.to_string(),
                sink.count(),
                agents,
                artifacts,
            ))
        }
        Err(error) => {
            finish_unsuccessful(store, &mut sink, &mut manifest, &error)?;
            Err(error)
        }
    }
}

fn resume_flow<C: ChatClient>(
    store: &SessionStore,
    pending: PendingExecution,
    decision: ApprovalDecision,
    client: &C,
    registry: &ToolRegistry,
    sink: &mut EventSink<'_>,
) -> Result<FlowSuccess, AgentRunError> {
    let session_id = pending.session_id.clone();
    let spec = pending.spec.clone();
    let skill_id = pending.skill_id.clone();
    let agents = pending.agents.clone();
    let agent_index = pending.agent_index;
    let turn_context = turn_execution_context(&spec, &agents);
    let mut answers = pending.answers.clone();
    let mut tool_results = pending.global_tool_results.clone();
    let mut usage_records = pending.global_usage_records.clone();
    let waiting = pending.pending_approval();

    let agent = agent_at(&agents, agent_index)?;
    match resume_current_agent(
        AgentResumeInput {
            client,
            registry,
            sink,
            spec: &spec,
            skill_id: skill_id.as_deref(),
            agent,
            turn_plan: turn_context.plan.as_ref(),
            execution_contract: &turn_context.contract,
        },
        &mut answers,
        &mut tool_results,
        &mut usage_records,
        waiting,
        decision,
    )? {
        ResumeState::Finished => {}
        ResumeState::Waiting(waiting) => {
            let approval_id = save_waiting(
                PendingSaveInput {
                    store,
                    session_id: &session_id,
                    spec: &spec,
                    skill_id: skill_id.as_deref(),
                    agents: &agents,
                    agent_index,
                    answers: &answers,
                    tool_results: &tool_results,
                    usage_records: &usage_records,
                },
                *waiting,
            )?;
            return Err(AgentRunError::WaitingApproval(approval_id));
        }
    }

    run_remaining_agents(
        RemainingAgentsInput {
            store,
            session_id: &session_id,
            spec: &spec,
            skill_id: skill_id.as_deref(),
            client,
            registry,
            sink,
            agents: &agents,
            first_index: agent_index + 1,
            turn_plan: turn_context.plan.as_ref(),
            execution_contract: &turn_context.contract,
        },
        &mut answers,
        &mut tool_results,
        &mut usage_records,
    )?;

    Ok((spec, skill_id, agents, answers, tool_results, usage_records))
}

fn resume_current_agent<C: ChatClient>(
    input: AgentResumeInput<'_, '_, C>,
    answers: &mut Vec<AgentAnswer>,
    tool_results: &mut Vec<ToolResult>,
    usage_records: &mut Vec<UsageRecord>,
    waiting: PendingApproval,
    decision: ApprovalDecision,
) -> Result<ResumeState, AgentRunError> {
    let AgentResumeInput {
        client,
        registry,
        sink,
        spec,
        skill_id,
        agent,
        turn_plan,
        execution_contract,
    } = input;

    let outcome = resume_agent(
        AgentRunContext {
            spec,
            skill_id,
            client,
            agent,
            registry,
            previous_answers: answers,
            previous_tool_results: tool_results,
            turn_plan,
            execution_contract,
        },
        sink,
        waiting,
        decision,
    )?;
    handle_outcome(sink, agent, answers, tool_results, usage_records, outcome)
}

fn run_remaining_agents<C: ChatClient>(
    input: RemainingAgentsInput<'_, '_, C>,
    answers: &mut Vec<AgentAnswer>,
    tool_results: &mut Vec<ToolResult>,
    usage_records: &mut Vec<UsageRecord>,
) -> Result<(), AgentRunError> {
    let RemainingAgentsInput {
        store,
        session_id,
        spec,
        skill_id,
        client,
        registry,
        sink,
        agents,
        first_index,
        turn_plan,
        execution_contract,
    } = input;

    for (agent_index, agent) in agents.iter().enumerate().skip(first_index) {
        let outcome = run_agent(
            AgentRunContext {
                spec,
                skill_id,
                client,
                agent,
                registry,
                previous_answers: answers,
                previous_tool_results: tool_results,
                turn_plan,
                execution_contract,
            },
            sink,
        )?;
        if let ResumeState::Waiting(waiting) =
            handle_outcome(sink, agent, answers, tool_results, usage_records, outcome)?
        {
            let approval_id = save_waiting(
                PendingSaveInput {
                    store,
                    session_id,
                    spec,
                    skill_id,
                    agents,
                    agent_index,
                    answers,
                    tool_results,
                    usage_records,
                },
                *waiting,
            )?;
            return Err(AgentRunError::WaitingApproval(approval_id));
        }
    }
    Ok(())
}

fn handle_outcome(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    answers: &mut Vec<AgentAnswer>,
    tool_results: &mut Vec<ToolResult>,
    usage_records: &mut Vec<UsageRecord>,
    outcome: AgentOutcome,
) -> Result<ResumeState, AgentRunError> {
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
            Ok(ResumeState::Finished)
        }
        AgentOutcome::Waiting(waiting) => Ok(ResumeState::Waiting(Box::new(waiting))),
    }
}

fn agent_at(agents: &[AgentProfile], index: usize) -> Result<&AgentProfile, AgentRunError> {
    agents
        .get(index)
        .ok_or_else(|| AgentRunError::Runtime(format!("missing agent at index {index}")))
}
