use gorsee_code_core::{AgentProfile, EventKind, TaskSpec};
use gorsee_code_session::SessionStore;
use gorsee_code_tool_runtime::ToolRegistry;
use serde_json::json;

use crate::{
    agent_loop::{run_agent, AgentOutcome, AgentRunContext},
    client::ChatClient,
    events::EventSink,
    execution::{save_pending_execution, ExecutionOutput, PendingExecution},
    turn_contract::turn_execution_context,
    AgentRunError,
};

pub(crate) struct ExecuteInput<'a, 'sink, C: ChatClient> {
    pub(crate) session_id: &'a str,
    pub(crate) spec: &'a TaskSpec,
    pub(crate) skill_id: Option<&'a str>,
    pub(crate) client: &'a C,
    pub(crate) agents: &'a [AgentProfile],
    pub(crate) registry: &'a ToolRegistry,
    pub(crate) sink: &'a mut EventSink<'sink>,
}

pub(crate) fn execute<C: ChatClient>(
    store: &SessionStore,
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
    let turn_context = turn_execution_context(spec, agents);
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
                turn_plan: turn_context.plan.as_ref(),
                execution_contract: &turn_context.contract,
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
                save_pending_execution(store, &snapshot)?;
                return Err(AgentRunError::WaitingApproval(approval_id));
            }
        }
    }
    Ok(ExecutionOutput {
        answers,
        tool_results,
        usage_records,
        turn_plan: turn_context.plan,
        execution_contract: turn_context.contract,
    })
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

fn start_events(
    sink: &mut EventSink<'_>,
    spec: &TaskSpec,
    skill_id: Option<&str>,
) -> Result<(), AgentRunError> {
    sink.push(
        None,
        EventKind::TurnStarted,
        json!({ "objective": spec.objective }),
    )?;
    if let Some(skill_id) = skill_id {
        sink.push(None, EventKind::SkillStarted, json!({ "skill": skill_id }))?;
    }
    Ok(())
}
