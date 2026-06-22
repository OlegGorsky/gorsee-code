use agent_client_protocol::schema::v1::{ContentBlock, NewSessionRequest, PromptRequest};
use gorsee_code_acp::{
    stdio::{AcpPromptRunner, AcpProtocolState},
    AcpAdapter, AcpTurnInput, AcpTurnOutput,
};
use gorsee_code_coding_core::{
    LcpTurnResponse, LcpTurnResponseInput, LcpUsageSnapshot, LocalCodingProtocol, OrchestrationPlan,
};
use gorsee_code_core::{default_agent_matrix, Event, EventKind};
use serde_json::{json, Value};

#[test]
fn initializes_with_agent_capabilities() {
    let temp = tempfile::tempdir().unwrap();
    let mut state = AcpProtocolState::new(temp.path());
    let response = state
        .handle_line(
            &request_line(
                1,
                "initialize",
                json!({"protocolVersion":1,"clientCapabilities":{}}),
            ),
            &NoopRunner,
        )
        .unwrap();
    let value: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(value["result"]["protocolVersion"], 1);
    assert_eq!(value["result"]["agentInfo"]["name"], "gorsee-code");
    assert_eq!(
        value["result"]["agentCapabilities"]["sessionCapabilities"]["list"],
        json!({})
    );
}

#[test]
fn new_session_creates_ready_manifest_and_list_returns_it() {
    let temp = tempfile::tempdir().unwrap();
    let mut state = AcpProtocolState::new(temp.path());
    let params = serde_json::to_value(NewSessionRequest::new(temp.path())).unwrap();

    let response = state
        .handle_line(&request_line(2, "session/new", params), &NoopRunner)
        .unwrap();
    let value: Value = serde_json::from_str(&response).unwrap();
    let session_id = value["result"]["sessionId"].as_str().unwrap();

    assert!(temp
        .path()
        .join(".gorsee-code/sessions")
        .join(session_id)
        .join("manifest.json")
        .exists());

    let response = state
        .handle_line(
            &request_line(3, "session/list", json!({"cwd": temp.path()})),
            &NoopRunner,
        )
        .unwrap();
    let value: Value = serde_json::from_str(&response).unwrap();
    assert_eq!(value["result"]["sessions"][0]["sessionId"], session_id);
}

#[test]
fn prompt_runs_executor_and_returns_end_turn() {
    let temp = tempfile::tempdir().unwrap();
    let mut state = AcpProtocolState::new(temp.path());
    let params = serde_json::to_value(NewSessionRequest::new(temp.path())).unwrap();
    let response = state
        .handle_line(&request_line(4, "session/new", params), &NoopRunner)
        .unwrap();
    let session_id = serde_json::from_str::<Value>(&response).unwrap()["result"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();
    let prompt = PromptRequest::new(
        session_id,
        vec![
            ContentBlock::from("привет"),
            ContentBlock::from("как дела?"),
        ],
    );

    let response = state
        .handle_line(
            &request_line(5, "session/prompt", serde_json::to_value(prompt).unwrap()),
            &SuccessRunner,
        )
        .unwrap();
    let value: Value = serde_json::from_str(&response).unwrap();

    assert_eq!(value["result"]["stopReason"], "end_turn");
}

#[test]
fn prompt_batch_emits_plan_update_before_prompt_response() {
    let temp = tempfile::tempdir().unwrap();
    let mut state = AcpProtocolState::new(temp.path());
    let params = serde_json::to_value(NewSessionRequest::new(temp.path())).unwrap();
    let response = state
        .handle_line(&request_line(6, "session/new", params), &NoopRunner)
        .unwrap();
    let session_id = serde_json::from_str::<Value>(&response).unwrap()["result"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();
    let prompt = PromptRequest::new(session_id, vec![ContentBlock::from("создай файл")]);

    let responses = state
        .handle_line_batch(
            &request_line(7, "session/prompt", serde_json::to_value(prompt).unwrap()),
            &SuccessRunner,
        )
        .unwrap();

    assert!(responses.len() >= 2);
    let first: Value = serde_json::from_str(&responses[0]).unwrap();
    let last: Value = serde_json::from_str(responses.last().unwrap()).unwrap();
    assert_eq!(first["method"], "session/update");
    assert_eq!(first["params"]["update"]["sessionUpdate"], "plan");
    assert_eq!(last["result"]["stopReason"], "end_turn");
}

struct NoopRunner;
struct SuccessRunner;

impl AcpPromptRunner for NoopRunner {
    fn run_acp_prompt(&self, _input: AcpTurnInput) -> Result<AcpTurnOutput, String> {
        Err("missing_auth: test runner".into())
    }
}

impl AcpPromptRunner for SuccessRunner {
    fn run_acp_prompt(&self, input: AcpTurnInput) -> Result<AcpTurnOutput, String> {
        let session_id = input.session_id.clone().unwrap_or_else(|| "s".into());
        let orchestration = AcpAdapter::default().plan_prompt(input, default_agent_matrix());
        let response = lcp_response(&orchestration, &session_id, "Готово.");
        Ok(AcpTurnOutput {
            protocol_version: AcpAdapter::protocol_version(),
            orchestration,
            response,
            summary: gorsee_code_agent::TaskRunSummary {
                session_id,
                events: 0,
                agents: Vec::new(),
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
