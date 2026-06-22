use std::{cell::RefCell, fs, path::Path};

use gorsee_code_agent::{ChatClient, TaskRunner};
use gorsee_code_core::{default_agent_matrix, AgentRole, EventKind, TaskSpec};
use gorsee_code_neurogate::{ChatRequest, ChatResponse};
use gorsee_code_safety::Redactor;
use gorsee_code_session::{ApprovalDecision, ApprovalStatus, SessionStore};
use serde_json::{json, Value};

#[test]
fn agent_cannot_execute_tool_outside_profile_contract() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let architect = default_agent_matrix()
        .into_iter()
        .find(|agent| agent.role == AgentRole::Architect)
        .unwrap();
    let client = MockClient::new(vec![
        json!({
            "message": "пытаюсь изменить файл",
            "tool_calls": [{
                "name": "apply_patch",
                "args": { "path": "src/lib.rs", "content": "pub fn forbidden() {}\n" }
            }]
        })
        .to_string(),
        json!({
            "message": "принял отказ runtime",
            "final_answer": "Не могу менять файлы из роли Architect."
        })
        .to_string(),
    ]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("посмотри проект", temp.path().display().to_string());

    let summary = runner
        .run_sequential_with_agents(&spec, &client, vec![architect])
        .unwrap();

    let store = SessionStore::new(temp.path().join(".gorsee-code"), Redactor::default());
    let approvals = store.pending_approvals(&summary.session_id).unwrap();
    let events = store.read_events(&summary.session_id).unwrap();

    assert!(approvals.is_empty());
    assert!(!temp.path().join("src/lib.rs").exists());
    assert!(!events
        .iter()
        .any(|event| event.kind == EventKind::ToolRequested));
    assert!(events.iter().any(|event| {
        event.kind == EventKind::Error
            && event.payload["name"] == "apply_patch"
            && event.payload["error"]
                .as_str()
                .is_some_and(|error| error.contains("tool not allowed for agent"))
    }));
}

#[test]
fn existing_file_write_requires_prior_file_context() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    fs::write(temp.path().join("src/lib.rs"), "pub fn current() {}\n").unwrap();
    let coder = default_agent_matrix()
        .into_iter()
        .find(|agent| agent.role == AgentRole::Coder)
        .unwrap();
    let client = MockClient::new(vec![
        json!({
            "message": "сразу меняю файл",
            "tool_calls": [{
                "name": "apply_patch",
                "args": { "path": "src/lib.rs", "content": "pub fn changed() {}\n" }
            }]
        })
        .to_string(),
        json!({
            "message": "сначала читаю файл",
            "tool_calls": [{
                "name": "read_file",
                "args": { "path": "src/lib.rs" }
            }]
        })
        .to_string(),
        json!({
            "message": "теперь меняю после чтения",
            "tool_calls": [{
                "name": "apply_patch",
                "args": { "path": "src/lib.rs", "content": "pub fn changed() {}\n" }
            }]
        })
        .to_string(),
    ]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("измени src/lib.rs", temp.path().display().to_string());

    let error = runner
        .run_sequential_with_agents(&spec, &client, vec![coder])
        .unwrap_err();

    let gorsee_code_agent::AgentRunError::WaitingApproval(approval_id) = error else {
        panic!("expected waiting approval, got {error:?}");
    };
    let store = SessionStore::new(temp.path().join(".gorsee-code"), Redactor::default());
    let approvals = store
        .pending_approvals(&only_session_id(temp.path()))
        .unwrap();

    assert_eq!(
        fs::read_to_string(temp.path().join("src/lib.rs")).unwrap(),
        "pub fn current() {}\n"
    );
    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0].id, approval_id);
    assert_eq!(approvals[0].tool_name, "apply_patch");
    assert_eq!(approvals[0].status, ApprovalStatus::Pending);
}

