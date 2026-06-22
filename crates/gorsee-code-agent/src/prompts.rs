use gorsee_code_coding_core::{ExecutionContract, TurnPlan};
use gorsee_code_core::{AgentProfile, TaskSpec};
use gorsee_code_neurogate::{ChatMessage, ChatRequest};
use gorsee_code_tool_runtime::ToolManifest;
use serde_json::json;

use crate::protocol::{AgentAnswer, ToolResult};

pub(crate) struct PromptContext<'a> {
    pub(crate) profile: &'a AgentProfile,
    pub(crate) spec: &'a TaskSpec,
    pub(crate) skill_id: Option<&'a str>,
    pub(crate) tools: &'a [ToolManifest],
    pub(crate) previous_answers: &'a [AgentAnswer],
    pub(crate) previous_tool_results: &'a [ToolResult],
    pub(crate) results: &'a [ToolResult],
    pub(crate) turn_plan: Option<&'a TurnPlan>,
    pub(crate) execution_contract: &'a ExecutionContract,
    pub(crate) step: usize,
}

pub(crate) fn request(context: PromptContext<'_>) -> ChatRequest {
    if lightweight_chat(&context) {
        return lightweight_chat_request(&context);
    }

    ChatRequest::new(
        context.profile.model.clone(),
        vec![
            ChatMessage {
                role: "system".into(),
                content: system_prompt(),
            },
            ChatMessage::user(static_context_prompt(&context)),
            ChatMessage::user(work_prompt(&context)),
        ],
    )
    .with_prompt_cache_key(prompt_cache_key(&context, "agent"))
    .with_prompt_cache_retention("24h")
}

fn lightweight_chat(context: &PromptContext<'_>) -> bool {
    context.profile.tools.is_empty()
        && context.tools.is_empty()
        && context.skill_id.is_none()
        && context.previous_answers.is_empty()
        && context.previous_tool_results.is_empty()
        && context.results.is_empty()
}

fn lightweight_chat_request(context: &PromptContext<'_>) -> ChatRequest {
    ChatRequest::new(
        context.profile.model.clone(),
        vec![
            ChatMessage {
                role: "system".into(),
                content: lightweight_system_prompt(),
            },
            ChatMessage::user(context.spec.objective.clone()),
        ],
    )
    .with_prompt_cache_key(prompt_cache_key(context, "chat"))
    .with_prompt_cache_retention("24h")
}

fn lightweight_system_prompt() -> String {
    [
        "You are Gorsee Code in lightweight chat mode.",
        "Answer briefly and naturally in Russian unless the user uses another language.",
        "Return only JSON: {\"final_answer\":\"...\"}.",
    ]
    .join(" ")
}

fn system_prompt() -> String {
    [
        "You are Gorsee Code, an autonomous production coding agent.",
        "Use workspace tools to inspect and change the repository.",
        "Respond only with JSON: {\"message\":\"...\",",
        "\"tool_calls\":[{\"name\":\"tool\",\"args\":{}}],",
        "\"final_answer\":\"...\"}.",
        "Call tools when you need repository facts. Provide final_answer only when done.",
        "For repository-editing work, do not dump full source code in final_answer as the implementation.",
        "Use file tools for changes, inspect diff, run suitable checks, then summarize files and verification.",
        "Architect/Scout roles hand off findings and plans; they must not claim an edit is impossible just because their own tool set is read-only.",
        "Only Validator/Summarizer should close coding turns for the user; earlier roles provide internal handoff context.",
        "run_test is only for project test commands: cargo, npm, pnpm, yarn, pytest, or go.",
        "Do not call run_test with cat, ls, file, python, shell snippets, or ad-hoc file checks; use read_file/list_files for file existence and content verification.",
        "If run_test reports skipped or unsupported command, do not retry with another ad-hoc shell command; summarize the skipped verification and any read_file/list_files evidence.",
    ]
    .join(" ")
}

fn static_context_prompt(context: &PromptContext<'_>) -> String {
    json!({
        "agent": {
            "id": context.profile.id(),
            "model": context.profile.model,
            "reasoning": context.profile.reasoning,
            "tools": context.profile.tools,
            "budget_tokens": context.profile.budget_tokens
        },
        "repo_path": context.spec.repo_path,
        "skill": context.skill_id,
        "budget_tokens": context.spec.budget_tokens,
        "available_tools": context.tools,
        "turn_plan": context.turn_plan,
        "execution_contract": context.execution_contract,
        "execution_policy": execution_policy(context),
    })
    .to_string()
}

fn work_prompt(context: &PromptContext<'_>) -> String {
    json!({
        "objective": context.spec.objective,
        "step": context.step,
        "previous_agents": context.previous_answers,
        "previous_tool_results": context.previous_tool_results,
        "tool_results": context.results,
    })
    .to_string()
}

fn execution_policy(context: &PromptContext<'_>) -> serde_json::Value {
    let can_write = context
        .tools
        .iter()
        .any(|tool| tool.name == "apply_patch" || tool.name == "propose_patch");
    json!({
        "write_tools_available": can_write,
        "must_use_tools_for_file_changes": can_write,
        "final_answer": {
            "chat_only": "answer naturally",
            "coding_task": "brief summary, changed files, diff/check status; no full code dump as substitute for edits"
        }
    })
}

fn prompt_cache_key(context: &PromptContext<'_>, mode: &str) -> String {
    let material = json!({
        "agent_id": context.profile.id(),
        "model": context.profile.model,
        "reasoning": context.profile.reasoning,
        "repo_path": context.spec.repo_path,
        "skill": context.skill_id,
        "tools": context.profile.tools,
        "available_tools": context.tools,
        "mode": mode,
    })
    .to_string();
    format!("gorsee:{mode}:v1:{}", short_hash(&material))
}

fn short_hash(input: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
