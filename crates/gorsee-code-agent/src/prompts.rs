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