#[test]
fn denied_run_test_finishes_with_skipped_verification() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let validator = default_agent_matrix()
        .into_iter()
        .find(|agent| agent.role == AgentRole::Validator)
        .unwrap();
    let client = MockClient::new(vec![
        json!({
            "message": "запускаю проверки",
            "tool_calls": [{
                "name": "run_test",
                "args": { "command": ["cargo", "test", "--workspace"] }
            }]
        })
        .to_string(),
        json!({
            "message": "проверка пропущена",
            "final_answer": "Проверки не запускал: пользователь отклонил команду."
        })
        .to_string(),
    ]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("запусти тесты", temp.path().display().to_string());

    let error = runner
        .run_sequential_with_agents(&spec, &client, vec![validator])
        .unwrap_err();
    let gorsee_code_agent::AgentRunError::WaitingApproval(approval_id) = error else {
        panic!("expected waiting approval, got {error:?}");
    };
    let session_id = only_session_id(temp.path());

    let summary = runner
        .resume_after_decision(&session_id, &approval_id, ApprovalDecision::Denied, &client)
        .unwrap();

    let store = SessionStore::new(temp.path().join(".gorsee-code"), Redactor::default());
    let manifest = store.read_manifest(&session_id).unwrap();
    let events = store.read_events(&session_id).unwrap();
    let verification = artifact_json(&summary.artifacts, "verification.json");

    assert_eq!(manifest.status, "ready");
    assert_eq!(verification["status"], "skipped");
    assert_eq!(verification["source"], "approval");
    assert_eq!(verification["command"], "run_test");
    assert_eq!(verification["reason"], "denied by user");
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::ToolDenied));
    assert!(events.iter().any(|event| {
        event.kind == EventKind::TestFinished
            && event.payload["status"] == "skipped"
            && event.payload["artifact"] == "verification.json"
    }));
    assert!(!events
        .iter()
        .any(|event| event.kind == EventKind::SessionFinished));
}

#[test]
fn failed_approved_patch_does_not_emit_patch_applied() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let coder = default_agent_matrix()
        .into_iter()
        .find(|agent| agent.role == AgentRole::Coder)
        .unwrap();
    let client = MockClient::new(vec![
        json!({
            "message": "предлагаю недопустимый патч",
            "tool_calls": [{
                "name": "apply_patch",
                "args": { "path": "../outside.rs", "content": "pub fn outside() {}\n" }
            }]
        })
        .to_string(),
        json!({
            "message": "после ошибки предлагаю безопасный новый файл",
            "tool_calls": [{
                "name": "apply_patch",
                "args": { "path": "src/new.rs", "content": "pub fn inside() {}\n" }
            }]
        })
        .to_string(),
    ]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("создай файл", temp.path().display().to_string());

    let error = runner
        .run_sequential_with_agents(&spec, &client, vec![coder])
        .unwrap_err();
    let gorsee_code_agent::AgentRunError::WaitingApproval(approval_id) = error else {
        panic!("expected waiting approval, got {error:?}");
    };
    let session_id = only_session_id(temp.path());
    let second_error = runner
        .resume_after_decision(
            &session_id,
            &approval_id,
            ApprovalDecision::Approved,
            &client,
        )
        .unwrap_err();
    let gorsee_code_agent::AgentRunError::WaitingApproval(_) = second_error else {
        panic!("expected second waiting approval, got {second_error:?}");
    };

    let store = SessionStore::new(temp.path().join(".gorsee-code"), Redactor::default());
    let events = store.read_events(&session_id).unwrap();

    assert!(events
        .iter()
        .any(|event| { event.kind == EventKind::Error && event.payload["name"] == "apply_patch" }));
    assert!(!events
        .iter()
        .any(|event| event.kind == EventKind::PatchApplied));
}

struct MockClient {
    replies: RefCell<Vec<String>>,
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

impl MockClient {
    fn new(replies: Vec<String>) -> Self {
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
        let content = self.replies.borrow_mut().pop().unwrap();
        Ok(ChatResponse {
            id: Some("mock".into()),
            object: Some("chat.completion".into()),
            choices: Some(vec![json!({ "message": { "content": content } })]),
            usage: None::<Value>,
        })
    }
}
