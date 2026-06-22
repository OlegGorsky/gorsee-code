use gorsee_code_core::{Event, EventKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CodingIntent, LcpExecutionStep, LcpTurnSnapshot, TranscriptEvent};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LcpTurnResponse {
    pub protocol_version: u16,
    pub session_id: String,
    pub turn_id: String,
    pub intent: CodingIntent,
    pub status: String,
    pub transcript: Vec<TranscriptEvent>,
    pub execution_steps: Vec<LcpExecutionStep>,
    pub diff: Option<LcpDiffSummary>,
    pub verification: Option<LcpVerificationSummary>,
    pub usage: LcpUsageSnapshot,
    pub approvals: Vec<LcpApprovalSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LcpUsageSnapshot {
    pub tokens_used: u64,
    pub cached_tokens: u64,
    pub tokens_limit: u64,
    pub percent_used: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcpApprovalSummary {
    pub id: String,
    pub agent_id: String,
    pub tool_name: String,
    pub risk: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcpDiffSummary {
    pub status: String,
    pub summary: String,
    pub files_changed: usize,
    pub additions: usize,
    pub deletions: usize,
    pub artifact: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcpVerificationSummary {
    pub status: String,
    pub summary: String,
    pub command: Option<String>,
    pub artifact: Option<String>,
}

pub struct LcpTurnResponseInput<'a> {
    pub session_id: &'a str,
    pub turn_id: Option<&'a str>,
    pub status: &'a str,
    pub events: &'a [Event],
    pub usage: LcpUsageSnapshot,
    pub approvals: Vec<LcpApprovalSummary>,
}

impl LcpUsageSnapshot {
    pub fn new(tokens_used: u64, tokens_limit: u64) -> Self {
        let percent_used = if tokens_limit == 0 {
            0.0
        } else {
            tokens_used as f64 / tokens_limit as f64 * 100.0
        };
        Self {
            tokens_used,
            cached_tokens: 0,
            tokens_limit,
            percent_used,
        }
    }

    pub fn with_cached_tokens(mut self, cached_tokens: u64) -> Self {
        self.cached_tokens = cached_tokens;
        self
    }
}

impl LcpTurnResponse {
    pub(crate) fn from_snapshot(
        snapshot: LcpTurnSnapshot,
        input: LcpTurnResponseInput<'_>,
    ) -> Self {
        Self {
            protocol_version: snapshot.protocol_version,
            session_id: input.session_id.into(),
            turn_id: input
                .turn_id
                .map(str::to_string)
                .unwrap_or_else(|| turn_id(input.session_id, input.events)),
            intent: snapshot.orchestration.intent.intent,
            status: input.status.into(),
            transcript: snapshot.transcript,
            execution_steps: snapshot.execution_steps,
            diff: diff_summary(input.events),
            verification: verification_summary(input.events),
            usage: input.usage,
            approvals: input.approvals,
        }
    }
}

fn turn_id(session_id: &str, events: &[Event]) -> String {
    events
        .iter()
        .rev()
        .find(|event| {
            matches!(
                event.kind,
                EventKind::TurnStarted | EventKind::SessionStarted
            )
        })
        .map(|event| format!("{session_id}:{:04}", event.sequence))
        .unwrap_or_else(|| format!("{session_id}:0000"))
}

fn diff_summary(events: &[Event]) -> Option<LcpDiffSummary> {
    let event = events
        .iter()
        .rev()
        .find(|event| event.kind == EventKind::DiffReady)?;
    Some(LcpDiffSummary {
        status: text(&event.payload, "status").unwrap_or_else(|| "ok".into()),
        summary: text(&event.payload, "summary").unwrap_or_else(|| "diff готов".into()),
        files_changed: number(&event.payload, "files_changed"),
        additions: number(&event.payload, "additions"),
        deletions: number(&event.payload, "deletions"),
        artifact: text(&event.payload, "artifact"),
    })
}

fn verification_summary(events: &[Event]) -> Option<LcpVerificationSummary> {
    let event = events
        .iter()
        .rev()
        .find(|event| matches!(event.kind, EventKind::TestStarted | EventKind::TestFinished))?;
    let finished = event.kind == EventKind::TestFinished;
    Some(LcpVerificationSummary {
        status: text(&event.payload, "status").unwrap_or_else(|| {
            if finished {
                "passed".into()
            } else {
                "running".into()
            }
        }),
        summary: text(&event.payload, "summary").unwrap_or_else(|| {
            if finished {
                "проверки завершены".into()
            } else {
                "проверки запущены".into()
            }
        }),
        command: text(&event.payload, "command"),
        artifact: text(&event.payload, "artifact"),
    })
}

fn text(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).map(str::to_string)
}

fn number(payload: &Value, key: &str) -> usize {
    payload
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use gorsee_code_core::{default_agent_matrix, Event, EventKind};
    use serde_json::json;

    use super::*;
    use crate::{LocalCodingProtocol, TurnRequest, WorkspaceRef};

    #[test]
    fn response_contains_stable_client_turn_state() {
        let protocol = LocalCodingProtocol::default();
        let orchestration = protocol
            .plan_turn(request("измени файл src/lib.rs"), default_agent_matrix())
            .orchestration;
        let events = vec![
            event(
                1,
                EventKind::TurnStarted,
                json!({"objective":"измени файл"}),
            ),
            event(
                2,
                EventKind::DiffReady,
                json!({
                    "status":"ok",
                    "summary":"diff готов: 1 файлов, +2 -0",
                    "files_changed":1,
                    "additions":2,
                    "deletions":0,
                    "artifact":"diff.json"
                }),
            ),
            event(
                3,
                EventKind::TestFinished,
                json!({
                    "status":"skipped",
                    "summary":"проверки пропущены: denied by user",
                    "artifact":"verification.json"
                }),
            ),
        ];

        let response = protocol.turn_response(orchestration, response_input("s1", &events));

        assert_eq!(response.session_id, "s1");
        assert_eq!(response.turn_id, "s1:0001");
        assert_eq!(response.intent, CodingIntent::Edit);
        assert_eq!(response.status, "ready");
        assert_eq!(response.usage.tokens_used, 5);
        assert_eq!(response.approvals[0].tool_name, "run_test");
        assert_eq!(response.diff.as_ref().unwrap().files_changed, 1);
        assert_eq!(response.verification.as_ref().unwrap().status, "skipped");
        assert_eq!(response.transcript[0].summary, "измени файл");
    }

    fn response_input<'a>(session_id: &'a str, events: &'a [Event]) -> LcpTurnResponseInput<'a> {
        LcpTurnResponseInput {
            session_id,
            turn_id: None,
            status: "ready",
            events,
            usage: LcpUsageSnapshot::new(5, 100),
            approvals: vec![LcpApprovalSummary {
                id: "a1".into(),
                agent_id: "validator".into(),
                tool_name: "run_test".into(),
                risk: "command".into(),
                status: "pending".into(),
            }],
        }
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

    fn event(sequence: u64, kind: EventKind, payload: serde_json::Value) -> Event {
        Event::new(sequence, "s", None, kind, payload)
    }
}
