use std::{cell::RefCell, fs, path::Path};

use gorsee_code_agent::{ChatClient, TaskRunner};
use gorsee_code_core::{default_agent_matrix, EventKind, TaskSpec};
use gorsee_code_neurogate::{ChatRequest, ChatResponse};
use gorsee_code_session::SessionStore;
use serde_json::{json, Value};

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
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("inspect repository", temp.path().display().to_string());

    let summary = runner.run_sequential(&spec, &client).unwrap();

    assert_eq!(client.requests.borrow().len(), 6);
    assert_eq!(client.requests.borrow()[0].model, "glm-5.1");
    assert_eq!(client.requests.borrow()[1].model, "glm-5.1");
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
        .any(|event| event.kind == EventKind::SessionFinished));
    assert_session_artifacts(&summary.artifacts);
}

fn final_answer(answer: &str) -> String {
    json!({
        "message": answer,
        "final_answer": answer
    })
    .to_string()
}

#[test]
fn final_answer_without_message_is_recorded_as_visible_agent_message() {
    let temp = tempfile::tempdir().unwrap();
    let agents = default_agent_matrix()
        .into_iter()
        .take(1)
        .collect::<Vec<_>>();
    let client = MockClient::new(vec![
        json!({ "final_answer": "Привет! Чем помочь?" }).to_string()
    ]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("привет", temp.path().display().to_string());

    let summary = runner
        .run_sequential_with_agents(&spec, &client, agents)
        .unwrap();

    let store = SessionStore::new(
        temp.path().join(".gorsee-code"),
        gorsee_code_safety::Redactor::default(),
    );
    let events = store.read_events(&summary.session_id).unwrap();
    assert!(events.iter().any(|event| {
        event.kind == EventKind::AgentMessage && event.payload["message"] == "Привет! Чем помочь?"
    }));
}

#[test]
fn final_answer_is_the_only_visible_agent_message() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let agents = default_agent_matrix()
        .into_iter()
        .take(1)
        .collect::<Vec<_>>();
    let client = MockClient::new(vec![
        json!({
            "message": "технический ход: смотрю файлы",
            "tool_calls": [{ "name": "list_files", "args": {} }]
        })
        .to_string(),
        json!({
            "message": "технический ход: готовлю ответ",
            "final_answer": "Готово, отвечаю нормально."
        })
        .to_string(),
    ]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("проверь проект", temp.path().display().to_string());

    let summary = runner
        .run_sequential_with_agents(&spec, &client, agents)
        .unwrap();

    let store = SessionStore::new(
        temp.path().join(".gorsee-code"),
        gorsee_code_safety::Redactor::default(),
    );
    let messages = store
        .read_events(&summary.session_id)
        .unwrap()
        .into_iter()
        .filter(|event| event.kind == EventKind::AgentMessage)
        .map(|event| event.payload["message"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(messages, ["Готово, отвечаю нормально."]);
}

#[test]
fn agent_without_tools_receives_no_tool_manifests() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let mut agent = default_agent_matrix().remove(0);
    agent.tools.clear();
    let client = MockClient::new(vec![final_answer("Привет!")]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("Привет", temp.path().display().to_string());

    runner
        .run_sequential_with_agents(&spec, &client, vec![agent])
        .unwrap();

    let requests = client.requests.borrow();
    assert!(requests[0].messages[0]
        .content
        .contains("lightweight chat mode"));
    assert!(requests[0]
        .prompt_cache_key
        .as_deref()
        .is_some_and(|key| key.starts_with("gorsee:chat:v1:")));
    assert_eq!(requests[0].messages[1].content, "Привет");
    assert!(!requests[0].messages[1].content.contains("available_tools"));
}

#[test]
fn agent_prompt_only_includes_allowed_tool_manifests() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let agent = default_agent_matrix().remove(0);
    let client = MockClient::new(vec![final_answer("готово")]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("посмотри проект", temp.path().display().to_string());

    runner
        .run_sequential_with_agents(&spec, &client, vec![agent])
        .unwrap();

    let requests = client.requests.borrow();
    let prompt = user_prompt_json(&requests[0]);
    let names = available_tool_names(&prompt);
    assert!(names.contains(&"list_files".to_string()));
    assert!(names.contains(&"read_file".to_string()));
    assert!(names.contains(&"search_text".to_string()));
    assert!(names.contains(&"repo_map".to_string()));
    assert!(!names.contains(&"apply_patch".to_string()));
    assert!(!names.contains(&"run_test".to_string()));
}

#[test]
fn agent_prompt_uses_stable_cache_prefix_before_dynamic_work() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let agent = default_agent_matrix().remove(0);
    let client = MockClient::new(vec![final_answer("one"), final_answer("two")]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));

    runner
        .run_sequential_with_agents(
            &TaskSpec::new("первая задача", temp.path().display().to_string()),
            &client,
            vec![agent.clone()],
        )
        .unwrap();
    runner
        .run_sequential_with_agents(
            &TaskSpec::new("вторая задача", temp.path().display().to_string()),
            &client,
            vec![agent],
        )
        .unwrap();

    let requests = client.requests.borrow();
    assert_eq!(requests[0].messages.len(), 3);
    assert_eq!(requests[0].prompt_cache_key, requests[1].prompt_cache_key);
    assert!(requests[0]
        .prompt_cache_key
        .as_deref()
        .is_some_and(|key| key.starts_with("gorsee:agent:v1:")));

    let static_context = user_prompt_json(&requests[0]);
    let first_work = work_prompt_json(&requests[0]);
    let second_work = work_prompt_json(&requests[1]);

    assert!(static_context.get("available_tools").is_some());
    assert!(static_context.get("objective").is_none());
    assert_eq!(first_work["objective"], "первая задача");
    assert_eq!(second_work["objective"], "вторая задача");
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
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("ship production system", temp.path().display().to_string());

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

#[test]
fn sequential_runner_writes_production_session_artifacts() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let client = MockClient::new(
        default_agent_matrix()
            .iter()
            .map(|agent| final_answer(&format!("{} done", agent.id())))
            .collect(),
    );
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("write artifacts", temp.path().display().to_string());

    let summary = runner.run_sequential(&spec, &client).unwrap();

    assert_session_artifacts(&summary.artifacts);
}

#[test]
fn sequential_runner_records_model_usage_and_budget_warning() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let client = MockClient::with_replies(vec![
        MockReply::with_usage(
            final_answer("architect done"),
            json!({
                "prompt_tokens": 55,
                "completion_tokens": 25,
                "total_tokens": 80
            }),
        ),
        MockReply::content(final_answer("scout done")),
        MockReply::content(final_answer("coder done")),
        MockReply::content(final_answer("validator done")),
        MockReply::content(final_answer("summarizer done")),
    ]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let mut spec = TaskSpec::new("track model usage", temp.path().display().to_string());
    spec.budget_tokens = 100;

    let summary = runner.run_sequential(&spec, &client).unwrap();

    let store = SessionStore::new(
        temp.path().join(".gorsee-code"),
        gorsee_code_safety::Redactor::default(),
    );
    let manifest = store.read_manifest(&summary.session_id).unwrap();
    assert_eq!(manifest.budget.tokens_used, 80);

    let events = store.read_events(&summary.session_id).unwrap();
    let warning = events
        .iter()
        .find(|event| event.kind == EventKind::BudgetWarning)
        .expect("missing budget warning");
    assert_eq!(warning.payload["used_tokens"], 80);
    assert_eq!(warning.payload["limit_tokens"], 100);
    assert_eq!(
        warning.payload["hook_messages"][0],
        "budget warning: 80.0% used"
    );

    let usage = artifact_json(&summary.artifacts, "usage.json");
    assert_eq!(usage["tokens_used"], 80);

    let ledger_path = temp
        .path()
        .join(".gorsee-code/sessions")
        .join(&summary.session_id)
        .join("token-ledger.json");
    let ledger: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(ledger_path).unwrap()).unwrap();
    assert_eq!(ledger["records"][0]["agent_id"], "architect");
    assert_eq!(ledger["records"][0]["input_tokens"], 55);
    assert_eq!(ledger["records"][0]["output_tokens"], 25);
}

