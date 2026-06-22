use crate::common::*;

#[test]
fn app_supports_mouse_project_open_and_composer_cursor_placement() {
    let project = temp_project();
    let state = workspace_running();
    let area = ratatui::layout::Rect::new(0, 0, 140, 42);
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    let file_row = app
        .project_row_for_path(Path::new("src/main.rs"))
        .expect("file row");
    assert_eq!(
        app.handle_mouse(left_click(3, file_row), area, &state,),
        AppIntent::None
    );
    assert_eq!(
        app.editor().expect("editor").path(),
        Path::new("src/main.rs")
    );

    app.handle_action(KeyAction::CloseEditor, &state);
    for value in "hello".chars() {
        app.handle_action(KeyAction::Insert(value), &state);
    }
    assert_eq!(
        app.handle_mouse(left_click(4, 37), area, &state),
        AppIntent::None
    );
    assert_eq!(app.input_cursor(), 3);
    app.handle_action(KeyAction::Insert('X'), &state);
    assert_eq!(app.input(), "helXlo");
}

#[test]
fn mouse_uses_compact_sidebar_project_rows_and_completion_rows() {
    let project = temp_project();
    let state = workspace_running();
    let area = ratatui::layout::Rect::new(0, 0, 80, 24);
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    assert!(app
        .project_entries()
        .iter()
        .any(|entry| entry.path() == Path::new("src/main.rs")));

    assert_eq!(
        app.handle_mouse(left_click(3, 15), area, &state),
        AppIntent::None
    );
    assert!(app
        .project_entries()
        .iter()
        .any(|entry| entry.path() == Path::new("src/main.rs")));

    assert_eq!(
        app.handle_mouse(left_click(3, 16), area, &state),
        AppIntent::None
    );
    assert!(
        !app.project_entries()
            .iter()
            .any(|entry| entry.path() == Path::new("src/main.rs")),
        "click on first visible project entry should toggle src"
    );

    app.handle_action(KeyAction::Insert('/'), &state);
    assert_eq!(
        app.handle_mouse(left_click(4, 12), area, &state),
        AppIntent::None
    );
    assert_ne!(app.input(), "/");
}

#[test]
fn mouse_sidebar_menu_items_are_actionable() {
    let state = workspace_running();
    let area = ratatui::layout::Rect::new(0, 0, 80, 24);
    let mut app = WorkspaceApp::new();

    assert_eq!(
        app.handle_mouse(left_click(4, 4), area, &state),
        AppIntent::Command("diff".into())
    );
    assert_eq!(app.center_panel(), CenterPanel::Diff);

    assert_eq!(
        app.handle_mouse(left_click(4, 3), area, &state),
        AppIntent::None
    );
    assert_eq!(app.center_panel(), CenterPanel::Timeline);
    assert_eq!(app.output(), None);
}

#[test]
fn mouse_sidebar_menu_items_work_on_release() {
    let state = workspace_running();
    let area = ratatui::layout::Rect::new(0, 0, 80, 24);
    let mut app = WorkspaceApp::new();

    assert_eq!(
        app.handle_mouse(left_release(4, 4), area, &state),
        AppIntent::Command("diff".into())
    );
    assert_eq!(app.center_panel(), CenterPanel::Diff);
}

#[test]
fn mouse_release_does_not_repeat_consumed_mouse_down() {
    let state = workspace_running();
    let area = ratatui::layout::Rect::new(0, 0, 80, 24);
    let mut app = WorkspaceApp::new();

    assert_eq!(
        app.handle_mouse(left_click(4, 4), area, &state),
        AppIntent::Command("diff".into())
    );
    assert_eq!(
        app.handle_mouse(left_release(4, 4), area, &state),
        AppIntent::None
    );
}

#[test]
fn mouse_project_release_does_not_toggle_after_mouse_down() {
    let project = temp_project();
    let state = workspace_running();
    let area = ratatui::layout::Rect::new(0, 0, 80, 24);
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    assert!(app
        .project_entries()
        .iter()
        .any(|entry| entry.path() == Path::new("src/main.rs")));

    assert_eq!(
        app.handle_mouse(left_click(3, 16), area, &state),
        AppIntent::None
    );
    assert!(!app
        .project_entries()
        .iter()
        .any(|entry| entry.path() == Path::new("src/main.rs")));

    assert_eq!(
        app.handle_mouse(left_release(3, 16), area, &state),
        AppIntent::None
    );
    assert!(!app
        .project_entries()
        .iter()
        .any(|entry| entry.path() == Path::new("src/main.rs")));
}
