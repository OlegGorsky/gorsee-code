use super::*;
use std::{fs, path::Path};

#[test]
fn empty_workspace_is_ready() {
    let temp = tempfile::tempdir().unwrap();
    let state = workspace_state(temp.path());

    assert_eq!(state.session.title, "Gorsee Code Workspace");
    assert_eq!(state.session.status, "ready");
    assert_eq!(state.budget.used_tokens, 0);
    assert!(state.agents.is_empty());
    assert!(state.timeline.is_empty());
}

#[test]
fn default_workspace_state_does_not_auto_open_latest_session() {
    let temp = tempfile::tempdir().unwrap();
    write_session(temp.path(), "latest", "2026-06-20T01:00:00Z", "finished");

    let state = workspace_state(temp.path());

    assert_eq!(state.session.id, "workspace");
    assert_eq!(state.session.status, "ready");
    assert!(state.timeline.is_empty());
}

#[test]
fn default_workspace_state_opens_latest_ready_session() {
    let temp = tempfile::tempdir().unwrap();
    write_session(temp.path(), "ready-chat", "2026-06-20T01:00:00Z", "ready");

    let state = workspace_state(temp.path());

    assert_eq!(state.session.id, "ready-chat");
    assert_eq!(state.session.status, "ready");
}

#[test]
fn workspace_state_can_load_requested_session() {
    let temp = tempfile::tempdir().unwrap();
    write_session(temp.path(), "older", "2026-06-20T00:00:00Z", "finished");
    write_session(temp.path(), "newer", "2026-06-20T01:00:00Z", "running");

    let state = workspace_state_for_session(temp.path(), Some("older"));

    assert_eq!(state.session.id, "older");
    assert_eq!(state.session.status, "finished");
}

#[test]
fn workspace_state_uses_project_configured_agent_models() {
    let temp = tempfile::tempdir().unwrap();
    write_session(temp.path(), "configured", "2026-06-20T01:00:00Z", "running");
    let mut config = gorsee_code_config::default_config("configured");
    config.agents.get_mut("architect").unwrap().model = "kimi-k2.6".into();
    config.save(temp.path().join("gorsee-code.toml")).unwrap();

    let state = workspace_state_for_session(temp.path(), Some("configured"));

    let architect = state
        .agents
        .iter()
        .find(|agent| agent.id == "architect")
        .unwrap();
    assert_eq!(architect.model, "kimi-k2.6");
}

#[test]
fn workspace_state_shows_only_session_manifest_agents() {
    let temp = tempfile::tempdir().unwrap();
    write_session(temp.path(), "single", "2026-06-20T01:00:00Z", "running");

    let state = workspace_state_for_session(temp.path(), Some("single"));

    assert_eq!(
        state
            .agents
            .iter()
            .map(|agent| agent.id.as_str())
            .collect::<Vec<_>>(),
        ["architect"]
    );
}

#[test]
fn workspace_state_uses_token_ledger_by_agent() {
    let temp = tempfile::tempdir().unwrap();
    write_session_with_agents(
        temp.path(),
        "ledger",
        "2026-06-20T01:00:00Z",
        "finished",
        &["architect", "coder"],
    );
    fs::write(
        temp.path()
            .join(".gorsee-code/sessions/ledger/token-ledger.json"),
        serde_json::json!({
            "records": [
                {
                    "agent_id": "architect",
                    "phase": "agent",
                    "model": "glm-5.1",
                    "input_tokens": 10,
                    "output_tokens": 20,
                    "cached_tokens": 5,
                    "reasoning_tokens": 15,
                    "estimated": false,
                    "credit_multiplier": 1.0
                },
                {
                    "agent_id": "coder",
                    "phase": "agent",
                    "model": "mimo-v2.5",
                    "input_tokens": 7,
                    "output_tokens": 3,
                    "cached_tokens": 0,
                    "reasoning_tokens": 0,
                    "estimated": false,
                    "credit_multiplier": 1.0
                }
            ]
        })
        .to_string(),
    )
    .unwrap();

    let state = workspace_state_for_session(temp.path(), Some("ledger"));

    assert_eq!(state.budget.used_tokens, 55);
    assert_eq!(state.budget.cached_tokens, 5);
    let architect = state
        .agents
        .iter()
        .find(|agent| agent.id == "architect")
        .unwrap();
    let coder = state
        .agents
        .iter()
        .find(|agent| agent.id == "coder")
        .unwrap();
    assert_eq!(architect.tokens_used, 45);
    assert_eq!(architect.cached_tokens, 5);
    assert_eq!(coder.tokens_used, 10);
    assert_eq!(coder.cached_tokens, 0);
}

