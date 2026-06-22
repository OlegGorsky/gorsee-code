use crate::common::*;

#[test]
fn project_panel_changes_working_folder_from_center() {
    let project = temp_project();
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    assert_eq!(submit_line(&mut app, "/project", &state), AppIntent::None);
    assert_eq!(app.center_panel(), CenterPanel::Project);
    assert_eq!(app.output(), None);

    app.handle_action(KeyAction::MoveSelectionDown, &state);
    app.handle_action(KeyAction::MoveSelectionDown, &state);
    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );

    assert_eq!(
        app.working_folder().expect("working folder"),
        project.path().parent().expect("temp parent")
    );
}

#[test]
fn project_panel_input_path_item_prefills_composer() {
    let project = temp_project();
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    assert_eq!(submit_line(&mut app, "/project", &state), AppIntent::None);
    app.handle_action(KeyAction::MoveSelectionDown, &state);

    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );

    assert_eq!(app.input(), "/project ");
    assert_eq!(app.input_cursor(), "/project ".len());
    assert_eq!(app.status(), Some("введите путь проекта"));
}

#[test]
fn project_panel_current_project_item_is_selectable() {
    let project = temp_project();
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    assert_eq!(submit_line(&mut app, "/project", &state), AppIntent::None);

    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );

    assert_eq!(app.working_folder(), Some(project.path()));
    assert_eq!(app.center_panel(), CenterPanel::Project);
}

#[test]
fn running_chat_shows_prompt_and_thinking_in_timeline() {
    let state = workspace_running();
    let mut app = WorkspaceApp::new();
    let intent = submit_line(&mut app, "Привет", &state);
    assert_eq!(intent, AppIntent::Submit("Привет".into()));

    let mut terminal = Terminal::new(TestBackend::new(140, 42)).expect("terminal");
    terminal
        .draw(|frame| render_frame(frame, &state, &app))
        .expect("render");

    let screen = buffer_text(terminal.backend().buffer());
    assert!(screen.contains("Вы · Привет"), "{screen}");
    assert!(screen.contains("думаю..."), "{screen}");
}

#[test]
fn project_command_resolves_sibling_folder() {
    let project = temp_project();
    let sibling = project.path().with_file_name(format!(
        "{}-sibling",
        project.path().file_name().unwrap().to_string_lossy()
    ));
    fs::create_dir_all(&sibling).expect("create sibling");
    let state = workspace_running();
    let mut app = WorkspaceApp::new();
    app.sync_project_root(project.path()).expect("scan project");

    let command = format!(
        "/project {}",
        sibling.file_name().unwrap().to_string_lossy()
    );
    assert_eq!(submit_line(&mut app, &command, &state), AppIntent::None);

    assert_eq!(app.working_folder(), Some(sibling.as_path()));
    let _ = fs::remove_dir_all(sibling);
}

#[test]
fn changing_project_clears_active_session_scope() {
    let project = temp_project();
    let other = temp_project();
    write_session(
        project.path(),
        "session-a",
        "2026-06-20T00:00:00Z",
        "running",
    );
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    assert_eq!(submit_line(&mut app, "/sessions", &state), AppIntent::None);
    app.handle_action(KeyAction::MoveSelectionDown, &state);
    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );
    assert_eq!(app.active_session_id(), Some("session-a"));

    app.choose_working_folder(other.path())
        .expect("choose other");

    assert_eq!(app.active_session_id(), None);
    assert!(app.session_items().is_empty());
}
