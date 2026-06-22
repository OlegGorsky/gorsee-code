use agent_client_protocol::schema::v1::{
    AgentNotification, ContentBlock, ContentChunk, JsonRpcMessage, Notification, Plan, PlanEntry,
    PlanEntryPriority, PlanEntryStatus, SessionNotification, SessionUpdate, ToolCall,
    ToolCallContent, ToolCallStatus,
};
use gorsee_code_coding_core::{
    LcpTurnResponse, PlanRisk, TranscriptEvent, TranscriptEventKind, TurnPlan,
};
use gorsee_code_core::{Event, EventKind};

pub fn response_notification_lines(
    session_id: &str,
    response: &LcpTurnResponse,
    plan: Option<&TurnPlan>,
    include_transcript: bool,
) -> Result<Vec<String>, serde_json::Error> {
    response_notifications(session_id, response, plan, include_transcript)
        .into_iter()
        .map(serialize_notification)
        .collect()
}

pub fn event_notification_lines(
    session_id: &str,
    event: &Event,
) -> Result<Vec<String>, serde_json::Error> {
    event_update(event)
        .map(|update| vec![session_notification(session_id, update)])
        .unwrap_or_default()
        .into_iter()
        .map(serialize_notification)
        .collect()
}

fn response_notifications(
    session_id: &str,
    response: &LcpTurnResponse,
    plan: Option<&TurnPlan>,
    include_transcript: bool,
) -> Vec<AgentNotification> {
    let mut updates = Vec::new();
    if let Some(update) = plan_update(plan) {
        updates.push(update);
    }
    if include_transcript {
        updates.extend(response.transcript.iter().filter_map(transcript_update));
    }
    updates
        .into_iter()
        .map(|update| session_notification(session_id, update))
        .collect()
}

fn plan_update(plan: Option<&TurnPlan>) -> Option<SessionUpdate> {
    let plan = plan?;
    let entries = plan
        .steps
        .iter()
        .map(|step| {
            PlanEntry::new(
                step.description.clone(),
                priority_for_risk(step.risk),
                PlanEntryStatus::Pending,
            )
        })
        .collect();
    Some(SessionUpdate::Plan(Plan::new(entries)))
}

fn transcript_update(event: &TranscriptEvent) -> Option<SessionUpdate> {
    match event.kind {
        TranscriptEventKind::AssistantMessage => Some(SessionUpdate::AgentMessageChunk(
            ContentChunk::new(ContentBlock::from(event.summary.clone())),
        )),
        TranscriptEventKind::Thinking => Some(SessionUpdate::AgentThoughtChunk(ContentChunk::new(
            ContentBlock::from(event.summary.clone()),
        ))),
        TranscriptEventKind::ToolSummary
        | TranscriptEventKind::DiffReady
        | TranscriptEventKind::ApprovalNeeded
        | TranscriptEventKind::VerificationResult => Some(SessionUpdate::ToolCall(
            ToolCall::new(format!("lcp-{}", event.sequence), event.summary.clone())
                .status(ToolCallStatus::Completed)
                .content(vec![ToolCallContent::from(event.summary.clone())]),
        )),
        TranscriptEventKind::ErrorSummary => Some(SessionUpdate::AgentMessageChunk(
            ContentChunk::new(ContentBlock::from(format!("Ошибка: {}", event.summary))),
        )),
        TranscriptEventKind::UserMessage => None,
    }
}

fn event_update(event: &Event) -> Option<SessionUpdate> {
    match event.kind {
        EventKind::AgentThinking => text_update(event, "message", SessionUpdate::AgentThoughtChunk),
        EventKind::AgentMessage => text_update(event, "message", SessionUpdate::AgentMessageChunk),
        EventKind::DiffReady => Some(simple_tool_update(
            event,
            &payload_text(event, "summary").unwrap_or_else(|| "diff готов".into()),
            ToolCallStatus::Completed,
        )),
        EventKind::TestStarted => Some(simple_tool_update(
            event,
            "проверки запущены",
            ToolCallStatus::InProgress,
        )),
        EventKind::TestFinished => Some(simple_tool_update(
            event,
            &payload_text(event, "summary").unwrap_or_else(|| "проверки завершены".into()),
            ToolCallStatus::Completed,
        )),
        EventKind::Error => error_update(event),
        _ => None,
    }
}

fn error_update(event: &Event) -> Option<SessionUpdate> {
    let text = payload_text(event, "error")?;
    Some(SessionUpdate::AgentMessageChunk(ContentChunk::new(
        ContentBlock::from(format!("Ошибка: {text}")),
    )))
}

