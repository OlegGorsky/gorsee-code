use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use gorsee_code_coding_core::{ExecutionContract, TurnPlan};
use gorsee_code_core::{AgentProfile, TaskSpec};
use gorsee_code_session::{SessionStore, SessionStoreError};
use gorsee_code_usage::UsageRecord;
use serde::{Deserialize, Serialize};

use crate::{
    agent_loop::PendingApproval,
    protocol::{AgentAnswer, ModelToolCall, ToolResult},
    AgentRunError,
};

const EXECUTION_FILE: &str = "execution.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PendingExecution {
    pub(crate) session_id: String,
    pub(crate) spec: TaskSpec,
    pub(crate) skill_id: Option<String>,
    pub(crate) agents: Vec<AgentProfile>,
    pub(crate) agent_index: usize,
    pub(crate) approval_id: String,
    pub(crate) step: usize,
    pub(crate) pending_call: ModelToolCall,
    pub(crate) remaining_calls: Vec<ModelToolCall>,
    pub(crate) final_answer: Option<String>,
    pub(crate) answers: Vec<AgentAnswer>,
    pub(crate) global_tool_results: Vec<ToolResult>,
    pub(crate) local_tool_results: Vec<ToolResult>,
    pub(crate) global_usage_records: Vec<UsageRecord>,
    pub(crate) local_usage_records: Vec<UsageRecord>,
}

pub(crate) struct ExecutionOutput {
    pub(crate) answers: Vec<AgentAnswer>,
    pub(crate) tool_results: Vec<ToolResult>,
    pub(crate) usage_records: Vec<UsageRecord>,
    pub(crate) turn_plan: Option<TurnPlan>,
    pub(crate) execution_contract: ExecutionContract,
}

impl PendingExecution {
    pub(crate) fn from_parts(parts: PendingExecutionParts<'_>) -> Self {
        let PendingExecutionParts {
            session_id,
            spec,
            skill_id,
            agents,
            agent_index,
            answers,
            global_tool_results,
            global_usage_records,
            pending,
        } = parts;

        Self {
            session_id,
            spec: spec.clone(),
            skill_id: skill_id.map(str::to_string),
            agents: agents.to_vec(),
            agent_index,
            approval_id: pending.approval_id,
            step: pending.step,
            pending_call: pending.pending_call,
            remaining_calls: pending.remaining_calls,
            final_answer: pending.final_answer,
            answers: answers.to_vec(),
            global_tool_results: global_tool_results.to_vec(),
            local_tool_results: pending.tool_results,
            global_usage_records: global_usage_records.to_vec(),
            local_usage_records: pending.usage_records,
        }
    }

    pub(crate) fn pending_approval(self) -> PendingApproval {
        PendingApproval {
            approval_id: self.approval_id,
            step: self.step,
            pending_call: self.pending_call,
            remaining_calls: self.remaining_calls,
            final_answer: self.final_answer,
            tool_results: self.local_tool_results,
            usage_records: self.local_usage_records,
        }
    }
}

pub(crate) struct PendingExecutionParts<'a> {
    pub(crate) session_id: String,
    pub(crate) spec: &'a TaskSpec,
    pub(crate) skill_id: Option<&'a str>,
    pub(crate) agents: &'a [AgentProfile],
    pub(crate) agent_index: usize,
    pub(crate) answers: &'a [AgentAnswer],
    pub(crate) global_tool_results: &'a [ToolResult],
    pub(crate) global_usage_records: &'a [UsageRecord],
    pub(crate) pending: PendingApproval,
}

pub(crate) fn save_pending_execution(
    store: &SessionStore,
    pending: &PendingExecution,
) -> Result<(), AgentRunError> {
    let path = store.session_dir(&pending.session_id).join(EXECUTION_FILE);
    let encoded = serde_json::to_string_pretty(pending).map_err(SessionStoreError::from)?;
    write_atomic(&path, encoded).map_err(SessionStoreError::from)?;
    Ok(())
}

pub(crate) fn load_pending_execution(
    store: &SessionStore,
    session_id: &str,
) -> Result<PendingExecution, AgentRunError> {
    let path = store.session_dir(session_id).join(EXECUTION_FILE);
    let text = fs::read_to_string(path).map_err(SessionStoreError::from)?;
    serde_json::from_str(&text)
        .map_err(SessionStoreError::from)
        .map_err(AgentRunError::from)
}

pub(crate) fn clear_pending_execution(
    store: &SessionStore,
    session_id: &str,
) -> Result<(), AgentRunError> {
    let path = store.session_dir(session_id).join(EXECUTION_FILE);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SessionStoreError::from(error).into()),
    }
}

fn write_atomic(path: &Path, content: impl AsRef<str>) -> std::io::Result<()> {
    let temp = temp_path(path);
    if let Err(error) = fs::write(&temp, content.as_ref()) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    Ok(())
}

fn temp_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    parent.join(format!(".{name}.tmp-{}-{now}", std::process::id()))
}
