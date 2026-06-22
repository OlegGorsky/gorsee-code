use std::{cell::RefCell, fs, path::Path};

use gorsee_code_agent::{ChatClient, TaskRunner};
use gorsee_code_core::{default_agent_matrix, AgentRole, EventKind, TaskSpec};
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
        .any(|event| event.kind == EventKind::TurnFinished));
    let manifest = store.read_manifest(&summary.session_id).unwrap();
    assert_eq!(manifest.status, "ready");
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
fn coder_prompt_requires_file_tools_for_file_changes() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let agent = default_agent_matrix()
        .into_iter()
        .find(|agent| agent.role == AgentRole::Coder)
        .unwrap();
    let client = MockClient::new(vec![json!({
        "message": "готовлю patch",
        "tool_calls": [{
            "name": "apply_patch",
            "args": { "path": "src/lib.rs", "content": "pub fn changed() -> bool { true }\n" }
        }]
    })
    .to_string()]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("измени файл", temp.path().display().to_string());

    assert!(matches!(
        runner
            .run_sequential_with_agents(&spec, &client, vec![agent])
            .unwrap_err(),
        gorsee_code_agent::AgentRunError::WaitingApproval(_)
    ));

    let requests = client.requests.borrow();
    let prompt = user_prompt_json(&requests[0]);
    assert_eq!(
        prompt["execution_policy"]["must_use_tools_for_file_changes"],
        true
    );
    assert_eq!(prompt["turn_plan"]["intent"], "edit");
    assert_eq!(prompt["execution_contract"]["requires_plan"], true);
    assert_eq!(prompt["execution_contract"]["diff_required"], true);
    assert_eq!(
        prompt["execution_contract"]["final_answer"]["forbid_full_code_dump"],
        true
    );
    let names = available_tool_names(&prompt);
    assert!(names.contains(&"propose_patch".to_string()));
    assert!(names.contains(&"apply_patch".to_string()));
    let required_tools = prompt["execution_contract"]["required_tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(required_tools.contains(&"apply_patch".to_string()));
    assert!(required_tools.contains(&"git_diff".to_string()));
    assert!(required_tools.contains(&"run_test".to_string()));
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
    assert_eq!(requests[0].prompt_cache_retention.as_deref(), Some("24h"));
    assert!(requests[0]
        .prompt_cache_key
        .as_deref()
        .is_some_and(|key| key.starts_with("gorsee:agent:v1:")));

    let static_context = user_prompt_json(&requests[0]);
    let first_work = work_prompt_json(&requests[0]);
    let second_work = work_prompt_json(&requests[1]);

    assert!(static_context.get("available_tools").is_some());
    assert_eq!(
        static_context["execution_policy"]["must_use_tools_for_file_changes"],
        false
    );
    assert!(static_context.get("objective").is_none());
    assert_eq!(first_work["objective"], "первая задача");
    assert_eq!(second_work["objective"], "вторая задача");
}

fn assert_session_artifacts(artifacts: &[gorsee_code_artifacts::ArtifactRecord]) {
    let names = artifact_names(artifacts);
    for expected in [
        "diff.json",
        "events.jsonl",
        "execution-contract.json",
        "limits-end.json",
        "limits-start.json",
        "manifest.json",
        "plan.json",
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