#[test]
fn timeline_hides_internal_events_and_labels_user_prompt() {
    let temp = tempfile::tempdir().unwrap();
    write_session(temp.path(), "chat", "2026-06-20T01:00:00Z", "running");
    let session = temp.path().join(".gorsee-code/sessions/chat");
    fs::write(
        session.join("events.jsonl"),
        [
            event_json(1, "session_started", None, r#"{"objective":"Привет"}"#),
            event_json(
                2,
                "agent_thinking",
                Some("architect"),
                r#"{"message":"думаю"}"#,
            ),
            event_json(
                3,
                "agent_message",
                Some("architect"),
                r#"{"message":"Привет! Чем помочь?"}"#,
            ),
            event_json(4, "tool_started", Some("architect"), r#"{"name":"read"}"#),
            event_json(
                5,
                "tool_requested",
                Some("architect"),
                r#"{"name":"read_file"}"#,
            ),
            event_json(
                6,
                "tool_finished",
                Some("architect"),
                r#"{"name":"read_file","output":"done"}"#,
            ),
            event_json(
                7,
                "context_updated",
                Some("architect"),
                r#"{"answers":1,"tool_results":1}"#,
            ),
            event_json(
                8,
                "artifact_created",
                None,
                r#"{"artifact":{"name":"report.md"}}"#,
            ),
            event_json(9, "session_finished", None, r#"{"status":"finished"}"#),
        ]
        .join("\n"),
    )
    .unwrap();

    let state = workspace_state_for_session(temp.path(), Some("chat"));

    assert_eq!(state.timeline.len(), 2);
    assert_eq!(state.timeline[0].kind, "user");
    assert_eq!(state.timeline[0].agent_id.as_deref(), Some("Вы"));
    assert_eq!(state.timeline[0].summary, "Привет");
    assert_eq!(state.timeline[1].kind, "assistant");
    assert_eq!(state.timeline[1].summary, "Привет! Чем помочь?");
}

#[test]
fn timeline_surfaces_structured_diff_and_verification_events() {
    let temp = tempfile::tempdir().unwrap();
    write_session(temp.path(), "chat", "2026-06-20T01:00:00Z", "ready");
    let session = temp.path().join(".gorsee-code/sessions/chat");
    fs::write(
        session.join("events.jsonl"),
        [
            event_json(
                1,
                "turn_started",
                None,
                r#"{"objective":"измени файл"}"#,
            ),
            event_json(
                2,
                "diff_ready",
                None,
                r#"{"summary":"diff готов: 1 файлов, +2 -0","artifact":"diff.json"}"#,
            ),
            event_json(
                3,
                "test_finished",
                None,
                r#"{"summary":"проверки пройдены: cargo test --workspace --quiet","artifact":"verification.json"}"#,
            ),
            event_json(
                4,
                "agent_message",
                Some("validator"),
                r#"{"message":"Готово: файл изменен, проверки прошли."}"#,
            ),
            event_json(5, "turn_finished", None, r#"{"status":"ready"}"#),
        ]
        .join("\n"),
    )
    .unwrap();

    let state = workspace_state_for_session(temp.path(), Some("chat"));

    assert_eq!(
        state
            .timeline
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>(),
        ["user", "patch", "verification", "assistant"]
    );
    assert_eq!(state.timeline[1].summary, "diff готов: 1 файлов, +2 -0");
    assert!(state.timeline[2].summary.contains("проверки пройдены"));
}

#[test]
fn timeline_does_not_show_workspace_ready_bootstrap() {
    let temp = tempfile::tempdir().unwrap();
    write_session(temp.path(), "chat", "2026-06-20T01:00:00Z", "running");
    let session = temp.path().join(".gorsee-code/sessions/chat");
    fs::write(
        session.join("events.jsonl"),
        event_json(
            1,
            "session_started",
            None,
            r#"{"objective":"workspace_ready"}"#,
        ),
    )
    .unwrap();

    let state = workspace_state_for_session(temp.path(), Some("chat"));

    assert!(state.timeline.is_empty());
}

fn write_session(root: &Path, id: &str, started_at: &str, status: &str) {
    write_session_with_agents(root, id, started_at, status, &["architect"]);
}

fn write_session_with_agents(
    root: &Path,
    id: &str,
    started_at: &str,
    status: &str,
    agents: &[&str],
) {
    let session = root.join(".gorsee-code/sessions").join(id);
    fs::create_dir_all(&session).unwrap();
    let agents = agents
        .iter()
        .map(|agent| format!(r#""{agent}""#))
        .collect::<Vec<_>>()
        .join(", ");
    fs::write(
        session.join("manifest.json"),
        format!(
            r#"{{
  "id": "{id}",
  "repo": "{}",
  "branch": "main",
  "started_at": "{started_at}",
  "status": "{status}",
  "agents": [{agents}],
  "budget": {{"tokens_limit": 80000, "tokens_used": 0}}
}}"#,
            root.display()
        ),
    )
    .unwrap();
    fs::write(session.join("events.jsonl"), "").unwrap();
    fs::write(session.join("approvals.jsonl"), "").unwrap();
}

fn event_json(sequence: u64, kind: &str, agent_id: Option<&str>, payload: &str) -> String {
    serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "session_id": "chat",
        "sequence": sequence,
        "timestamp": "2026-06-20T01:00:00Z",
        "kind": kind,
        "agent_id": agent_id,
        "payload": serde_json::from_str::<serde_json::Value>(payload).unwrap()
    })
    .to_string()
}
