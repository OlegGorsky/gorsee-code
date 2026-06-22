use std::path::Path;

use gorsee_code_core::{AgentProfile, EventKind};
use gorsee_code_session::ApprovalDecision;
use gorsee_code_tool_runtime::{ToolManifest, ToolRegistry, ToolRuntimeError};
use gorsee_code_tools::test_runner::{
    default_test_command, test_command_allowed, test_command_from_args,
    test_command_supported_in_workspace,
};
use gorsee_code_usage::UsageRecord;
use serde_json::{json, Value};

use crate::{
    agent_loop::PendingApproval,
    agent_tool_policy::read_before_write_feedback,
    events::EventSink,
    protocol::{ModelToolCall, ToolResult},
    tool_events::{
        record_approval_event, record_patch_applied, record_patch_proposal, record_tool_failure,
        record_tool_request, record_tool_success, redact_args,
    },
    AgentRunError,
};

pub(crate) struct ToolBatch<'a> {
    pub(crate) repo_path: &'a str,
    pub(crate) step: usize,
    pub(crate) calls: Vec<ModelToolCall>,
    pub(crate) final_answer: Option<String>,
    pub(crate) previous_results: &'a [ToolResult],
    pub(crate) results: &'a mut Vec<ToolResult>,
    pub(crate) usage_records: &'a [UsageRecord],
}

struct ToolHistory<'a> {
    previous: &'a [ToolResult],
    local: &'a [ToolResult],
}

enum ToolRunOutcome {
    Finished(ToolResult),
    Waiting(String),
}

pub(crate) fn allowed_manifests(
    agent: &AgentProfile,
    registry: &ToolRegistry,
) -> Vec<ToolManifest> {
    if agent.tools.is_empty() {
        return Vec::new();
    }
    registry
        .manifests()
        .into_iter()
        .filter(|manifest| agent.tools.iter().any(|tool| tool_matches(tool, manifest)))
        .collect()
}

