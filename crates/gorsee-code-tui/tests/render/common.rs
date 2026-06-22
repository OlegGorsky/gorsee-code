#![allow(dead_code)]

use std::time::{SystemTime, UNIX_EPOCH};
pub(crate) use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
pub(crate) use gorsee_code_tui::{
    action_for_key, render_app, render_frame, render_workspace, AppIntent, AttachmentKind,
    CenterPanel, CompletionKind, FocusPane, KeyAction, WorkspaceApp,
};
pub(crate) use gorsee_code_ui_state::{approval_waiting, workspace_running, ToolCallView};
pub(crate) use ratatui::{backend::TestBackend, style::Color, Terminal};

pub(crate) fn assert_product_output(output: &str) {
    let lowered = output.to_lowercase();
    for forbidden in [
        word(&['f', 'o', 'u', 'n', 'd', 'a', 't', 'i', 'o', 'n']),
        word(&[
            'v', 'e', 'r', 't', 'i', 'c', 'a', 'l', ' ', 's', 'l', 'i', 'c', 'e',
        ]),
        word(&['f', 'i', 'x', 't', 'u', 'r', 'e']),
        word(&['s', 'c', 'a', 'f', 'f', 'o', 'l', 'd']),
        word(&['m', 'v', 'p']),
        word(&['m', 'i', 'n', 'i', 'm', 'a', 'l']),
        word(&['d', 'e', 'm', 'o']),
        word(&['p', 'l', 'a', 'c', 'e', 'h', 'o', 'l', 'd', 'e', 'r']),
        word(&['m', 'i', 's', 's', 'i', 'o', 'n']),
    ] {
        assert!(
            !lowered.contains(&forbidden),
            "output contains forbidden product wording: {forbidden}\n{output}"
        );
    }
}

fn word(chars: &[char]) -> String {
    chars.iter().collect()
}

pub(crate) fn char_key(value: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(value), KeyModifiers::NONE)
}

pub(crate) fn left_click(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

pub(crate) fn left_drag(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

pub(crate) fn left_release(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

pub(crate) fn scroll_down(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

pub(crate) fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
    let area = buffer.area;
    let mut output = String::new();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            output.push_str(buffer[(x, y)].symbol());
        }
        output.push('\n');
    }
    output
}

pub(crate) fn right_panel_text(buffer: &ratatui::buffer::Buffer) -> String {
    let area = buffer.area;
    let start = area.right().saturating_sub(32);
    let main_bottom = area.bottom().saturating_sub(6);
    let mut output = String::new();
    for y in area.top()..main_bottom {
        for x in start..area.right() {
            output.push_str(buffer[(x, y)].symbol());
        }
        output.push('\n');
    }
    output
}

pub(crate) fn buffer_has_bg(buffer: &ratatui::buffer::Buffer, bg: Color) -> bool {
    buffer.content().iter().any(|cell| cell.bg == bg)
}

pub(crate) fn submit_line(
    app: &mut WorkspaceApp,
    line: &str,
    state: &gorsee_code_ui_state::WorkspaceState,
) -> AppIntent {
    for value in line.chars() {
        assert_eq!(
            app.handle_action(KeyAction::Insert(value), state),
            AppIntent::None
        );
    }
    app.handle_action(KeyAction::Submit, state)
}

pub(crate) struct TempProject {
    path: PathBuf,
}

impl TempProject {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(crate) fn temp_project() -> TempProject {
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("gorsee-code-tui-test-{id}"));
    fs::create_dir_all(root.join("src/ui")).expect("create src");
    fs::create_dir_all(root.join("target")).expect("create ignored dir");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main");
    fs::write(root.join("src/ui/widgets.rs"), "pub fn widget() {}\n").expect("write widget");
    fs::write(root.join("README.md"), "# Gorsee\n").expect("write readme");
    fs::write(root.join("target/build.log"), "ignored\n").expect("write ignored");
    TempProject { path: root }
}

pub(crate) fn write_session(root: &Path, id: &str, started_at: &str, status: &str) {
    let session = root.join(".gorsee-code/sessions").join(id);
    fs::create_dir_all(&session).expect("create session");
    fs::write(
        session.join("manifest.json"),
        format!(
            r#"{{
  "id": "{id}",
  "repo": "{}",
  "branch": "main",
  "started_at": "{started_at}",
  "status": "{status}",
  "agents": ["architect"],
  "budget": {{"tokens_limit": 80000, "tokens_used": 0}}
}}"#,
            root.display()
        ),
    )
    .expect("write manifest");
    fs::write(session.join("events.jsonl"), "").expect("write events");
    fs::write(session.join("approvals.jsonl"), "").expect("write approvals");
}
