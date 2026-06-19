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
    ChatRequest {
        model: context.profile.model.clone(),
        stream: false,
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: system_prompt(),
            },
            ChatMessage::user(user_prompt(&context)),
        ],
    }
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

fn user_prompt(context: &PromptContext<'_>) -> String {
    json!({
        "agent": {
            "id": context.profile.id(),
            "model": context.profile.model,
            "reasoning": context.profile.reasoning,
            "tools": context.profile.tools,
            "budget_tokens": context.profile.budget_tokens
        },
        "objective": context.spec.objective,
        "repo_path": context.spec.repo_path,
        "skill": context.skill_id,
        "budget_tokens": context.spec.budget_tokens,
        "step": context.step,
        "available_tools": context.tools,
        "previous_agents": context.previous_answers,
        "previous_tool_results": context.previous_tool_results,
        "tool_results": context.results,
    })
    .to_string()
}
