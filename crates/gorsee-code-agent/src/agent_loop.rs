use gorsee_code_core::{AgentProfile, EventKind, MissionSpec};
use gorsee_code_tool_runtime::{ToolRegistry, ToolRuntimeError};
use serde_json::json;

use crate::{
    client::ChatClient,
    events::EventSink,
    prompts::PromptContext,
    protocol::{parse_response, AgentAnswer, ModelToolCall, ToolResult},
    AgentRunError,
};

const MAX_STEPS: usize = 8;

pub(crate) struct AgentOutcome {
    pub(crate) answer: AgentAnswer,
    pub(crate) tool_results: Vec<ToolResult>,
}

pub(crate) struct AgentRunContext<'a, C: ChatClient> {
    pub(crate) spec: &'a MissionSpec,
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
    let mut tool_results = Vec::new();
    for step in 1..=MAX_STEPS {
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
        let turn = parse_response(&context.client.complete(&request)?)?;
        record_message(sink, context.agent, turn.message.as_deref())?;
        run_tools(
            sink,
            context.agent,
            context.registry,
            turn.tool_calls,
            &mut tool_results,
        )?;
        if let Some(answer) = turn.final_answer {
            return Ok(AgentOutcome {
                answer: AgentAnswer::new(context.agent.id(), answer),
                tool_results,
            });
        }
    }
    Err(AgentRunError::InvalidModelResponse(format!(
        "{} did not finish within step limit",
        context.agent.id()
    )))
}

fn start_agent(sink: &mut EventSink<'_>, agent: &AgentProfile) -> Result<(), AgentRunError> {
    sink.push(
        Some(agent.id()),
        EventKind::AgentStarted,
        json!({ "model": agent.model, "reasoning": agent.reasoning }),
    )?;
    Ok(())
}

fn run_tools(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    registry: &ToolRegistry,
    calls: Vec<ModelToolCall>,
    results: &mut Vec<ToolResult>,
) -> Result<(), AgentRunError> {
    for call in calls {
        let result = run_tool(sink, agent, registry, &call)?;
        results.push(result);
    }
    Ok(())
}

fn run_tool(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    registry: &ToolRegistry,
    call: &ModelToolCall,
) -> Result<ToolResult, AgentRunError> {
    sink.push(
        Some(agent.id()),
        EventKind::ToolRequested,
        json!({ "name": call.name, "args": call.args }),
    )?;
    sink.push(
        Some(agent.id()),
        EventKind::ToolStarted,
        json!({ "name": call.name }),
    )?;
    match registry.run(&call.name, call.args.clone()) {
        Ok(output) => record_tool_success(sink, agent, call, output.text),
        Err(error) => record_tool_failure(sink, agent, call, error),
    }
}

fn record_tool_success(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    call: &ModelToolCall,
    text: String,
) -> Result<ToolResult, AgentRunError> {
    sink.push(
        Some(agent.id()),
        EventKind::ToolFinished,
        json!({ "name": call.name, "output": text }),
    )?;
    Ok(ToolResult::success(agent.id(), call.name.clone(), text))
}

fn record_tool_failure(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    call: &ModelToolCall,
    error: ToolRuntimeError,
) -> Result<ToolResult, AgentRunError> {
    let text = error.to_string();
    sink.push(
        Some(agent.id()),
        EventKind::Error,
        json!({ "name": call.name, "error": text }),
    )?;
    Ok(ToolResult::failure(agent.id(), call.name.clone(), text))
}

fn record_message(
    sink: &mut EventSink<'_>,
    agent: &AgentProfile,
    message: Option<&str>,
) -> Result<(), AgentRunError> {
    if let Some(message) = message.filter(|message| !message.trim().is_empty()) {
        sink.push(
            Some(agent.id()),
            EventKind::AgentMessage,
            json!({ "message": message }),
        )?;
    }
    Ok(())
}
