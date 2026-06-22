use crate::common::*;

#[test]
fn app_opens_selected_project_file_and_toggles_dirs_from_keyboard() {
    let project = temp_project();
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    assert_eq!(app.project_entries()[0].path(), Path::new("src"));

    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );
    assert!(
        !app.project_entries()
            .iter()
            .any(|entry| entry.path() == Path::new("src/main.rs")),
        "enter on directory should collapse it"
    );

    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );
    app.handle_action(KeyAction::MoveSelectionDown, &state);
    app.handle_action(KeyAction::MoveSelectionDown, &state);

    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );
    assert_eq!(
        app.editor().expect("editor").path(),
        Path::new("src/main.rs")
    );
}

#[test]
fn editor_supports_scroll_keys_and_renders_visible_window() {
    let project = temp_project();
    fs::write(
        project.path().join("LONG.md"),
        (0..40)
            .map(|index| format!("line-{index:02}"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .expect("write long file");
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    app.open_project_file(Path::new("LONG.md"))
        .expect("open file");
    assert_eq!(app.editor().expect("editor").scroll(), 0);

    assert_eq!(
        action_for_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), false),
        KeyAction::ScrollDown
    );
    app.handle_action(KeyAction::ScrollDown, &state);
    app.handle_action(KeyAction::ScrollDown, &state);
    assert!(app.editor().expect("editor").scroll() > 0);

    let mut terminal = Terminal::new(TestBackend::new(120, 26)).expect("terminal");
    terminal
        .draw(|frame| render_frame(frame, &state, &app))
        .expect("render");
    let screen = buffer_text(terminal.backend().buffer());
    assert!(screen.contains("line-02"));
    assert!(!screen.contains("line-00"));
}

#[test]
fn mouse_wheel_scrolls_open_editor() {
    let project = temp_project();
    fs::write(
        project.path().join("LONG.md"),
        (0..40)
            .map(|index| format!("line-{index:02}"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .expect("write long file");
    let state = workspace_running();
    let area = ratatui::layout::Rect::new(0, 0, 140, 42);
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    app.open_project_file(Path::new("LONG.md"))
        .expect("open file");

    assert_eq!(
        app.handle_mouse(scroll_down(60, 10), area, &state),
        AppIntent::None
    );

    assert!(app.editor().expect("editor").scroll() > 0);
}

#[test]
fn mouse_drag_selection_copies_center_text() {
    let state = workspace_running();
    let area = ratatui::layout::Rect::new(0, 0, 140, 42);
    let mut app = WorkspaceApp::new();

    assert_eq!(
        app.handle_mouse(left_click(35, 5), area, &state),
        AppIntent::None
    );
    assert_eq!(
        app.handle_mouse(left_drag(80, 7), area, &state),
        AppIntent::None
    );
    let intent = app.handle_mouse(left_release(80, 7), area, &state);

    match intent {
        AppIntent::Copy(text) => {
            assert!(!text.trim().is_empty());
            assert_eq!(app.status(), Some("Скопировано!"));
        }
        other => panic!("expected copy intent, got {other:?}"),
    }
}

#[test]
fn mouse_drag_selection_renders_visible_highlight() {
    let state = workspace_running();
    let area = ratatui::layout::Rect::new(0, 0, 140, 42);
    let mut app = WorkspaceApp::new();

    assert_eq!(
        app.handle_mouse(left_click(35, 5), area, &state),
        AppIntent::None
    );
    assert_eq!(
        app.handle_mouse(left_drag(80, 7), area, &state),
        AppIntent::None
    );

    let mut terminal = Terminal::new(TestBackend::new(140, 42)).expect("terminal");
    terminal
        .draw(|frame| render_frame(frame, &state, &app))
        .expect("render");

    assert!(buffer_has_bg(
        terminal.backend().buffer(),
        Color::Rgb(76, 60, 180)
    ));
}
