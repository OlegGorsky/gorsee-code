use gorsee_code_core::AgentProfile;
use gorsee_code_safety::PathPolicy;

use crate::protocol::{ModelToolCall, ToolResult};

pub(crate) fn read_before_write_feedback(
    agent: &AgentProfile,
    repo_path: &str,
    previous_results: &[ToolResult],
    local_results: &[ToolResult],
    call: &ModelToolCall,
) -> Option<ToolResult> {
    if !matches!(call.name.as_str(), "apply_patch" | "propose_patch") {
        return None;
    }
    let path = call.args.get("path")?.as_str()?;
    if !existing_workspace_file(repo_path, path) || prior_file_context(path, previous_results) {
        return None;
    }
    if prior_file_context(path, local_results) {
        return None;
    }
    Some(ToolResult::failure(
        agent.id(),
        "execution_policy",
        format!(
            "read_file or search_text required before {} on existing file: {}",
            call.name, path
        ),
    ))
}

fn existing_workspace_file(repo_path: &str, path: &str) -> bool {
    let Ok(policy) = PathPolicy::new(repo_path) else {
        return false;
    };
    policy
        .resolve_existing(path)
        .is_ok_and(|target| target.is_file())
}

fn prior_file_context(path: &str, results: &[ToolResult]) -> bool {
    results.iter().any(|result| {
        result.ok
            && (result.name == "search_text"
                || (result.name == "read_file" && read_result_matches(path, result)))
    })
}

fn read_result_matches(path: &str, result: &ToolResult) -> bool {
    let Some(read_path) = result
        .json
        .as_ref()
        .and_then(|payload| payload.get("path"))
        .and_then(serde_json::Value::as_str)
    else {
        return true;
    };
    normalize_tool_path(read_path) == normalize_tool_path(path)
}

fn normalize_tool_path(path: &str) -> String {
    path.trim_start_matches("./").replace('\\', "/")
}
