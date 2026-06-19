use std::{cell::RefCell, fs, path::Path};

use gorsee_code_agent::{AgentRunError, ChatClient, TaskRunner};
use gorsee_code_core::{EventKind, TaskSpec};
use gorsee_code_neurogate::{ChatRequest, ChatResponse};
use gorsee_code_safety::Redactor;
use gorsee_code_session::{ApprovalDecision, ApprovalStatus, SessionStore};
use serde_json::{json, Value};

#[test]
fn write_risk_tool_creates_pending_approval_and_waiting_manifest() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let client = MockClient::new(vec![json!({
        "message": "prepare patch",
        "tool_calls": [{
            "name": "propose_patch",
            "args": { "path": "src/lib.rs", "content": "pub fn ok() {}" }
        }]
    })
    .to_string()]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("change code", temp.path().display().to_string());

    let error = runner.run_sequential(&spec, &client).unwrap_err();

    let AgentRunError::WaitingApproval(approval_id) = error else {
        panic!("expected waiting approval, got {error:?}");
    };
    let session_id = only_session_id(temp.path());
    let store = SessionStore::new(temp.path().join(".gorsee-code"), Redactor::default());
    let approvals = store.pending_approvals(&session_id).unwrap();
    let events = store.read_events(&session_id).unwrap();
    let manifest = fs::read_to_string(
        temp.path()
            .join(".gorsee-code")
            .join("sessions")
            .join(&session_id)
            .join("manifest.json"),
    )
    .unwrap();

    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0].id, approval_id);
    assert_eq!(approvals[0].status, ApprovalStatus::Pending);
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::ToolRequested));
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::PatchProposed));
    assert!(manifest.contains("\"status\": \"waiting_approval\""));
}

#[test]
fn approval_events_redact_structured_tool_args_before_persisting() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let client = MockClient::new(vec![json!({
        "message": "prepare patch",
        "tool_calls": [{
            "name": "propose_patch",
            "args": {
                "path": "src/lib.rs",
                "content": "pub fn ok() {}",
                "password": "plain-secret",
                "headers": { "authorization": "Bearer abc.def" },
                "nested": { "api_key": "k123", "token": "t123" }
            }
        }]
    })
    .to_string()]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("change code", temp.path().display().to_string());

    let error = runner.run_sequential(&spec, &client).unwrap_err();
    assert!(matches!(error, AgentRunError::WaitingApproval(_)));

    let session_id = only_session_id(temp.path());
    let store = SessionStore::new(temp.path().join(".gorsee-code"), Redactor::default());
    let approvals = store.pending_approvals(&session_id).unwrap();
    let events = store.read_events(&session_id).unwrap();
    let persisted = serde_json::to_string(&(approvals, events)).unwrap();

    for secret in ["plain-secret", "Bearer abc.def", "k123", "t123"] {
        assert!(
            !persisted.contains(secret),
            "persisted payload leaked {secret}: {persisted}"
        );
    }
    assert!(persisted.contains("[REDACTED]"));
}

#[test]
fn approved_tool_resumes_and_finishes_the_interrupted_run() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let client = MockClient::new(vec![
        json!({
            "message": "write approved change",
            "tool_calls": [{
                "name": "apply_patch",
                "args": { "path": "src/lib.rs", "content": "pub fn shipped() -> bool { true }\n" }
            }]
        })
        .to_string(),
        final_answer("architect continued"),
        final_answer("scout done"),
        final_answer("coder done"),
        final_answer("validator done"),
        final_answer("summarizer done"),
    ]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("ship approved change", temp.path().display().to_string());

    let error = runner.run_sequential(&spec, &client).unwrap_err();
    let AgentRunError::WaitingApproval(approval_id) = error else {
        panic!("expected waiting approval, got {error:?}");
    };
    let session_id = only_session_id(temp.path());

    let summary = runner
        .resume_after_decision(
            &session_id,
            &approval_id,
            ApprovalDecision::Approved,
            &client,
        )
        .unwrap();

    let store = SessionStore::new(temp.path().join(".gorsee-code"), Redactor::default());
    let events = store.read_events(&session_id).unwrap();
    let manifest = store.read_manifest(&session_id).unwrap();
    let written = fs::read_to_string(temp.path().join("src/lib.rs")).unwrap();

    assert_eq!(summary.session_id, session_id);
    assert_eq!(manifest.status, "finished");
    assert!(written.contains("shipped"));
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::ToolApproved));
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::PatchApplied));
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::SessionFinished));
}

#[test]
fn approved_tool_resume_preserves_usage_from_pending_run() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let client = MockClient::with_replies(vec![
        MockReply::with_usage(
            json!({
                "message": "write approved change",
                "tool_calls": [{
                    "name": "apply_patch",
                    "args": { "path": "src/lib.rs", "content": "pub fn shipped() -> bool { true }\n" }
                }]
            })
            .to_string(),
            json!({
                "prompt_tokens": 55,
                "completion_tokens": 25,
                "total_tokens": 80
            }),
        ),
        MockReply::content(final_answer("architect continued")),
        MockReply::content(final_answer("scout done")),
        MockReply::content(final_answer("coder done")),
        MockReply::content(final_answer("validator done")),
        MockReply::content(final_answer("summarizer done")),
    ]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let mut spec = TaskSpec::new("ship approved change", temp.path().display().to_string());
    spec.budget_tokens = 100;

    let error = runner.run_sequential(&spec, &client).unwrap_err();
    let AgentRunError::WaitingApproval(approval_id) = error else {
        panic!("expected waiting approval, got {error:?}");
    };
    let session_id = only_session_id(temp.path());

    let summary = runner
        .resume_after_decision(
            &session_id,
            &approval_id,
            ApprovalDecision::Approved,
            &client,
        )
        .unwrap();

    let store = SessionStore::new(temp.path().join(".gorsee-code"), Redactor::default());
    let manifest = store.read_manifest(&session_id).unwrap();
    assert_eq!(manifest.budget.tokens_used, 80);

    let events = store.read_events(&session_id).unwrap();
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::BudgetWarning));

    let usage = artifact_json(&summary.artifacts, "usage.json");
    assert_eq!(usage["tokens_used"], 80);
}

fn only_session_id(root: &std::path::Path) -> String {
    let mut sessions = fs::read_dir(root.join(".gorsee-code").join("sessions"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    sessions.sort();
    assert_eq!(sessions.len(), 1);
    sessions.pop().unwrap()
}

fn final_answer(answer: &str) -> String {
    json!({
        "message": answer,
        "final_answer": answer
    })
    .to_string()
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
}

impl MockClient {
    fn new(replies: Vec<String>) -> Self {
        Self::with_replies(replies.into_iter().map(MockReply::content).collect())
    }

    fn with_replies(replies: Vec<MockReply>) -> Self {
        Self {
            replies: RefCell::new(replies.into_iter().rev().collect()),
        }
    }
}

impl ChatClient for MockClient {
    fn complete(
        &self,
        _request: &ChatRequest,
    ) -> Result<ChatResponse, gorsee_code_agent::AgentRunError> {
        let reply = self.replies.borrow_mut().pop().unwrap();
        Ok(ChatResponse {
            id: Some("mock".into()),
            object: Some("chat.completion".into()),
            choices: Some(vec![json!({ "message": { "content": reply.content } })]),
            usage: reply.usage,
        })
    }
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