pub(crate) fn run_tools_until_wait(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    registry: &ToolRegistry,
    allowed_manifests: &[ToolManifest],
    batch: ToolBatch<'_>,
) -> Result<Option<PendingApproval>, AgentRunError> {
    let ToolBatch {
        repo_path,
        step,
        calls,
        final_answer,
        previous_results,
        results,
        usage_records,
    } = batch;

    for index in 0..calls.len() {
        let call = &calls[index];
        match run_tool(
            sink,
            agent,
            registry,
            allowed_manifests,
            repo_path,
            ToolHistory {
                previous: previous_results,
                local: results,
            },
            call,
        )? {
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

pub(crate) fn run_decided_tool(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    registry: &ToolRegistry,
    allowed_manifests: &[ToolManifest],
    call: &ModelToolCall,
    approval_id: &str,
    decision: ApprovalDecision,
) -> Result<ToolResult, AgentRunError> {
    match decision {
        ApprovalDecision::Approved => {
            run_approved_tool(sink, agent, registry, allowed_manifests, call, approval_id)
        }
        ApprovalDecision::Denied => record_denied_tool(sink, agent, call, approval_id),
    }
}

fn tool_matches(tool: &str, manifest: &ToolManifest) -> bool {
    manifest.name == tool
        || manifest.capabilities.iter().any(|capability| {
            capability == tool || grouped_tool_matches(tool, &manifest.name, capability)
        })
}

fn grouped_tool_matches(tool: &str, name: &str, capability: &str) -> bool {
    match tool {
        "read" => matches!(
            capability,
            "files:list" | "files:read" | "git:status" | "git:recent"
        ),
        "search" => capability == "files:search",
        "repo_map" => capability == "context:repo_map",
        "diff" => matches!(capability, "git:diff" | "git:changed_files"),
        "propose_patch" => matches!(name, "propose_patch" | "apply_patch"),
        "run_test" => capability == "tests:run",
        "mcp" => matches!(capability, "mcp:list" | "mcp:call"),
        _ => false,
    }
}

fn run_tool(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    registry: &ToolRegistry,
    allowed_manifests: &[ToolManifest],
    repo_path: &str,
    history: ToolHistory<'_>,
    call: &ModelToolCall,
) -> Result<ToolRunOutcome, AgentRunError> {
    if !tool_allowed(call, allowed_manifests) {
        let result = record_disallowed_tool(sink, agent, call)?;
        return Ok(ToolRunOutcome::Finished(result));
    }
    if let Some(result) =
        read_before_write_feedback(agent, repo_path, history.previous, history.local, call)
    {
        return Ok(ToolRunOutcome::Finished(result));
    }
    if let Some(result) = run_skipped_test_without_approval(sink, agent, registry, repo_path, call)?
    {
        return Ok(ToolRunOutcome::Finished(result));
    }
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
        Ok(output) => record_tool_success(sink, agent, call, output),
        Err(error) => record_tool_failure(sink, agent, call, error),
    }?;
    Ok(ToolRunOutcome::Finished(result))
}

fn run_skipped_test_without_approval(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    registry: &ToolRegistry,
    repo_path: &str,
    call: &ModelToolCall,
) -> Result<Option<ToolResult>, AgentRunError> {
    if call.name != "run_test" {
        return Ok(None);
    }
    if should_request_test_approval(repo_path, &call.args) {
        return Ok(None);
    }
    record_tool_request(sink, agent, call, None, None)?;
    sink.push(
        Some(agent.id()),
        EventKind::ToolStarted,
        json!({ "name": call.name }),
    )?;
    let result = match registry.run_approved(&call.name, call.args.clone()) {
        Ok(output) => record_tool_success(sink, agent, call, output),
        Err(error) => record_tool_failure(sink, agent, call, error),
    }?;
    Ok(Some(result))
}

fn should_request_test_approval(repo_path: &str, args: &Value) -> bool {
    let root = Path::new(repo_path);
    let Some(command) = test_command_from_args(args) else {
        return !default_test_command(root).is_empty();
    };
    !command.is_empty()
        && test_command_allowed(&command)
        && test_command_supported_in_workspace(root, &command)
}

fn run_approved_tool(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    registry: &ToolRegistry,
    allowed_manifests: &[ToolManifest],
    call: &ModelToolCall,
    approval_id: &str,
) -> Result<ToolResult, AgentRunError> {
    if !tool_allowed(call, allowed_manifests) {
        return record_disallowed_tool(sink, agent, call);
    }
    record_approval_event(sink, agent, call, approval_id, EventKind::ToolApproved)?;
    sink.push(
        Some(agent.id()),
        EventKind::ToolStarted,
        json!({ "name": call.name, "approval_id": approval_id }),
    )?;
    let result = match registry.run_approved(&call.name, call.args.clone()) {
        Ok(output) => record_tool_success(sink, agent, call, output),
        Err(error) => record_tool_failure(sink, agent, call, error),
    }?;
    if result.ok {
        record_patch_applied(sink, agent, call, approval_id)?;
    }
    Ok(result)
}

fn record_denied_tool(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    call: &ModelToolCall,
    approval_id: &str,
) -> Result<ToolResult, AgentRunError> {
    record_approval_event(sink, agent, call, approval_id, EventKind::ToolDenied)?;
    Ok(ToolResult::failure(
        agent.id(),
        call.name.clone(),
        "denied by user",
    ))
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

fn tool_allowed(call: &ModelToolCall, allowed_manifests: &[ToolManifest]) -> bool {
    allowed_manifests
        .iter()
        .any(|manifest| manifest.name == call.name)
}

fn record_disallowed_tool(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    call: &ModelToolCall,
) -> Result<ToolResult, AgentRunError> {
    record_tool_failure(
        sink,
        agent,
        call,
        ToolRuntimeError::PermissionDenied(format!("tool not allowed for agent: {}", call.name)),
    )
}
