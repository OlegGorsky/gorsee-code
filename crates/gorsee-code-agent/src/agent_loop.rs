use gorsee_code_core::{AgentProfile, EventKind, TaskSpec};
use gorsee_code_session::ApprovalDecision;
use gorsee_code_tool_runtime::ToolRegistry;
use gorsee_code_usage::UsageRecord;
use serde_json::json;

use crate::{
    budget_events::usage_record_from_response,
    client::ChatClient,
    events::EventSink,
    prompts::PromptContext,
    protocol::{parse_response, AgentAnswer, ModelToolCall, ToolResult},
    tool_events::{
        record_approval_event, record_message, record_patch_applied, record_patch_proposal,
        record_tool_failure, record_tool_request, record_tool_success, redact_args,
    },
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
    let result = run_decided_tool(
        sink,
        context.agent,
        context.registry,
        &pending.pending_call,
        &pending.approval_id,
        decision,
    )?;
    tool_results.push(result);
    if let Some(waiting) = run_tools_until_wait(
        sink,
        context.agent,
        context.registry,
        ToolBatch {
            step: pending.step,
            calls: pending.remaining_calls,
            final_answer: pending.final_answer.clone(),
            results: &mut tool_results,
            usage_records: &usage_records,
        },
    )? {
        return Ok(AgentOutcome::Waiting(waiting));
    }
    if let Some(answer) = pending.final_answer {
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
        let manifests = context.registry.manifests();
        let request = crate::prompts::request(PromptContext {
            profile: context.agent,
            spec: context.spec,
            skill_id: context.skill_id,
            tools: &manifests,
            previous_answers: context.previous_answers,
            previous_tool_results: context.previous_tool_results,
            results: &tool_results,
            step,
        });
        let response = context.client.complete(&request)?;
        if let Some(record) = usage_record_from_response(context.agent, &response) {
            usage_records.push(record);
        }
        let turn = parse_response(&response)?;
        record_message(sink, context.agent, turn.message.as_deref())?;
        if let Some(waiting) = run_tools_until_wait(
            sink,
            context.agent,
            context.registry,
            ToolBatch {
                step,
                calls: turn.tool_calls,
                final_answer: turn.final_answer.clone(),
                results: &mut tool_results,
                usage_records: &usage_records,
            },
        )? {
            return Ok(AgentOutcome::Waiting(waiting));
        }
        if let Some(answer) = turn.final_answer {
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

fn run_tools_until_wait(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    registry: &ToolRegistry,
    batch: ToolBatch<'_>,
) -> Result<Option<PendingApproval>, AgentRunError> {
    let ToolBatch {
        step,
        calls,
        final_answer,
        results,
        usage_records,
    } = batch;

    for index in 0..calls.len() {
        let call = &calls[index];
        match run_tool(sink, agent, registry, call)? {
            ToolRunOutcome::Finished(result) => results.push(result),
            ToolRunOutcome::Waiting(approval_id) => {
                return Ok(Some(PendingApproval {
                    approval_id,
                    step,
                    pending_call: call.clone(),
                    remaining_calls: calls[index + 1..].to_vec(),
                    final_answer,
                    tool_results: results.clone(),
                    usage_records: usage_records.to_vec(),
                }));
            }
        }
    }
    Ok(None)
}

struct ToolBatch<'a> {
    step: usize,
    calls: Vec<ModelToolCall>,
    final_answer: Option<String>,
    results: &'a mut Vec<ToolResult>,
    usage_records: &'a [UsageRecord],
}

enum ToolRunOutcome {
    Finished(ToolResult),
    Waiting(String),
}

fn run_tool(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    registry: &ToolRegistry,
    call: &ModelToolCall,
) -> Result<ToolRunOutcome, AgentRunError> {
    if let Some(approval_id) = request_approval_if_needed(sink, agent, registry, call)? {
        return Ok(ToolRunOutcome::Waiting(approval_id));
    }
    record_tool_request(sink, agent, call, None, None)?;
    sink.push(
        Some(agent.id()),
        EventKind::ToolStarted,
        json!({ "name": call.name }),
    )?;
    let result = match registry.run(&call.name, call.args.clone()) {
        Ok(output) => record_tool_success(sink, agent, call, output.text),
        Err(error) => record_tool_failure(sink, agent, call, error),
    }?;
    Ok(ToolRunOutcome::Finished(result))
}

fn run_decided_tool(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    registry: &ToolRegistry,
    call: &ModelToolCall,
    approval_id: &str,
    decision: ApprovalDecision,
) -> Result<ToolResult, AgentRunError> {
    match decision {
        ApprovalDecision::Approved => run_approved_tool(sink, agent, registry, call, approval_id),
        ApprovalDecision::Denied => record_denied_tool(sink, agent, call, approval_id),
    }
}

fn run_approved_tool(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    registry: &ToolRegistry,
    call: &ModelToolCall,
    approval_id: &str,
) -> Result<ToolResult, AgentRunError> {
    record_approval_event(sink, agent, call, approval_id, EventKind::ToolApproved)?;
    sink.push(
        Some(agent.id()),
        EventKind::ToolStarted,
        json!({ "name": call.name, "approval_id": approval_id }),
    )?;
    let result = match registry.run_approved(&call.name, call.args.clone()) {
        Ok(output) => record_tool_success(sink, agent, call, output.text),
        Err(error) => record_tool_failure(sink, agent, call, error),
    }?;
    record_patch_applied(sink, agent, call, approval_id)?;
    Ok(result)
}

fn record_denied_tool(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    call: &ModelToolCall,
    approval_id: &str,
) -> Result<ToolResult, AgentRunError> {
    record_approval_event(sink, agent, call, approval_id, EventKind::ToolDenied)?;
    let result = ToolResult::failure(agent.id(), call.name.clone(), "denied by user");
    Ok(result)
}

fn request_approval_if_needed(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    registry: &ToolRegistry,
    call: &ModelToolCall,
) -> Result<Option<String>, AgentRunError> {
    let Some(manifest) = registry.approval_required(&call.name)? else {
        return Ok(None);
    };
    let approval = sink.create_approval(
        agent.id(),
        &call.name,
        redact_args(&call.args),
        manifest.risk,
    )?;
    record_tool_request(sink, agent, call, Some(&approval.id), Some(manifest.risk))?;
    record_patch_proposal(sink, agent, call, &approval.id)?;
    Ok(Some(approval.id))
}
