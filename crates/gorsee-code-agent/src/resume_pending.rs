use crate::{
    agent_loop::PendingApproval,
    execution::{save_pending_execution, PendingExecution, PendingExecutionParts},
    resume_types::PendingSaveInput,
    AgentRunError,
};

pub(crate) fn save_waiting(
    input: PendingSaveInput<'_>,
    waiting: PendingApproval,
) -> Result<String, AgentRunError> {
    let PendingSaveInput {
        store,
        session_id,
        spec,
        skill_id,
        agents,
        agent_index,
        answers,
        tool_results,
        usage_records,
    } = input;

    let approval_id = waiting.approval_id.clone();
    let snapshot = PendingExecution::from_parts(PendingExecutionParts {
        session_id: session_id.to_string(),
        spec,
        skill_id,
        agents,
        agent_index,
        answers,
        global_tool_results: tool_results,
        global_usage_records: usage_records,
        pending: waiting,
    });
    save_pending_execution(store, &snapshot)?;
    Ok(approval_id)
}

pub(crate) fn validate_pending(
    pending: &PendingExecution,
    session_id: &str,
    approval_id: &str,
) -> Result<(), AgentRunError> {
    if pending.session_id != session_id || pending.approval_id != approval_id {
        return Err(AgentRunError::Runtime(format!(
            "approval {approval_id} does not match pending execution"
        )));
    }
    Ok(())
}
