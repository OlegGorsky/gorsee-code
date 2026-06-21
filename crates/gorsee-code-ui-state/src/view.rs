use gorsee_code_core::{AgentProfile, AgentStatus, Event, EventKind};
use gorsee_code_usage::BudgetStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionView {
    pub id: String,
    pub title: String,
    pub status: String,
    pub repo: String,
    pub branch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentView {
    pub id: String,
    pub role: String,
    pub model: String,
    pub status: String,
    pub tokens_used: u64,
    pub tokens_limit: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventView {
    pub sequence: u64,
    pub kind: String,
    pub agent_id: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallView {
    pub id: String,
    pub name: String,
    pub status: String,
    pub risk: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetView {
    pub used_tokens: u64,
    pub limit_tokens: u64,
    pub percent_used: f64,
    pub warning: bool,
    pub stopped: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub session: SessionView,
    pub agents: Vec<AgentView>,
    pub timeline: Vec<EventView>,
    pub budget: BudgetView,
    pub approvals: Vec<ToolCallView>,
    pub gateway_status: String,
}

impl AgentView {
    pub fn from_profile(profile: &AgentProfile, status: AgentStatus, tokens_used: u64) -> Self {
        Self::from_parts(
            profile.id(),
            &profile.model,
            status,
            tokens_used,
            profile.budget_tokens,
        )
    }

    pub fn from_parts(
        id: &str,
        model: &str,
        status: AgentStatus,
        tokens_used: u64,
        tokens_limit: u64,
    ) -> Self {
        Self {
            id: id.into(),
            role: id.into(),
            model: model.into(),
            status: format!("{status:?}").to_lowercase(),
            tokens_used,
            tokens_limit,
        }
    }
}

impl EventView {
    pub fn from_event(event: &Event) -> Self {
        Self {
            sequence: event.sequence,
            kind: kind_label(&event.kind).into(),
            agent_id: event_actor(event),
            summary: summarize_event(event),
        }
    }
}

impl From<BudgetStatus> for BudgetView {
    fn from(status: BudgetStatus) -> Self {
        Self {
            used_tokens: status.used_tokens,
            limit_tokens: status.limit_tokens,
            percent_used: status.percent_used,
            warning: status.warning,
            stopped: status.stopped,
        }
    }
}

fn summarize_event(event: &Event) -> String {
    match &event.kind {
        EventKind::SessionStarted => {
            payload_text(event, "objective").unwrap_or_else(|| "новая сессия".to_string())
        }
        EventKind::SessionFinished => "готово".into(),
        EventKind::SessionPaused => "сессия на паузе".into(),
        EventKind::SessionResumed => "сессия продолжена".into(),
        EventKind::AgentStarted => payload_text(event, "model")
            .map(|model| format!("начал работу · {model}"))
            .unwrap_or_else(|| "начал работу".into()),
        EventKind::AgentMessage => payload_message(event).unwrap_or_else(|| "ответ".into()),
        EventKind::ToolRequested => tool_summary(event, "запросил"),
        EventKind::ToolStarted => tool_summary(event, "запустил"),
        EventKind::ToolFinished => tool_finished_summary(event),
        EventKind::ToolApproved => tool_summary(event, "подтвержден"),
        EventKind::ToolDenied => tool_summary(event, "отклонен"),
        EventKind::PatchProposed => tool_summary(event, "предложил patch"),
        EventKind::PatchApplied => "patch применен".into(),
        EventKind::BudgetWarning => "лимит близко к порогу".into(),
        EventKind::BudgetExceeded => "лимит исчерпан".into(),
        EventKind::Error => payload_text(event, "error")
            .or_else(|| payload_message(event))
            .unwrap_or_else(|| "ошибка".into()),
        EventKind::SkillStarted => payload_text(event, "skill")
            .map(|skill| format!("скилл запущен: {skill}"))
            .unwrap_or_else(|| "скилл запущен".into()),
        EventKind::SkillFinished => payload_text(event, "skill")
            .map(|skill| format!("скилл завершен: {skill}"))
            .unwrap_or_else(|| "скилл завершен".into()),
        EventKind::ArtifactCreated => "артефакт создан".into(),
        EventKind::TestStarted => "тесты запущены".into(),
        EventKind::TestFinished => "тесты завершены".into(),
        EventKind::SearchStarted => "поиск запущен".into(),
        EventKind::SearchFinished => "поиск завершен".into(),
        EventKind::HookStarted => "hook запущен".into(),
        EventKind::HookFinished => "hook завершен".into(),
        EventKind::ModelCapabilityDetected => "модель проверена".into(),
        EventKind::VisionAnalyzed => "изображение проанализировано".into(),
        EventKind::ImageGenerated => "изображение создано".into(),
        EventKind::AgentDelegated => "делегировал задачу".into(),
        EventKind::AgentThinking => "думает".into(),
        EventKind::ContextUpdated => context_summary(event),
    }
}

fn kind_label(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::SessionStarted => "user",
        EventKind::AgentMessage => "assistant",
        EventKind::ToolRequested
        | EventKind::ToolStarted
        | EventKind::ToolFinished
        | EventKind::ToolApproved
        | EventKind::ToolDenied => "tool",
        EventKind::PatchProposed | EventKind::PatchApplied => "patch",
        EventKind::BudgetWarning | EventKind::BudgetExceeded => "limit",
        EventKind::Error => "error",
        _ => "process",
    }
}

fn event_actor(event: &Event) -> Option<String> {
    match &event.kind {
        EventKind::SessionStarted => Some("Вы".into()),
        _ => event.agent_id.clone(),
    }
}

fn payload_message(event: &Event) -> Option<String> {
    payload_text(event, "message").or_else(|| payload_text(event, "text"))
}

fn payload_text(event: &Event, key: &str) -> Option<String> {
    event
        .payload
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn tool_summary(event: &Event, action: &str) -> String {
    payload_text(event, "name")
        .or_else(|| payload_text(event, "tool"))
        .map(|name| format!("{action} {name}"))
        .unwrap_or_else(|| action.to_string())
}

fn tool_finished_summary(event: &Event) -> String {
    let name = payload_text(event, "name").unwrap_or_else(|| "tool".into());
    match payload_text(event, "output") {
        Some(output) => format!("завершил {name}: {}", compact(&output, 96)),
        None => format!("завершил {name}"),
    }
}

fn context_summary(event: &Event) -> String {
    let answers = event
        .payload
        .get("answers")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let tool_results = event
        .payload
        .get("tool_results")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    format!("контекст обновлен: {answers} ответов, {tool_results} результатов")
}

fn compact(value: &str, limit: usize) -> String {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.chars().count() <= limit {
        return value;
    }
    let mut compact = value
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    compact.push('…');
    compact
}