fn assert_session_artifacts(artifacts: &[gorsee_code_artifacts::ArtifactRecord]) {
    let names = artifact_names(artifacts);
    for expected in [
        "diff.patch",
        "events.jsonl",
        "limits-end.json",
        "limits-start.json",
        "manifest.json",
        "report.md",
        "usage.json",
    ] {
        assert!(names.contains(&expected.to_string()), "missing {expected}");
    }
    assert!(artifacts
        .iter()
        .all(|artifact| Path::new(&artifact.path).exists()));
}

fn artifact_names(artifacts: &[gorsee_code_artifacts::ArtifactRecord]) -> Vec<String> {
    artifacts
        .iter()
        .filter_map(|artifact| Path::new(&artifact.path).file_name())
        .filter_map(|name| name.to_str())
        .map(ToOwned::to_owned)
        .collect()
}

fn artifact_json(artifacts: &[gorsee_code_artifacts::ArtifactRecord], name: &str) -> Value {
    let artifact = artifacts
        .iter()
        .find(|artifact| {
            Path::new(&artifact.path)
                .file_name()
                .is_some_and(|file| file == name)
        })
        .unwrap_or_else(|| panic!("missing {name}"));
    serde_json::from_str(&fs::read_to_string(&artifact.path).unwrap()).unwrap()
}

fn user_prompt_json(request: &ChatRequest) -> Value {
    serde_json::from_str(&request.messages[1].content).unwrap()
}

fn work_prompt_json(request: &ChatRequest) -> Value {
    serde_json::from_str(&request.messages[2].content).unwrap()
}

fn available_tool_names(prompt: &Value) -> Vec<String> {
    prompt["available_tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_string())
        .collect()
}

#[derive(Debug, Clone)]
struct MockReply {
    content: String,
    usage: Option<Value>,
}

impl MockReply {
    fn content(content: String) -> Self {
        Self {
            content,
            usage: None,
        }
    }

    fn with_usage(content: String, usage: Value) -> Self {
        Self {
            content,
            usage: Some(usage),
        }
    }
}

struct MockClient {
    replies: RefCell<Vec<MockReply>>,
    requests: RefCell<Vec<ChatRequest>>,
}

impl MockClient {
    fn new(replies: Vec<String>) -> Self {
        Self::with_replies(replies.into_iter().map(MockReply::content).collect())
    }

    fn with_replies(replies: Vec<MockReply>) -> Self {
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
        let reply = self.replies.borrow_mut().pop().unwrap();
        Ok(ChatResponse {
            id: Some("mock".into()),
            object: Some("chat.completion".into()),
            choices: Some(vec![json!({ "message": { "content": reply.content } })]),
            usage: reply.usage,
        })
    }
}
