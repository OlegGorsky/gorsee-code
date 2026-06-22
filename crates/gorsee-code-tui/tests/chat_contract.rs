use std::{fs, path::Path};

use gorsee_code_tui::{render_frame, WorkspaceApp};
use gorsee_code_ui_state::workspace_state_for_session;
use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

#[test]
fn rendered_timeline_shows_chat_messages_without_internal_events() {
    let temp = tempfile::tempdir().unwrap();
    write_session(temp.path());

    let state = workspace_state_for_session(temp.path(), Some("chat"));
    assert_eq!(
        state
            .timeline
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>(),
        ["user", "assistant"]
    );

    let mut terminal = Terminal::new(TestBackend::new(140, 42)).expect("terminal");
    let app = WorkspaceApp::new();
    terminal
        .draw(|frame| render_frame(frame, &state, &app))
        .expect("render");
    let screen = buffer_text(terminal.backend().buffer());

    assert!(!screen.contains("#0001"));
    assert!(!screen.contains("#0003"));
    assert!(screen.contains("Вы"));
    assert!(screen.contains("привет"));
    assert!(screen.contains("Architect"));
    assert!(screen.contains("Здравствуйте"));
    for hidden in [
        "Процесс",
        "начал работу",
        "tool requested",
        "tool finished",
        "артефакт создан",
        "сессия завершена",
        "готово",
    ] {
        assert!(
            !screen.contains(hidden),
            "internal timeline text leaked: {hidden}\n{screen}"
        );
    }
}

fn write_session(root: &Path) {
    let session = root.join(".gorsee-code/sessions/chat");
    fs::create_dir_all(&session).unwrap();
    fs::write(
        session.join("manifest.json"),
        format!(
            r#"{{
  "id": "chat",
  "repo": "{}",
  "branch": "main",
  "started_at": "2026-06-20T01:00:00Z",
  "status": "running",
  "agents": ["architect"],
  "budget": {{"tokens_limit": 80000, "tokens_used": 546}}
}}"#,
            root.display()
        ),
    )
    .unwrap();
    fs::write(
        session.join("events.jsonl"),
        [
            event_json(1, "session_started", None, r#"{"objective":"привет"}"#),
            event_json(
                2,
                "agent_started",
                Some("architect"),
                r#"{"model":"glm-5.1"}"#,
            ),
            event_json(
                3,
                "agent_message",
                Some("architect"),
                r#"{"message":"Здравствуйте"}"#,
            ),
            event_json(
                4,
                "tool_requested",
                Some("architect"),
                r#"{"name":"read_file"}"#,
            ),
            event_json(
                5,
                "tool_finished",
                Some("architect"),
                r#"{"name":"read_file","output":"done"}"#,
            ),
            event_json(
                6,
                "artifact_created",
                None,
                r#"{"artifact":{"name":"README.md"}}"#,
            ),
            event_json(7, "session_finished", None, r#"{"status":"finished"}"#),
        ]
        .join("\n"),
    )
    .unwrap();
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

fn buffer_text(buffer: &Buffer) -> String {
    let mut text = String::new();
    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        text.push_str(line.trim_end());
        text.push('\n');
    }
    text
}
