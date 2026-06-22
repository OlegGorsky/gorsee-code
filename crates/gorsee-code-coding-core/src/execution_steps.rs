use gorsee_code_core::{Event, EventKind};

use crate::{lcp::LcpExecutionStep, orchestrator::OrchestrationPlan, turn::ExecutionStepState};

pub(crate) fn execution_steps(
    orchestration: &OrchestrationPlan,
    events: &[Event],
) -> Vec<LcpExecutionStep> {
    let mut steps = orchestration
        .plan
        .as_ref()
        .map(|plan| {
            plan.steps
                .iter()
                .map(|step| LcpExecutionStep {
                    id: step.id.clone(),
                    state: ExecutionStepState::Pending,
                    risk: step.risk,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for event in events {
        apply_event_to_steps(&mut steps, event);
    }
    steps
}

fn apply_event_to_steps(steps: &mut [LcpExecutionStep], event: &Event) {
    match event.kind {
        EventKind::ToolRequested => {
            let state = if has_approval_id(event) {
                ExecutionStepState::WaitingApproval
            } else {
                ExecutionStepState::Running
            };
            mark_tool_step(steps, event, state);
        }
        EventKind::ToolApproved | EventKind::ToolStarted => {
            mark_tool_step(steps, event, ExecutionStepState::Running);
        }
        EventKind::ToolFinished => {
            mark_tool_step(steps, event, terminal_state(event));
        }
        EventKind::PatchProposed => {
            mark_named_step(steps, "apply_changes", ExecutionStepState::WaitingApproval)
        }
        EventKind::PatchApplied => {
            mark_named_step(steps, "apply_changes", ExecutionStepState::Succeeded);
        }
        EventKind::DiffReady => {
            mark_first_existing(
                steps,
                &["review_diff", "read_diff", "review_state"],
                terminal_state(event),
            );
        }
        EventKind::TestStarted => mark_first_existing(
            steps,
            &["verify", "run_checks"],
            ExecutionStepState::Running,
        ),
        EventKind::TestFinished => {
            mark_first_existing(steps, &["verify", "run_checks"], terminal_state(event));
        }
        EventKind::ToolDenied | EventKind::Error => {
            mark_tool_step(steps, event, ExecutionStepState::Failed);
        }
        EventKind::TurnFinished => finish_running_steps(steps),
        _ => {}
    }
}

fn mark_tool_step(steps: &mut [LcpExecutionStep], event: &Event, state: ExecutionStepState) {
    if let Some(tool) = tool_name(event) {
        if mark_by_expected_tool(steps, &tool, state) {
            return;
        }
        if let Some(step_id) = fallback_step_for_tool(&tool) {
            mark_named_step(steps, step_id, state);
        }
    }
}

fn mark_by_expected_tool(
    steps: &mut [LcpExecutionStep],
    tool: &str,
    state: ExecutionStepState,
) -> bool {
    let Some(step) = steps
        .iter_mut()
        .find(|step| expected_tool_matches(&step.id, tool))
    else {
        return false;
    };
    update_state(step, state);
    true
}

fn expected_tool_matches(step_id: &str, tool: &str) -> bool {
    match step_id {
        "inspect_repo" | "search_context" => {
            matches!(
                tool,
                "repo_map" | "list_files" | "read_file" | "search_text"
            )
        }
        "apply_changes" => matches!(tool, "propose_patch" | "apply_patch"),
        "review_diff" | "read_diff" | "review_state" => {
            matches!(tool, "git_diff" | "git_changed_files" | "git_status")
        }
        "verify" | "run_checks" => tool == "run_test",
        "release_commands" => tool == "run_command",
        _ => false,
    }
}

fn fallback_step_for_tool(tool: &str) -> Option<&'static str> {
    match tool {
        "repo_map" | "list_files" | "read_file" | "search_text" => Some("inspect_repo"),
        "propose_patch" | "apply_patch" => Some("apply_changes"),
        "git_diff" | "git_changed_files" | "git_status" => Some("review_diff"),
        "run_test" => Some("verify"),
        "run_command" => Some("release_commands"),
        _ => None,
    }
}

fn mark_first_existing(steps: &mut [LcpExecutionStep], ids: &[&str], state: ExecutionStepState) {
    if let Some(step) = steps
        .iter_mut()
        .find(|step| ids.iter().any(|id| step.id == *id))
    {
        update_state(step, state);
    }
}

fn mark_named_step(steps: &mut [LcpExecutionStep], id: &str, state: ExecutionStepState) {
    if let Some(step) = steps.iter_mut().find(|step| step.id == id) {
        update_state(step, state);
    }
}

fn update_state(step: &mut LcpExecutionStep, state: ExecutionStepState) {
    if step.state == ExecutionStepState::Succeeded
        && matches!(
            state,
            ExecutionStepState::Pending
                | ExecutionStepState::Running
                | ExecutionStepState::WaitingApproval
        )
    {
        return;
    }
    step.state = state;
}

fn finish_running_steps(steps: &mut [LcpExecutionStep]) {
    for step in steps {
        if step.state == ExecutionStepState::Running {
            step.state = ExecutionStepState::Succeeded;
        }
    }
}

fn tool_name(event: &Event) -> Option<String> {
    event
        .payload
        .get("name")
        .or_else(|| event.payload.get("tool"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn has_approval_id(event: &Event) -> bool {
    event
        .payload
        .get("approval_id")
        .is_some_and(|value| !value.is_null())
}

fn terminal_state(event: &Event) -> ExecutionStepState {
    match event
        .payload
        .get("status")
        .and_then(serde_json::Value::as_str)
    {
        Some("failed" | "error" | "unavailable") => ExecutionStepState::Failed,
        Some("skipped") => ExecutionStepState::Skipped,
        _ => ExecutionStepState::Succeeded,
    }
}

#[cfg(test)]
mod tests {
    use gorsee_code_core::{default_agent_matrix, Event};
    use serde_json::json;

    use super::*;
    use crate::{LocalCodingProtocol, TurnRequest, WorkspaceRef};

    #[test]
    fn failed_tool_finished_marks_matching_step_failed() {
        let protocol = LocalCodingProtocol::default();
        let orchestration = protocol
            .plan_turn(request("проверь текущий diff"), default_agent_matrix())
            .orchestration;
        let steps = execution_steps(
            &orchestration,
            &[event(
                EventKind::ToolFinished,
                json!({"name":"git_diff","status":"failed"}),
            )],
        );

        assert_eq!(state(&steps, "read_diff"), ExecutionStepState::Failed);
    }

    fn request(message: &str) -> TurnRequest {
        TurnRequest {
            workspace: WorkspaceRef {
                root: "/repo".into(),
                branch: None,
                session_id: None,
            },
            message: message.into(),
            user_id: None,
        }
    }

    fn event(kind: EventKind, payload: serde_json::Value) -> Event {
        Event::new(1, "s1", Some("validator".into()), kind, payload)
    }

    fn state(steps: &[LcpExecutionStep], id: &str) -> ExecutionStepState {
        steps
            .iter()
            .find(|step| step.id == id)
            .unwrap_or_else(|| panic!("missing step {id}"))
            .state
    }
}
