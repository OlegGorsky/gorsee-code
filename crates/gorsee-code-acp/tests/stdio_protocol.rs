use agent_client_protocol::schema::v1::{ContentBlock, NewSessionRequest, PromptRequest};
use gorsee_code_acp::{
    stdio::{AcpPromptRunner, AcpProtocolState},
    AcpAdapter, AcpTurnInput, AcpTurnOutput,
};
use gorsee_code_agent::EventObserver;
use gorsee_code_coding_core::{
    LcpTurnResponse, LcpTurnResponseInput, LcpUsageSnapshot, LocalCodingProtocol, OrchestrationPlan,
};
use gorsee_code_core::{default_agent_matrix, Event, EventKind};
use serde_json::{json, Value};

#[test]
fn invalid_params_are_protocol_errors_not_process_errors() {
    let temp = tempfile::tempdir().unwrap();
    let mut state = AcpProtocolState::new(temp.path());

    let response = state
        .handle_line(
            &request_line(1, "initialize", json!({"protocolVersion": 70000})),
            &MissingAuthRunner,
        )
        .unwrap();
    let value: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(value["error"]["code"], -32602);
}

#[test]
fn missing_auth_on_prompt_is_mapped_to_acp_auth_required() {
    let temp = tempfile::tempdir().unwrap();
    let mut state = AcpProtocolState::new(temp.path());
    let new_session = serde_json::to_value(NewSessionRequest::new(temp.path())).unwrap();
    let response = state
        .handle_line(
            &request_line(1, "session/new", new_session),
            &MissingAuthRunner,
        )
        .unwrap();
    let session_id = serde_json::from_str::<Value>(&response).unwrap()["result"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();
    let prompt = PromptRequest::new(session_id, vec![ContentBlock::from("привет")]);

    let response = state
        .handle_line(
            &request_line(2, "session/prompt", serde_json::to_value(prompt).unwrap()),
            &MissingAuthRunner,
        )
        .unwrap();
    let value: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(value["error"]["code"], -32000);
}

#[test]
fn prompt_streams_observed_agent_message_before_end_turn() {
    let temp = tempfile::tempdir().unwrap();
    let mut state = AcpProtocolState::new(temp.path());
    let new_session = serde_json::to_value(NewSessionRequest::new(temp.path())).unwrap();
    let response = state
        .handle_line(
            &request_line(3, "session/new", new_session),
            &MissingAuthRunner,
        )
        .unwrap();
    let session_id = serde_json::from_str::<Value>(&response).unwrap()["result"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();
    let prompt = PromptRequest::new(session_id, vec![ContentBlock::from("привет")]);

    let responses = state
        .handle_line_batch(
            &request_line(4, "session/prompt", serde_json::to_value(prompt).unwrap()),
            &StreamingRunner,
        )
        .unwrap();
    let joined = responses.join("\n");
    let last: Value = serde_json::from_str(responses.last().unwrap()).unwrap();

    assert!(joined.contains("agent_message_chunk"));
    assert!(joined.contains("Привет! Чем помочь?"));
    assert!(!joined.contains("tool_requested"));
    assert!(!joined.contains("tool_finished"));
    assert!(!joined.contains("read_file"));
    assert_eq!(last["result"]["stopReason"], "end_turn");
}

struct MissingAuthRunner;
struct StreamingRunner;

impl AcpPromptRunner for MissingAuthRunner {
    fn run_acp_prompt(&self, _input: AcpTurnInput) -> Result<AcpTurnOutput, String> {
        Err("missing_auth: test runner".into())
    }
}

impl AcpPromptRunner for StreamingRunner {
    fn run_acp_prompt(&self, _input: AcpTurnInput) -> Result<AcpTurnOutput, String> {
        Err("streaming runner requires observed prompt".into())
    }

    fn run_acp_prompt_observed(
        &self,
        input: AcpTurnInput,
        mut observer: Box<EventObserver>,
    ) -> Result<AcpTurnOutput, String> {
        let session_id = input.session_id.clone().unwrap_or_else(|| "s".into());
        observer(&Event::new(
            1,
            &session_id,
            Some("architect".into()),
            EventKind::AgentMessage,
            json!({"message":"Привет! Чем помочь?"}),
        ))
        .map_err(|error| error.to_string())?;
        observer(&Event::new(
            2,
            &session_id,
            Some("architect".into()),
            EventKind::ToolRequested,
            json!({"name":"read_file"}),
        ))
        .map_err(|error| error.to_string())?;
        observer(&Event::new(
            3,
            &session_id,
            Some("architect".into()),
            EventKind::ToolFinished,
            json!({"name":"read_file","output":"done"}),
        ))
        .map_err(|error| error.to_string())?;
        let orchestration = AcpAdapter::default().plan_prompt(input, default_agent_matrix());
        let response = lcp_response(&orchestration, &session_id, "Привет! Чем помочь?");
        Ok(AcpTurnOutput {
            protocol_version: AcpAdapter::protocol_version(),
            orchestration,
            response,
            summary: gorsee_code_agent::TaskRunSummary {
                session_id,
                events: 1,
                agents: vec!["architect".into()],
                artifacts: Vec::new(),
            },
        })
    }
}

fn lcp_response(
    orchestration: &OrchestrationPlan,
    session_id: &str,
    assistant_message: &str,
) -> LcpTurnResponse {
    let events = vec![
        Event::new(
            1,
            session_id,
            None,
            EventKind::TurnStarted,
            json!({"objective": orchestration.request.message.clone()}),
        ),
        Event::new(
            2,
            session_id,
            Some("architect".into()),
            EventKind::AgentMessage,
            json!({"message": assistant_message}),
        ),
    ];
    LocalCodingProtocol::default().turn_response(
        orchestration.clone(),
        LcpTurnResponseInput {
            session_id,
            turn_id: None,
            status: "ready",
            events: &events,
            usage: LcpUsageSnapshot::new(0, 80_000),
            approvals: Vec::new(),
        },
    )
}

fn request_line(id: i64, method: &str, params: Value) -> String {
    serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    }))
    .unwrap()
}
