use std::{cell::RefCell, fs, path::Path};

use gorsee_code_agent::{ChatClient, MissionRunner};
use gorsee_code_core::{default_agent_matrix, EventKind, MissionSpec};
use gorsee_code_neurogate::{ChatRequest, ChatResponse};
use gorsee_code_session::SessionStore;
use serde_json::json;

#[test]
fn sequential_runner_uses_model_and_records_tool_results() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let client = MockClient::new(vec![
        json!({
            "message": "inspect repo",
            "tool_calls": [{ "name": "list_files", "args": {} }]
        })
        .to_string(),
        json!({
            "message": "files inspected",
            "final_answer": "architect done"
        })
        .to_string(),
        final_answer("scout done"),
        final_answer("coder done"),
        final_answer("validator done"),
        final_answer("summarizer done"),
    ]);
    let runner = MissionRunner::new(temp.path().join(".gorsee-code"));
    let spec = MissionSpec::new("inspect repository", temp.path().display().to_string());

    let summary = runner.run_sequential(&spec, &client).unwrap();

    assert_eq!(client.requests.borrow().len(), 6);
    assert_eq!(client.requests.borrow()[0].model, "neurogate/gpt-5");
    assert_eq!(client.requests.borrow()[1].model, "neurogate/gpt-5");
    let store = SessionStore::new(
        temp.path().join(".gorsee-code"),
        gorsee_code_safety::Redactor::default(),
    );
    let events = store.read_events(&summary.session_id).unwrap();
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::ToolStarted));
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::ToolFinished));
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::MissionFinished));
    assert!(Path::new(&summary.artifacts[0].path).exists());
}

fn final_answer(answer: &str) -> String {
    json!({
        "message": answer,
        "final_answer": answer
    })
    .to_string()
}

#[test]
fn sequential_runner_executes_the_full_agent_matrix() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let client = MockClient::new(
        default_agent_matrix()
            .iter()
            .map(|agent| {
                json!({
                    "message": format!("{} finished", agent.id()),
                    "final_answer": format!("{} answer", agent.id())
                })
                .to_string()
            })
            .collect(),
    );
    let runner = MissionRunner::new(temp.path().join(".gorsee-code"));
    let spec = MissionSpec::new("ship production system", temp.path().display().to_string());

    let summary = runner.run_sequential(&spec, &client).unwrap();

    let expected_models = default_agent_matrix()
        .into_iter()
        .map(|agent| agent.model)
        .collect::<Vec<_>>();
    let requested_models = client
        .requests
        .borrow()
        .iter()
        .map(|request| request.model.clone())
        .collect::<Vec<_>>();
    assert_eq!(requested_models, expected_models);

    let store = SessionStore::new(
        temp.path().join(".gorsee-code"),
        gorsee_code_safety::Redactor::default(),
    );
    let events = store.read_events(&summary.session_id).unwrap();
    let started_agents = events
        .iter()
        .filter(|event| event.kind == EventKind::AgentStarted)
        .filter_map(|event| event.agent_id.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        started_agents,
        ["architect", "scout", "coder", "validator", "summarizer"]
    );
}

struct MockClient {
    replies: RefCell<Vec<String>>,
    requests: RefCell<Vec<ChatRequest>>,
}

impl MockClient {
    fn new(replies: Vec<String>) -> Self {
        Self {
            replies: RefCell::new(replies.into_iter().rev().collect()),
            requests: RefCell::new(Vec::new()),
        }
    }
}

impl ChatClient for MockClient {
    fn complete(
        &self,
        request: &ChatRequest,
    ) -> Result<ChatResponse, gorsee_code_agent::AgentRunError> {
        self.requests.borrow_mut().push(request.clone());
        let content = self.replies.borrow_mut().pop().unwrap();
        Ok(ChatResponse {
            id: Some("mock".into()),
            object: Some("chat.completion".into()),
            choices: Some(vec![json!({ "message": { "content": content } })]),
            usage: None,
        })
    }
}
