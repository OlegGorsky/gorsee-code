use gorsee_code_coding_core::{route_intent, CodingIntent, ExecutionContract};
use gorsee_code_core::{AgentProfile, AgentRole};
use gorsee_code_tool_runtime::ToolManifest;

use crate::{final_answer_content::final_answer_contract_feedback, protocol::ToolResult};

pub(crate) fn final_answer_policy_retry(
    agent: &AgentProfile,
    objective: &str,
    manifests: &[ToolManifest],
    previous_results: &[ToolResult],
    tool_results: &[ToolResult],
    answer: &str,
    contract: &ExecutionContract,
) -> Option<ToolResult> {
    if must_retry_for_missing_patch(agent, objective, manifests, tool_results, answer) {
        return Some(policy_feedback(
            agent,
            "coding task must change files with apply_patch before final answer; inspect diff, verify, then summarize",
        ));
    }
    if closes_turn_with_contract(agent) {
        if let Some(feedback) =
            missing_contract_gate(agent, manifests, previous_results, tool_results, contract)
        {
            return Some(feedback);
        }
        return final_answer_contract_feedback(answer, &contract.final_answer)
            .map(|feedback| policy_feedback(agent, &feedback));
    }
    None
}

fn must_retry_for_missing_patch(
    agent: &AgentProfile,
    objective: &str,
    manifests: &[ToolManifest],
    tool_results: &[ToolResult],
    answer: &str,
) -> bool {
    if !must_apply_patch_before_final(objective, manifests)
        || has_result(tool_results, &["apply_patch"])
    {
        return false;
    }
    agent.role == AgentRole::Coder || looks_like_code_dump(answer)
}

fn missing_contract_gate(
    agent: &AgentProfile,
    manifests: &[ToolManifest],
    previous_results: &[ToolResult],
    tool_results: &[ToolResult],
    contract: &ExecutionContract,
) -> Option<ToolResult> {
    if contract.diff_required
        && has_manifest(manifests, &["git_diff", "git_changed_files"])
        && !has_result_pair(
            previous_results,
            tool_results,
            &["git_diff", "git_changed_files"],
        )
    {
        return Some(policy_feedback(
            agent,
            "final answer requires inspecting structured git diff with git_diff or git_changed_files",
        ));
    }
    if contract.verification_required
        && has_manifest(manifests, &["run_test"])
        && !verification_gate_satisfied(previous_results, tool_results)
    {
        return Some(policy_feedback(
            agent,
            "final answer requires running verification with run_test or reporting why it was skipped",
        ));
    }
    None
}

fn policy_feedback(agent: &AgentProfile, text: &str) -> ToolResult {
    ToolResult::failure(agent.id(), "execution_policy", text)
}

fn closes_turn_with_contract(agent: &AgentProfile) -> bool {
    matches!(agent.role, AgentRole::Validator | AgentRole::Summarizer)
}

fn must_apply_patch_before_final(objective: &str, manifests: &[ToolManifest]) -> bool {
    route_intent(objective).intent == CodingIntent::Edit
        && manifests
            .iter()
            .any(|manifest| manifest.name == "apply_patch")
}

fn has_manifest(manifests: &[ToolManifest], names: &[&str]) -> bool {
    manifests
        .iter()
        .any(|manifest| names.iter().any(|name| manifest.name == *name))
}

fn has_result_pair(left: &[ToolResult], right: &[ToolResult], names: &[&str]) -> bool {
    has_result(left, names) || has_result(right, names)
}

fn verification_gate_satisfied(left: &[ToolResult], right: &[ToolResult]) -> bool {
    has_result_pair(left, right, &["run_test"])
        || has_denied_result_pair(left, right, &["run_test"])
}

fn has_denied_result_pair(left: &[ToolResult], right: &[ToolResult], names: &[&str]) -> bool {
    left.iter().chain(right).any(|result| {
        !result.ok
            && names.iter().any(|name| result.name == *name)
            && result.text == "denied by user"
    })
}

fn has_result(results: &[ToolResult], names: &[&str]) -> bool {
    results
        .iter()
        .any(|result| result.ok && names.iter().any(|name| result.name == *name))
}

fn looks_like_code_dump(answer: &str) -> bool {
    let lower = answer.to_lowercase();
    lower.contains("```")
        || lower.contains("fn ")
        || lower.contains("class ")
        || lower.contains("import ")
        || lower.contains("def ")
        || lower.lines().count() > 18
}

#[cfg(test)]
#[path = "final_policy_tests.rs"]
mod final_policy_tests;
