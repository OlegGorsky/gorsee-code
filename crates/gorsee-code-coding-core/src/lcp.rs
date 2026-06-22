use gorsee_code_core::{AgentProfile, Event};
use serde::{Deserialize, Serialize};

use crate::{
    execution_steps::execution_steps,
    lcp_response::{LcpTurnResponse, LcpTurnResponseInput},
    orchestrator::{CodingOrchestrator, OrchestrationPlan},
    transcript::{TranscriptEvent, TranscriptMapper},
    turn::{ExecutionStepState, PlanRisk, TurnRequest},
};

pub const LCP_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LcpTurnSnapshot {
    pub protocol_version: u16,
    pub orchestration: OrchestrationPlan,
    pub transcript: Vec<TranscriptEvent>,
    pub execution_steps: Vec<LcpExecutionStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcpExecutionStep {
    pub id: String,
    pub state: ExecutionStepState,
    pub risk: PlanRisk,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LocalCodingProtocol {
    orchestrator: CodingOrchestrator,
    transcript: TranscriptMapper,
}

impl LocalCodingProtocol {
    pub fn plan_turn(&self, request: TurnRequest, profiles: Vec<AgentProfile>) -> LcpTurnSnapshot {
        self.snapshot(self.orchestrator.plan_turn(request, profiles), &[])
    }

    pub fn snapshot(&self, orchestration: OrchestrationPlan, events: &[Event]) -> LcpTurnSnapshot {
        LcpTurnSnapshot {
            protocol_version: LCP_VERSION,
            execution_steps: execution_steps(&orchestration, events),
            transcript: self.transcript.map_events(events),
            orchestration,
        }
    }

    pub fn transcript(&self, events: &[Event]) -> Vec<TranscriptEvent> {
        self.transcript.map_events(events)
    }

    pub fn turn_response(
        &self,
        orchestration: OrchestrationPlan,
        input: LcpTurnResponseInput<'_>,
    ) -> LcpTurnResponse {
        let snapshot = self.snapshot(orchestration, input.events);
        LcpTurnResponse::from_snapshot(snapshot, input)
    }
}

#[cfg(test)]
mod tests {
    use gorsee_code_core::{default_agent_matrix, Event, EventKind};
    use serde_json::json;

    use super::*;
    use crate::{TurnRequest, WorkspaceRef};

    #[test]
    fn lcp_plan_turn_exposes_orchestration_and_step_states() {
        let snapshot = LocalCodingProtocol::default()
            .plan_turn(request("создай файл"), default_agent_matrix());

        assert_eq!(snapshot.protocol_version, LCP_VERSION);
        assert_eq!(
            snapshot.orchestration.intent.intent,
            crate::CodingIntent::Edit
        );
        assert_eq!(
            snapshot
                .execution_steps
                .iter()
                .map(|step| (step.id.as_str(), step.state))
                .collect::<Vec<_>>(),
            vec![
                ("inspect_repo", ExecutionStepState::Pending),
                ("apply_changes", ExecutionStepState::Pending),
                ("review_diff", ExecutionStepState::Pending),
                ("verify", ExecutionStepState::Pending),
            ]
        );
    }

    #[test]
    fn lcp_transcript_is_clean_and_protocol_stable() {
        let protocol = LocalCodingProtocol::default();
        let transcript = protocol.transcript(&[
            event(
                1,
                EventKind::TurnStarted,
                None,
                json!({"objective":"привет"}),
            ),
            event(
                2,
                EventKind::ToolStarted,
                Some("architect"),
                json!({"name":"read"}),
            ),
            event(
                3,
                EventKind::AgentMessage,
                Some("architect"),
                json!({"message":"Привет!"}),
            ),
            event(
                4,
                EventKind::TurnFinished,
                None,
                json!({"status":"finished"}),
            ),
        ]);

        assert_eq!(transcript.len(), 2);
        assert_eq!(transcript[0].summary, "привет");
        assert_eq!(transcript[1].summary, "Привет!");
    }

    #[test]
    fn lcp_step_states_follow_tool_and_approval_events() {
        let protocol = LocalCodingProtocol::default();
        let orchestration = protocol
            .plan_turn(request("создай файл src/main.rs"), default_agent_matrix())
            .orchestration;
        let snapshot = protocol.snapshot(
            orchestration,
            &[
                event(
                    1,
                    EventKind::ToolStarted,
                    Some("architect"),
                    json!({"name":"read_file"}),
                ),
                event(
                    2,
                    EventKind::ToolFinished,
                    Some("architect"),
                    json!({"name":"read_file"}),
                ),
                event(
                    3,
                    EventKind::ToolRequested,
                    Some("coder"),
                    json!({"name":"apply_patch","approval_id":"a1"}),
                ),
            ],
        );

        assert_eq!(
            state(&snapshot, "inspect_repo"),
            ExecutionStepState::Succeeded
        );
        assert_eq!(
            state(&snapshot, "apply_changes"),
            ExecutionStepState::WaitingApproval
        );
    }

    #[test]
    fn lcp_step_states_follow_diff_and_verification_events() {
        let protocol = LocalCodingProtocol::default();
        let orchestration = protocol
            .plan_turn(request("измени файл src/main.rs"), default_agent_matrix())
            .orchestration;
        let snapshot = protocol.snapshot(
            orchestration,
            &[
                event(
                    1,
                    EventKind::PatchApplied,
                    Some("coder"),
                    json!({"name":"apply_patch"}),
                ),
                event(
                    2,
                    EventKind::DiffReady,
                    Some("validator"),
                    json!({"status":"ok","summary":"diff готов: 1 файлов, +2 -0"}),
                ),
                event(
                    3,
                    EventKind::TestStarted,
                    Some("validator"),
                    json!({"name":"run_test"}),
                ),
                event(
                    4,
                    EventKind::TestFinished,
                    Some("validator"),
                    json!({"name":"run_test"}),
                ),
            ],
        );

        assert_eq!(
            state(&snapshot, "apply_changes"),
            ExecutionStepState::Succeeded
        );
        assert_eq!(
            state(&snapshot, "review_diff"),
            ExecutionStepState::Succeeded
        );
        assert_eq!(state(&snapshot, "verify"), ExecutionStepState::Succeeded);
    }

    #[test]
    fn lcp_marks_failed_verification_step_as_failed() {
        let protocol = LocalCodingProtocol::default();
        let orchestration = protocol
            .plan_turn(request("запусти тесты"), default_agent_matrix())
            .orchestration;
        let snapshot = protocol.snapshot(
            orchestration,
            &[event(
                1,
                EventKind::TestFinished,
                Some("validator"),
                json!({"status":"failed","summary":"проверки не прошли: cargo test"}),
            )],
        );

        assert_eq!(state(&snapshot, "run_checks"), ExecutionStepState::Failed);
    }

    #[test]
    fn lcp_marks_skipped_verification_step_as_skipped() {
        let protocol = LocalCodingProtocol::default();
        let orchestration = protocol
            .plan_turn(request("запусти тесты"), default_agent_matrix())
            .orchestration;
        let snapshot = protocol.snapshot(
            orchestration,
            &[event(
                1,
                EventKind::TestFinished,
                Some("validator"),
                json!({"status":"skipped","summary":"проверки пропущены: denied by user"}),
            )],
        );

        assert_eq!(state(&snapshot, "run_checks"), ExecutionStepState::Skipped);
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

    fn event(
        sequence: u64,
        kind: EventKind,
        agent_id: Option<&str>,
        payload: serde_json::Value,
    ) -> Event {
        Event::new(sequence, "s", agent_id.map(str::to_string), kind, payload)
    }

    fn state(snapshot: &LcpTurnSnapshot, id: &str) -> ExecutionStepState {
        snapshot
            .execution_steps
            .iter()
            .find(|step| step.id == id)
            .map(|step| step.state)
            .expect("step exists")
    }
}
