use gorsee_code_core::{AgentProfile, TaskSpec};
use gorsee_code_session::SessionStore;
use gorsee_code_tool_runtime::ToolRegistry;
use gorsee_code_usage::UsageRecord;

use crate::{
    agent_loop::PendingApproval,
    client::ChatClient,
    events::EventSink,
    protocol::{AgentAnswer, ToolResult},
};

pub(crate) type FlowSuccess = (
    TaskSpec,
    Option<String>,
    Vec<AgentProfile>,
    Vec<AgentAnswer>,
    Vec<ToolResult>,
    Vec<UsageRecord>,
);

pub(crate) struct AgentResumeInput<'a, 'sink, C: ChatClient> {
    pub(crate) client: &'a C,
    pub(crate) registry: &'a ToolRegistry,
    pub(crate) sink: &'a mut EventSink<'sink>,
    pub(crate) spec: &'a TaskSpec,
    pub(crate) skill_id: Option<&'a str>,
    pub(crate) agent: &'a AgentProfile,
}

pub(crate) struct RemainingAgentsInput<'a, 'sink, C: ChatClient> {
    pub(crate) store: &'a SessionStore,
    pub(crate) session_id: &'a str,
    pub(crate) spec: &'a TaskSpec,
    pub(crate) skill_id: Option<&'a str>,
    pub(crate) client: &'a C,
    pub(crate) registry: &'a ToolRegistry,
    pub(crate) sink: &'a mut EventSink<'sink>,
    pub(crate) agents: &'a [AgentProfile],
    pub(crate) first_index: usize,
}

pub(crate) struct PendingSaveInput<'a> {
    pub(crate) store: &'a SessionStore,
    pub(crate) session_id: &'a str,
    pub(crate) spec: &'a TaskSpec,
    pub(crate) skill_id: Option<&'a str>,
    pub(crate) agents: &'a [AgentProfile],
    pub(crate) agent_index: usize,
    pub(crate) answers: &'a [AgentAnswer],
    pub(crate) tool_results: &'a [ToolResult],
    pub(crate) usage_records: &'a [UsageRecord],
}

pub(crate) enum ResumeState {
    Finished,
    Waiting(PendingApproval),
}
