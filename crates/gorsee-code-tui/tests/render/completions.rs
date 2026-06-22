use crate::common::*;

#[test]
fn app_offers_slash_commands_and_at_file_mentions() {
    let project = temp_project();
    let state = workspace_running();

    let mut commands = WorkspaceApp::new();
    commands
        .sync_project_root(project.path())
        .expect("scan project");
    commands.handle_action(KeyAction::Insert('/'), &state);

    assert_eq!(commands.completion_kind(), Some(CompletionKind::Commands));
    assert!(commands
        .completion_items()
        .iter()
        .any(|item| item.insert_text() == "/project"));

    commands.handle_action(KeyAction::AcceptCompletion, &state);
    assert_eq!(commands.input(), "/project");

    let mut files = WorkspaceApp::new();
    files
        .sync_project_root(project.path())
        .expect("scan project");
    files.handle_action(KeyAction::Insert('@'), &state);
    files.handle_action(KeyAction::Insert('s'), &state);

    assert_eq!(files.completion_kind(), Some(CompletionKind::Files));
    assert!(files
        .completion_items()
        .iter()
        .any(|item| item.insert_text() == "@src/main.rs"));

    files.handle_action(KeyAction::AcceptCompletion, &state);
    assert_eq!(files.input(), "@src/main.rs");
}

#[test]
fn completion_keeps_full_list_and_scrolls_selection_window() {
    let project = temp_project();
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    app.handle_action(KeyAction::Insert('/'), &state);
    for _ in 0..10 {
        app.handle_action(KeyAction::MoveSelectionDown, &state);
    }

    assert!(app.completion_items().len() > 8);
    assert_eq!(app.completion_selected(), 10);

    let mut terminal = Terminal::new(TestBackend::new(120, 30)).expect("terminal");
    terminal
        .draw(|frame| render_frame(frame, &state, &app))
        .expect("render");
    let screen = buffer_text(terminal.backend().buffer());
    assert!(screen.contains(app.completion_items()[10].label()));
}

#[test]
fn sessions_panel_selects_active_session_from_keyboard() {
    let project = temp_project();
    write_session(
        project.path(),
        "session-a",
        "2026-06-20T00:00:00Z",
        "finished",
    );
    write_session(
        project.path(),
        "session-b",
        "2026-06-20T01:00:00Z",
        "running",
    );
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    app.handle_action(KeyAction::FocusNext, &state);
    app.handle_action(KeyAction::MoveSelectionDown, &state);
    app.handle_action(KeyAction::MoveSelectionDown, &state);
    app.handle_action(KeyAction::MoveSelectionDown, &state);

    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );
    assert_eq!(app.center_panel(), CenterPanel::Sessions);
    assert_eq!(
        app.session_items(),
        vec![
            "Новая сессия".to_string(),
            "session-b".to_string(),
            "session-a".to_string()
        ]
    );

    app.handle_action(KeyAction::MoveSelectionDown, &state);
    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );
    assert_eq!(app.active_session_id(), Some("session-b"));
}

#[test]
fn sessions_panel_selects_active_session_from_mouse() {
    let project = temp_project();
    write_session(
        project.path(),
        "session-a",
        "2026-06-20T00:00:00Z",
        "finished",
    );
    write_session(
        project.path(),
        "session-b",
        "2026-06-20T01:00:00Z",
        "running",
    );
    let state = workspace_running();
    let area = ratatui::layout::Rect::new(0, 0, 140, 42);
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    assert_eq!(
        app.handle_mouse(left_click(4, 5), area, &state),
        AppIntent::None
    );
    assert_eq!(app.center_panel(), CenterPanel::Sessions);
    assert_eq!(
        app.handle_mouse(left_click(35, 5), area, &state),
        AppIntent::None
    );
    assert_eq!(app.active_session_id(), Some("session-b"));
}

#[test]
fn models_panel_changes_model_choice_and_emits_set_command() {
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.handle_action(KeyAction::FocusNext, &state);
    app.handle_action(KeyAction::MoveSelectionDown, &state);
    app.handle_action(KeyAction::MoveSelectionDown, &state);
    app.handle_action(KeyAction::MoveSelectionDown, &state);
    app.handle_action(KeyAction::MoveSelectionDown, &state);

    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );
    assert_eq!(app.center_panel(), CenterPanel::Models);
    assert_eq!(app.model_selected_agent(), Some("architect"));
    assert_eq!(app.model_selected_model(), Some("glm-5.1"));

    app.handle_action(KeyAction::MoveRight, &state);
    assert_eq!(app.model_selected_model(), Some("kimi-k2.6"));
    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::Command("models set --agent architect --model kimi-k2.6".into())
    );
}

#[test]
fn completion_arrows_and_enter_select_commands_and_files() {
    let project = temp_project();
    let state = workspace_running();
    let mut commands = WorkspaceApp::new();

    commands
        .sync_project_root(project.path())
        .expect("scan project");
    commands.handle_action(KeyAction::Insert('/'), &state);
    assert!(commands
        .completion_items()
        .iter()
        .take(8)
        .any(|item| item.insert_text() == "/models"));
    assert!(commands
        .completion_items()
        .iter()
        .take(8)
        .any(|item| item.insert_text() == "/limits"));
    assert!(!commands
        .completion_items()
        .iter()
        .take(8)
        .any(|item| item.insert_text() == "/budget"));

    commands.handle_action(KeyAction::MoveSelectionDown, &state);
    assert_eq!(commands.completion_selected(), 1);
    let expected_command = commands.completion_items()[1].insert_text().to_string();
    assert_eq!(
        commands.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );
    assert_eq!(commands.input(), expected_command);

    let mut files = WorkspaceApp::new();
    files
        .sync_project_root(project.path())
        .expect("scan project");
    files.handle_action(KeyAction::Insert('@'), &state);
    files.handle_action(KeyAction::MoveSelectionDown, &state);
    assert_eq!(files.completion_selected(), 1);
    assert_eq!(
        files.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );
    assert!(files.input().starts_with('@'));
    assert_ne!(files.input(), "@");
}

#[test]
fn completion_menus_follow_terminal_arrow_keys() {
    let project = temp_project();
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    app.handle_action(KeyAction::Insert('/'), &state);
    assert_eq!(app.completion_kind(), Some(CompletionKind::Commands));

    let down = action_for_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), false);
    let up = action_for_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), false);
    app.handle_action(down, &state);
    assert_eq!(app.completion_selected(), 1);
    app.handle_action(up, &state);
    assert_eq!(app.completion_selected(), 0);

    app.handle_action(KeyAction::Backspace, &state);
    app.handle_action(KeyAction::Insert('@'), &state);
    assert_eq!(app.completion_kind(), Some(CompletionKind::Files));
    app.handle_action(down, &state);
    assert_eq!(app.completion_selected(), 1);
}
