use std::{cell::RefCell, fs, path::Path};

use gorsee_code_agent::{ChatClient, TaskRunner};
use gorsee_code_core::{default_agent_matrix, EventKind, TaskSpec};
use gorsee_code_neurogate::{ChatRequest, ChatResponse};
use gorsee_code_session::SessionStore;
use serde_json::{json, Value};

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
    let spec = TaskSpec::new("inspect artifacts", temp.path().display().to_string());

    let summary = runner.run_sequential(&spec, &client).unwrap();

    assert_session_artifacts(&summary.artifacts);
    let contract = artifact_json(&summary.artifacts, "execution-contract.json");
    assert_eq!(contract["requires_plan"], true);
    assert_eq!(contract["final_answer"]["raw_output_in_details_only"], true);
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

#[test]
fn turn_runner_appends_events_and_usage_to_existing_session() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let agent = default_agent_matrix().remove(0);
    let client = MockClient::with_replies(vec![
        MockReply::with_usage(
            final_answer("first"),
            json!({"prompt_tokens": 10, "completion_tokens": 5}),
        ),
        MockReply::with_usage(
            final_answer("second"),
            json!({"prompt_tokens": 20, "completion_tokens": 7}),
        ),
    ]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let first = TaskSpec::new("первый turn", temp.path().display().to_string());
    let summary = runner
        .run_sequential_with_agents(&first, &client, vec![agent.clone()])
        .unwrap();
    let store = SessionStore::new(
        temp.path().join(".gorsee-code"),
        gorsee_code_safety::Redactor::default(),
    );
    let mut manifest = store.read_manifest(&summary.session_id).unwrap();
    manifest.status = "ready".into();
    store.write_manifest(&manifest).unwrap();

    let second = TaskSpec::new("второй turn", temp.path().display().to_string());
    let turn_summary = runner
        .run_turn_with_agents(&summary.session_id, &second, &client, vec![agent])
        .unwrap();

    assert_eq!(turn_summary.session_id, summary.session_id);
    let manifest = store.read_manifest(&summary.session_id).unwrap();
    assert_eq!(manifest.status, "ready");
    assert_eq!(manifest.budget.tokens_used, 42);

    let events = store.read_events(&summary.session_id).unwrap();
    let user_prompts = events
        .iter()
        .filter(|event| event.kind == EventKind::TurnStarted)
        .map(|event| event.payload["objective"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(user_prompts, ["первый turn", "второй turn"]);
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == EventKind::SessionFinished)
            .count(),
        0
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == EventKind::TurnFinished)
            .count(),
        2
    );

    let ledger_path = temp
        .path()
        .join(".gorsee-code/sessions")
        .join(&summary.session_id)
        .join("token-ledger.json");
    let ledger: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(ledger_path).unwrap()).unwrap();
    assert_eq!(ledger["records"].as_array().unwrap().len(), 2);
}

fn final_answer(answer: &str) -> String {
    json!({
        "message": answer,
        "final_answer": answer
    })
    .to_string()
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
        let reply = self
            .replies
            .borrow_mut()
            .pop()
            .expect("mock replies exhausted");
        Ok(ChatResponse {
            id: Some("mock".into()),
            object: Some("chat.completion".into()),
            choices: Some(vec![json!({ "message": { "content": reply.content } })]),
            usage: reply.usage,
        })
    }
}
