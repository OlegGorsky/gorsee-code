mod support;

use std::fs;

use gorsee_code_agent::{AgentRunError, TaskRunner};
use gorsee_code_core::{AgentRole, EventKind, TaskSpec};
use gorsee_code_safety::{Redactor, RiskClass};
use gorsee_code_session::{ApprovalDecision, ApprovalStatus, SessionStore};
use serde_json::json;
use support::{
    agent_by_role, artifact_json, final_answer, init_git, only_session_id, MockClient, MockReply,
};

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
            "name": "apply_patch",
            "args": { "path": "src/lib.rs", "content": "pub fn ok() {}" }
        }]
    })
    .to_string()]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("change code", temp.path().display().to_string());

    let error = runner
        .run_sequential_with_agents(&spec, &client, vec![agent_by_role(AgentRole::Coder)])
        .unwrap_err();

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
                "name": "apply_patch",
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

    let error = runner
        .run_sequential_with_agents(&spec, &client, vec![agent_by_role(AgentRole::Coder)])
        .unwrap_err();
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
    init_git(temp.path());
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

    let error = runner
        .run_sequential_with_agents(&spec, &client, vec![agent_by_role(AgentRole::Coder)])
        .unwrap_err();
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
    assert_eq!(manifest.status, "ready");
    assert!(written.contains("shipped"));
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::ToolApproved));
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::PatchApplied));
    assert!(events.iter().any(|event| {
        event.kind == EventKind::DiffReady
            && event.payload["summary"]
                .as_str()
                .is_some_and(|summary| summary.contains("diff готов"))
            && event.payload["artifact"] == "diff.json"
    }));
    assert!(!events
        .iter()
        .any(|event| event.kind == EventKind::TestFinished));
    let verification = artifact_json(&summary.artifacts, "verification.json");
    assert_eq!(verification["status"], "skipped");
    assert_eq!(verification["reason"], "verification tool was not run");
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::TurnFinished));
    assert!(!events
        .iter()
        .any(|event| event.kind == EventKind::SessionFinished));
}

#[test]
fn run_test_requires_command_approval() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let validator = agent_by_role(AgentRole::Validator);
    let client = MockClient::new(vec![json!({
        "message": "запускаю проверки",
        "tool_calls": [{
            "name": "run_test",
            "args": { "command": ["cargo", "test", "--workspace"] }
        }]
    })
    .to_string()]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("запусти тесты", temp.path().display().to_string());

    let error = runner
        .run_sequential_with_agents(&spec, &client, vec![validator])
        .unwrap_err();

    let AgentRunError::WaitingApproval(approval_id) = error else {
        panic!("expected waiting approval, got {error:?}");
    };
    let session_id = only_session_id(temp.path());
    let store = SessionStore::new(temp.path().join(".gorsee-code"), Redactor::default());
    let approvals = store.pending_approvals(&session_id).unwrap();

    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0].id, approval_id);
    assert_eq!(approvals[0].tool_name, "run_test");
    assert_eq!(approvals[0].risk, RiskClass::Command);
    assert_eq!(approvals[0].status, ApprovalStatus::Pending);
}

#[test]
fn empty_auto_run_test_without_detected_command_skips_without_approval() {
    let temp = tempfile::tempdir().unwrap();
    let validator = agent_by_role(AgentRole::Validator);
    let client = MockClient::new(vec![
        json!({
            "message": "ищу проверки",
            "tool_calls": [{
                "name": "run_test",
                "args": {}
            }]
        })
        .to_string(),
        final_answer("Проверки пропущены: подходящая команда не найдена."),
    ]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("запусти проверки", temp.path().display().to_string());

    let summary = runner
        .run_sequential_with_agents(&spec, &client, vec![validator])
        .unwrap();

    let session_id = only_session_id(temp.path());
    let store = SessionStore::new(temp.path().join(".gorsee-code"), Redactor::default());
    let approvals = store.pending_approvals(&session_id).unwrap();
    let verification = artifact_json(&summary.artifacts, "verification.json");

    assert!(approvals.is_empty());
    assert_eq!(verification["status"], "skipped");
    assert_eq!(
        verification["output"],
        "no supported verification command detected"
    );
}

#[test]
fn unsupported_run_test_command_skips_without_approval() {
    let temp = tempfile::tempdir().unwrap();
    let validator = agent_by_role(AgentRole::Validator);
    let client = MockClient::new(vec![
        json!({
            "message": "проверяю файл",
            "tool_calls": [{
                "name": "run_test",
                "args": { "command": "cat hello.txt" }
            }]
        })
        .to_string(),
        final_answer("Команда не запускалась: cat не является тестовой командой."),
    ]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("посмотри проект", temp.path().display().to_string());

    runner
        .run_sequential_with_agents(&spec, &client, vec![validator])
        .unwrap();

    let session_id = only_session_id(temp.path());
    let store = SessionStore::new(temp.path().join(".gorsee-code"), Redactor::default());
    let approvals = store.pending_approvals(&session_id).unwrap();
    let events = store.read_events(&session_id).unwrap();

    assert!(approvals.is_empty());
    assert!(events.iter().any(|event| {
        event.kind == EventKind::ToolFinished
            && event.payload["name"] == "run_test"
            && event.payload["status"] == "skipped"
            && event.payload["output"]
                .as_str()
                .is_some_and(|output| output.contains("cat"))
    }));
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

    let error = runner
        .run_sequential_with_agents(&spec, &client, vec![agent_by_role(AgentRole::Coder)])
        .unwrap_err();
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