fn text_update(
    event: &Event,
    key: &str,
    wrap: impl FnOnce(ContentChunk) -> SessionUpdate,
) -> Option<SessionUpdate> {
    let text = payload_text(event, key)?;
    Some(wrap(ContentChunk::new(ContentBlock::from(text))))
}

fn simple_tool_update(event: &Event, label: &str, status: ToolCallStatus) -> SessionUpdate {
    let summary = compact(label);
    SessionUpdate::ToolCall(
        ToolCall::new(format!("event-{}", event.sequence), summary.clone())
            .status(status)
            .content(vec![ToolCallContent::from(summary)]),
    )
}

fn priority_for_risk(risk: PlanRisk) -> PlanEntryPriority {
    match risk {
        PlanRisk::Read => PlanEntryPriority::Low,
        PlanRisk::Write => PlanEntryPriority::High,
        PlanRisk::Command => PlanEntryPriority::Medium,
    }
}

fn serialize_notification(notification: AgentNotification) -> Result<String, serde_json::Error> {
    let method = notification.method().to_string();
    serde_json::to_string(&JsonRpcMessage::wrap(Notification {
        method: method.into(),
        params: Some(notification),
    }))
}

fn session_notification(session_id: &str, update: SessionUpdate) -> AgentNotification {
    AgentNotification::SessionNotification(SessionNotification::new(session_id.to_string(), update))
}

fn payload_text(event: &Event, key: &str) -> Option<String> {
    event
        .payload
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(compact)
        .filter(|value| !value.is_empty())
}

fn compact(text: &str) -> String {
    let mut value = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.chars().count() > 220 {
        value = value.chars().take(217).collect::<String>() + "...";
    }
    value
}

#[cfg(test)]
mod tests {
    use gorsee_code_coding_core::{
        LcpTurnResponseInput, LcpUsageSnapshot, LocalCodingProtocol, TurnRequest, WorkspaceRef,
    };
    use gorsee_code_core::{default_agent_matrix, Event, EventKind};
    use serde_json::{json, Value};

    use super::*;

    #[test]
    fn lcp_snapshot_maps_to_plan_and_agent_message_notifications() {
        let protocol = LocalCodingProtocol::default();
        let orchestration = protocol
            .plan_turn(request("создай файл"), default_agent_matrix())
            .orchestration;
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
                EventKind::AgentMessage,
                json!({"message":"Готово: файл создан."}),
            ),
            Event::new(3, "s1", None, EventKind::TurnFinished, json!({})),
        ];
        let response = protocol.turn_response(
            orchestration.clone(),
            LcpTurnResponseInput {
                session_id: "s1",
                turn_id: None,
                status: "ready",
                events: &events,
                usage: LcpUsageSnapshot::new(0, 80_000),
                approvals: Vec::new(),
            },
        );

        let lines = response_notification_lines("s1", &response, orchestration.plan.as_ref(), true)
            .unwrap();

        assert!(lines.len() >= 2);
        let first: Value = serde_json::from_str(&lines[0]).unwrap();
        let last: Value = serde_json::from_str(lines.last().unwrap()).unwrap();
        assert_eq!(first["method"], "session/update");
        assert_eq!(first["params"]["update"]["sessionUpdate"], "plan");
        assert_eq!(
            last["params"]["update"]["sessionUpdate"],
            "agent_message_chunk"
        );
    }

    #[test]
    fn event_notifications_use_structured_diff_and_verification_summaries() {
        let diff_lines = event_notification_lines(
            "s1",
            &Event::new(
                7,
                "s1",
                None,
                EventKind::DiffReady,
                json!({"summary":"diff готов: 1 файлов, +2 -0"}),
            ),
        )
        .unwrap();
        let verification_lines = event_notification_lines(
            "s1",
            &Event::new(
                8,
                "s1",
                None,
                EventKind::TestFinished,
                json!({"summary":"проверки пройдены: cargo test"}),
            ),
        )
        .unwrap();

        let diff: Value = serde_json::from_str(&diff_lines[0]).unwrap();
        let verification: Value = serde_json::from_str(&verification_lines[0]).unwrap();
        assert!(diff.to_string().contains("diff готов"));
        assert!(verification.to_string().contains("проверки пройдены"));
    }

    fn request(message: &str) -> TurnRequest {
        TurnRequest {
            workspace: WorkspaceRef {
                root: "/repo".into(),
                branch: None,
                session_id: Some("s1".into()),
            },
            message: message.into(),
            user_id: Some("u".into()),
        }
    }
}
