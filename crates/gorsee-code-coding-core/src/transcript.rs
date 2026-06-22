use std::collections::BTreeSet;

use gorsee_code_core::{Event, EventKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptEventKind {
    UserMessage,
    AssistantMessage,
    Thinking,
    ToolSummary,
    DiffReady,
    ApprovalNeeded,
    VerificationResult,
    ErrorSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptEvent {
    pub sequence: u64,
    pub kind: TranscriptEventKind,
    pub agent_id: Option<String>,
    pub summary: String,
    pub detail: Option<String>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TranscriptMapper;

impl TranscriptMapper {
    pub fn map_events(&self, events: &[Event]) -> Vec<TranscriptEvent> {
        let visible_messages = visible_agent_messages(events);
        let mapped = events
            .iter()
            .filter_map(|event| map_event(event, &visible_messages))
            .collect();
        assistant_messages_last_in_turn(mapped)
    }
}

fn assistant_messages_last_in_turn(events: Vec<TranscriptEvent>) -> Vec<TranscriptEvent> {
    let mut output = Vec::new();
    let mut turn = Vec::new();
    for event in events {
        if event.kind == TranscriptEventKind::UserMessage {
            flush_turn(&mut output, &mut turn);
        }
        turn.push(event);
    }
    flush_turn(&mut output, &mut turn);
    output
}

fn flush_turn(output: &mut Vec<TranscriptEvent>, turn: &mut Vec<TranscriptEvent>) {
    if turn.is_empty() {
        return;
    }
    let events = std::mem::take(turn);
    let (messages, other): (Vec<_>, Vec<_>) = events
        .into_iter()
        .partition(|event| event.kind == TranscriptEventKind::AssistantMessage);
    output.extend(other);
    output.extend(messages);
}

fn visible_agent_messages(events: &[Event]) -> BTreeSet<u64> {
    let mut visible = BTreeSet::new();
    let mut latest_in_turn = None;
    let mut in_visible_turn = false;
    for event in events {
        if starts_visible_turn(event) {
            latest_in_turn = None;
            in_visible_turn = true;
            continue;
        }
        if in_visible_turn && event.kind == EventKind::AgentMessage {
            latest_in_turn = Some(event.sequence);
        }
        if in_visible_turn && finishes_visible_turn(event) {
            if let Some(sequence) = latest_in_turn.take() {
                visible.insert(sequence);
            }
            in_visible_turn = false;
        }
    }
    visible
}

fn starts_visible_turn(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::SessionStarted | EventKind::TurnStarted
    ) && event
        .payload
        .get("objective")
        .and_then(Value::as_str)
        .is_some_and(|objective| !is_bootstrap_objective(objective))
}

fn finishes_visible_turn(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::SessionFinished | EventKind::TurnFinished
    )
}

fn map_event(event: &Event, visible_agent_messages: &BTreeSet<u64>) -> Option<TranscriptEvent> {
    match event.kind {
        EventKind::SessionStarted | EventKind::TurnStarted => user_message(event),
        EventKind::AgentMessage if visible_agent_messages.contains(&event.sequence) => {
            message_event(event, TranscriptEventKind::AssistantMessage)
        }
        EventKind::AgentMessage => None,
        EventKind::ToolRequested if has_approval_id(event) => approval_request_event(event),
        EventKind::ToolApproved | EventKind::ToolDenied => {
            tool_event(event, TranscriptEventKind::ApprovalNeeded)
        }
        EventKind::PatchProposed | EventKind::PatchApplied | EventKind::DiffReady => {
            tool_event(event, TranscriptEventKind::DiffReady)
        }
        EventKind::TestStarted | EventKind::TestFinished => {
            tool_event(event, TranscriptEventKind::VerificationResult)
        }
        EventKind::Error => error_event(event),
        _ => None,
    }
}

fn approval_request_event(event: &Event) -> Option<TranscriptEvent> {
    let tool_name = payload_text(&event.payload, "name")
        .or_else(|| payload_text(&event.payload, "tool"))
        .unwrap_or_else(|| "tool".into());
    Some(TranscriptEvent {
        sequence: event.sequence,
        kind: TranscriptEventKind::ApprovalNeeded,
        agent_id: event.agent_id.clone(),
        summary: format!("требуется подтверждение: {tool_name}"),
        detail: payload_text(&event.payload, "approval_id"),
    })
}

fn user_message(event: &Event) -> Option<TranscriptEvent> {
    let summary = payload_text(&event.payload, "objective")?;
    if is_bootstrap_objective(&summary) {
        return None;
    }
    Some(TranscriptEvent {
        sequence: event.sequence,
        kind: TranscriptEventKind::UserMessage,
        agent_id: Some("Вы".into()),
        summary,
        detail: None,
    })
}

fn message_event(event: &Event, kind: TranscriptEventKind) -> Option<TranscriptEvent> {
    let summary = payload_text(&event.payload, "message")
        .or_else(|| payload_text(&event.payload, "text"))
        .or_else(|| payload_text(&event.payload, "final_answer"))?;
    Some(TranscriptEvent {
        sequence: event.sequence,
        kind,
        agent_id: event.agent_id.clone(),
        summary,
        detail: None,
    })
}

fn tool_event(event: &Event, kind: TranscriptEventKind) -> Option<TranscriptEvent> {
    let summary =
        match event.kind {
            EventKind::ToolApproved => "инструмент подтвержден".into(),
            EventKind::ToolDenied => "инструмент отклонен".into(),
            EventKind::PatchProposed => "подготовлен diff".into(),
            EventKind::DiffReady => {
                payload_text(&event.payload, "summary").unwrap_or_else(|| "diff готов".into())
            }
            EventKind::PatchApplied => payload_text(&event.payload, "summary")
                .unwrap_or_else(|| "изменения применены".into()),
            EventKind::TestStarted => "проверки запущены".into(),
            EventKind::TestFinished => payload_text(&event.payload, "summary")
                .unwrap_or_else(|| "проверки завершены".into()),
            _ => payload_text(&event.payload, "name")?,
        };
    Some(TranscriptEvent {
        sequence: event.sequence,
        kind,
        agent_id: event.agent_id.clone(),
        summary,
        detail: payload_text(&event.payload, "output")
            .or_else(|| payload_text(&event.payload, "artifact")),
    })
}

fn error_event(event: &Event) -> Option<TranscriptEvent> {
    let detail = payload_text(&event.payload, "error")
        .or_else(|| payload_text(&event.payload, "message"))
        .unwrap_or_else(|| "ошибка выполнения".into());
    Some(TranscriptEvent {
        sequence: event.sequence,
        kind: TranscriptEventKind::ErrorSummary,
        agent_id: event.agent_id.clone(),
        summary: human_error(&detail),
        detail: Some(detail),
    })
}

fn payload_text(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(compact)
        .filter(|value| !value.is_empty())
}

fn has_approval_id(event: &Event) -> bool {
    event
        .payload
        .get("approval_id")
        .and_then(Value::as_str)
        .is_some_and(|id| !id.trim().is_empty())
}

fn compact(text: &str) -> String {
    let mut value = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.chars().count() > 220 {
        value = value.chars().take(217).collect::<String>() + "...";
    }
    value
}

fn human_error(detail: &str) -> String {
    let lower = detail.to_lowercase();
    if lower.contains("missing_auth") {
        return "Не найден ключ NeuroGate API.".into();
    }
    if lower.contains("invalid model response") {
        return "Модель вернула ответ в неподдерживаемом формате.".into();
    }
    compact(detail)
}

fn is_bootstrap_objective(summary: &str) -> bool {
    let normalized = summary.to_lowercase().replace('_', " ");
    matches!(
        normalized.trim(),
        "workspace ready" | "gorsee code workspace" | "готово"
    )
}
