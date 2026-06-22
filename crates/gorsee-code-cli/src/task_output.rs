use gorsee_code_agent::TaskTurnOutput;
use gorsee_code_coding_core::{TranscriptEvent, TranscriptEventKind};

pub fn format_task_output(output: &TaskTurnOutput) -> String {
    let mut out = format!(
        "run: session={}\nstatus={}\nintent={:?}\nevents={}\nagents={}\nartifacts={}\n",
        output.summary.session_id,
        output.response.status,
        output.response.intent,
        output.summary.events,
        output.summary.agents.join(","),
        output.summary.artifacts.len()
    );
    append_transcript(&mut out, &output.response.transcript);
    append_usage(&mut out, output);
    out
}

fn append_transcript(out: &mut String, transcript: &[TranscriptEvent]) {
    for event in transcript {
        let Some(label) = label_for(&event.kind) else {
            continue;
        };
        if event.kind == TranscriptEventKind::ApprovalNeeded {
            append_approval(out, event);
        } else {
            append_labeled_summary(out, label, &event.summary);
        }
    }
}

fn append_usage(out: &mut String, output: &TaskTurnOutput) {
    let usage = output.response.usage;
    out.push_str(&format!(
        "usage={}/{} tokens cached={}\n",
        usage.tokens_used, usage.tokens_limit, usage.cached_tokens
    ));
}

fn append_labeled_summary(out: &mut String, label: &str, summary: &str) {
    let summary = summary.trim();
    if summary.is_empty() {
        return;
    }
    if summary.contains('\n') {
        out.push_str(label);
        out.push_str(":\n");
        for line in summary.lines() {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
    } else {
        out.push_str(label);
        out.push_str(": ");
        out.push_str(summary);
        out.push('\n');
    }
}

fn append_approval(out: &mut String, event: &TranscriptEvent) {
    let summary = event.summary.trim();
    let Some(id) = event
        .detail
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    else {
        append_labeled_summary(out, "approval", summary);
        return;
    };
    out.push_str("approval: ");
    out.push_str(id);
    if !summary.is_empty() {
        out.push(' ');
        out.push_str(summary);
    }
    out.push('\n');
}

fn label_for(kind: &TranscriptEventKind) -> Option<&'static str> {
    match kind {
        TranscriptEventKind::UserMessage => None,
        TranscriptEventKind::AssistantMessage => Some("answer"),
        TranscriptEventKind::Thinking => Some("thinking"),
        TranscriptEventKind::ToolSummary => Some("tool"),
        TranscriptEventKind::DiffReady => Some("diff"),
        TranscriptEventKind::ApprovalNeeded => Some("approval"),
        TranscriptEventKind::VerificationResult => Some("verification"),
        TranscriptEventKind::ErrorSummary => Some("error"),
    }
}

#[cfg(test)]
mod tests {
    use gorsee_code_agent::{TaskRunSummary, TaskTurnOutput};
    use gorsee_code_coding_core::{
        CodingOrchestrator, LcpTurnResponseInput, LcpUsageSnapshot, LocalCodingProtocol,
        TurnRequest, WorkspaceRef,
    };
    use gorsee_code_core::{default_agent_matrix, Event, EventKind};
    use serde_json::json;

    use super::format_task_output;

    #[test]
    fn task_output_includes_clean_answer_without_raw_events() {
        let orchestration = CodingOrchestrator.plan_turn(
            TurnRequest {
                workspace: WorkspaceRef {
                    root: "/tmp/project".into(),
                    branch: None,
                    session_id: None,
                },
                message: "привет".into(),
                user_id: Some("cli".into()),
            },
            default_agent_matrix(),
        );
        let events = vec![
            Event::new(
                1,
                "s1",
                None,
                EventKind::TurnStarted,
                json!({"objective":"привет"}),
            ),
            Event::new(
                2,
                "s1",
                Some("architect".into()),
                EventKind::ToolRequested,
                json!({"name":"read_file"}),
            ),
            Event::new(
                3,
                "s1",
                Some("architect".into()),
                EventKind::AgentMessage,
                json!({"message":"Привет! Чем помочь?"}),
            ),
            Event::new(4, "s1", None, EventKind::TurnFinished, json!({})),
        ];
        let response = LocalCodingProtocol::default().turn_response(
            orchestration.clone(),
            LcpTurnResponseInput {
                session_id: "s1",
                turn_id: None,
                status: "ready",
                events: &events,
                usage: LcpUsageSnapshot::new(42, 80_000).with_cached_tokens(10),
                approvals: Vec::new(),
            },
        );
        let output = TaskTurnOutput {
            orchestration,
            response,
            summary: TaskRunSummary {
                session_id: "s1".into(),
                events: events.len(),
                agents: vec!["architect".into()],
                artifacts: Vec::new(),
            },
        };

        let rendered = format_task_output(&output);

        assert!(rendered.contains("answer: Привет! Чем помочь?"));
        assert!(rendered.contains("usage=42/80000 tokens cached=10"));
        assert!(!rendered.contains("tool_requested"));
        assert!(!rendered.contains("read_file"));
    }

    #[test]
    fn task_output_includes_actionable_approval_id() {
        let orchestration = CodingOrchestrator.plan_turn(
            TurnRequest {
                workspace: WorkspaceRef {
                    root: "/tmp/project".into(),
                    branch: None,
                    session_id: None,
                },
                message: "создай файл smoke.txt с текстом hello".into(),
                user_id: Some("cli".into()),
            },
            default_agent_matrix(),
        );
        let events = vec![
            Event::new(
                1,
                "s1",
                None,
                EventKind::TurnStarted,
                json!({"objective":"создай файл"}),
            ),
            Event::new(
                2,
                "s1",
                Some("coder".into()),
                EventKind::ToolRequested,
                json!({"name":"apply_patch","approval_id":"appr_0001"}),
            ),
        ];
        let response = LocalCodingProtocol::default().turn_response(
            orchestration.clone(),
            LcpTurnResponseInput {
                session_id: "s1",
                turn_id: None,
                status: "waiting_approval",
                events: &events,
                usage: LcpUsageSnapshot::new(0, 80_000),
                approvals: Vec::new(),
            },
        );
        let output = TaskTurnOutput {
            orchestration,
            response,
            summary: TaskRunSummary {
                session_id: "s1".into(),
                events: events.len(),
                agents: vec!["coder".into()],
                artifacts: Vec::new(),
            },
        };

        let rendered = format_task_output(&output);

        assert!(rendered.contains("approval: appr_0001 требуется подтверждение: apply_patch"));
    }
}
