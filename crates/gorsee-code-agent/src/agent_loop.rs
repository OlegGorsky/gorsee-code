use gorsee_code_coding_core::{ExecutionContract, TurnPlan};
use gorsee_code_core::{AgentProfile, EventKind, TaskSpec};
use gorsee_code_session::ApprovalDecision;
use gorsee_code_tool_runtime::ToolRegistry;
use gorsee_code_usage::UsageRecord;
use serde_json::json;

use crate::{
    agent_tools::{allowed_manifests, run_decided_tool, run_tools_until_wait, ToolBatch},
    budget_events::usage_record_from_response,
    client::ChatClient,
    events::EventSink,
    final_policy::final_answer_policy_retry,
    prompts::PromptContext,
    protocol::{parse_response, AgentAnswer, ModelToolCall, ToolResult},
    tool_events::record_message,
    AgentRunError,
};

const MAX_STEPS: usize = 8;

pub(crate) enum AgentOutcome {
    Finished {
        answer: AgentAnswer,
        tool_results: Vec<ToolResult>,
        usage_records: Vec<UsageRecord>,
    },
    Waiting(PendingApproval),
}

pub(crate) struct PendingApproval {
    pub(crate) approval_id: String,
    pub(crate) step: usize,
    pub(crate) pending_call: ModelToolCall,
    pub(crate) remaining_calls: Vec<ModelToolCall>,
    pub(crate) final_answer: Option<String>,
    pub(crate) tool_results: Vec<ToolResult>,
    pub(crate) usage_records: Vec<UsageRecord>,
}

pub(crate) struct AgentRunContext<'a, C: ChatClient> {
    pub(crate) spec: &'a TaskSpec,
    pub(crate) skill_id: Option<&'a str>,
    pub(crate) client: &'a C,
    pub(crate) agent: &'a AgentProfile,
    pub(crate) registry: &'a ToolRegistry,
    pub(crate) previous_answers: &'a [AgentAnswer],
    pub(crate) previous_tool_results: &'a [ToolResult],
    pub(crate) turn_plan: Option<&'a TurnPlan>,
    pub(crate) execution_contract: &'a ExecutionContract,
}

pub(crate) fn run_agent<C: ChatClient>(
    context: AgentRunContext<'_, C>,
    sink: &mut EventSink<'_>,
) -> Result<AgentOutcome, AgentRunError> {
    start_agent(sink, context.agent)?;
    continue_agent(context, sink, 1, Vec::new(), Vec::new())
}

pub(crate) fn resume_agent<C: ChatClient>(
    context: AgentRunContext<'_, C>,
    sink: &mut EventSink<'_>,
    pending: PendingApproval,
    decision: ApprovalDecision,
) -> Result<AgentOutcome, AgentRunError> {
    let mut tool_results = pending.tool_results;
    let usage_records = pending.usage_records;
    let manifests = allowed_manifests(context.agent, context.registry);
    let result = run_decided_tool(
        sink,
        context.agent,
        context.registry,
        &manifests,
        &pending.pending_call,
        &pending.approval_id,
        decision,
    )?;
    tool_results.push(result);
    if let Some(waiting) = run_tools_until_wait(
        sink,
        context.agent,
        context.registry,
        &manifests,
        ToolBatch {
            repo_path: &context.spec.repo_path,
            step: pending.step,
            calls: pending.remaining_calls,
            final_answer: pending.final_answer.clone(),
            previous_results: context.previous_tool_results,
            results: &mut tool_results,
            usage_records: &usage_records,
        },
    )? {
        return Ok(AgentOutcome::Waiting(waiting));
    }
    if let Some(answer) = pending.final_answer {
        if let Some(policy_feedback) = final_answer_policy_retry(
            context.agent,
            &context.spec.objective,
            &manifests,
            context.previous_tool_results,
            &tool_results,
            &answer,
            context.execution_contract,
        ) {
            tool_results.push(policy_feedback);
            return continue_agent(context, sink, pending.step + 1, tool_results, usage_records);
        }
        record_message(sink, context.agent, Some(&answer))?;
        return Ok(finished(context.agent, answer, tool_results, usage_records));
    }
    continue_agent(context, sink, pending.step + 1, tool_results, usage_records)
}

fn continue_agent<C: ChatClient>(
    context: AgentRunContext<'_, C>,
    sink: &mut EventSink<'_>,
    first_step: usize,
    mut tool_results: Vec<ToolResult>,
    mut usage_records: Vec<UsageRecord>,
) -> Result<AgentOutcome, AgentRunError> {
    for step in first_step..=MAX_STEPS {
        let manifests = allowed_manifests(context.agent, context.registry);
        let request = crate::prompts::request(PromptContext {
            profile: context.agent,
            spec: context.spec,
            skill_id: context.skill_id,
            tools: &manifests,
            previous_answers: context.previous_answers,
            previous_tool_results: context.previous_tool_results,
            results: &tool_results,
            turn_plan: context.turn_plan,
            execution_contract: context.execution_contract,
            step,
        });
        let response = context.client.complete(&request)?;
        if let Some(record) = usage_record_from_response(context.agent, &response) {
            usage_records.push(record);
        }
        let turn = parse_response(&response)?;
        if let Some(waiting) = run_tools_until_wait(
            sink,
            context.agent,
            context.registry,
            &manifests,
            ToolBatch {
                repo_path: &context.spec.repo_path,
                step,
                calls: turn.tool_calls,
                final_answer: turn.final_answer.clone(),
                previous_results: context.previous_tool_results,
                results: &mut tool_results,
                usage_records: &usage_records,
            },
        )? {
            return Ok(AgentOutcome::Waiting(waiting));
        }
        if let Some(answer) = turn.final_answer {
            if let Some(policy_feedback) = final_answer_policy_retry(
                context.agent,
                &context.spec.objective,
                &manifests,
                context.previous_tool_results,
                &tool_results,
                &answer,
                context.execution_contract,
            ) {
                tool_results.push(policy_feedback);
                continue;
            }
            record_message(sink, context.agent, Some(&answer))?;
            return Ok(finished(context.agent, answer, tool_results, usage_records));
        }
    }
    Err(AgentRunError::InvalidModelResponse(format!(
        "{} did not finish within step limit",
        context.agent.id()
    )))
}

fn finished(
    agent: &AgentProfile,
    answer: String,
    tool_results: Vec<ToolResult>,
    usage_records: Vec<UsageRecord>,
) -> AgentOutcome {
    AgentOutcome::Finished {
        answer: AgentAnswer::new(agent.id(), answer),
        tool_results,
        usage_records,
    }
}

fn start_agent(sink: &mut EventSink<'_>, agent: &AgentProfile) -> Result<(), AgentRunError> {
    sink.push(
        Some(agent.id()),
        EventKind::AgentStarted,
        json!({ "model": agent.model, "reasoning": agent.reasoning }),
    )?;
    Ok(())
}
