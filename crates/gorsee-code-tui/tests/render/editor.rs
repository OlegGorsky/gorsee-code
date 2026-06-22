use crate::common::*;

#[test]
fn key_mapping_keeps_typing_natural_until_prompt_is_empty() {
    assert_eq!(action_for_key(char_key('a'), true), KeyAction::Insert('a'));
    assert_eq!(action_for_key(char_key('d'), true), KeyAction::Insert('d'));
    assert_eq!(action_for_key(char_key('p'), true), KeyAction::Insert('p'));
    assert_eq!(action_for_key(char_key('r'), true), KeyAction::Insert('r'));
    assert_eq!(action_for_key(char_key('q'), true), KeyAction::Quit);

    assert_eq!(action_for_key(char_key('a'), false), KeyAction::Insert('a'));
    assert_eq!(action_for_key(char_key('d'), false), KeyAction::Insert('d'));
    assert_eq!(action_for_key(char_key('p'), false), KeyAction::Insert('p'));
    assert_eq!(action_for_key(char_key('r'), false), KeyAction::Insert('r'));
    assert_eq!(action_for_key(char_key('q'), false), KeyAction::Insert('q'));
    assert_eq!(
        action_for_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), false),
        KeyAction::Submit
    );
    assert_eq!(
        action_for_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), false),
        KeyAction::FocusNext
    );
}

#[test]
fn key_mapping_supports_cursor_and_multiline_composer() {
    assert_eq!(
        action_for_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), false),
        KeyAction::MoveLeft
    );
    assert_eq!(
        action_for_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), false),
        KeyAction::MoveRight
    );
    assert_eq!(
        action_for_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT), false),
        KeyAction::Newline
    );
    assert_eq!(
        action_for_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT), false),
        KeyAction::Newline
    );
}

#[test]
fn app_edits_input_at_cursor() {
    let mut app = WorkspaceApp::new();
    let state = workspace_running();

    app.handle_action(KeyAction::Insert('a'), &state);
    app.handle_action(KeyAction::Insert('b'), &state);
    app.handle_action(KeyAction::Insert('c'), &state);
    app.handle_action(KeyAction::MoveLeft, &state);
    app.handle_action(KeyAction::Insert('X'), &state);

    assert_eq!(app.input(), "abXc");
    assert_eq!(app.input_cursor(), 3);
}

#[test]
fn app_scans_project_tree_and_opens_files_in_editor() {
    let project = temp_project();
    let mut app = WorkspaceApp::new();
    app.sync_project_root(project.path()).expect("scan project");

    let entries = app.project_entries();
    assert!(
        entries
            .iter()
            .any(|entry| entry.path() == Path::new("src/main.rs") && !entry.is_dir()),
        "project tree should expose real files: {entries:?}"
    );

    app.open_project_file(Path::new("src/main.rs"))
        .expect("open source file");

    let editor = app.editor().expect("editor");
    assert_eq!(editor.path(), Path::new("src/main.rs"));
    assert!(editor.text().contains("fn main()"));
    assert!(app.is_editor_open());

    let mut terminal = Terminal::new(TestBackend::new(140, 42)).expect("terminal");
    terminal
        .draw(|frame| render_frame(frame, &workspace_running(), &app))
        .expect("render");

    let screen = buffer_text(terminal.backend().buffer());
    assert!(screen.contains("src/main.rs"));
    assert!(screen.contains("fn main()"));
    assert!(screen.contains("Ctrl+S"));
    assert_product_output(&screen);
}

#[test]
fn app_edits_saves_and_closes_open_file() {
    let project = temp_project();
    let file = project.path().join("README.md");
    let mut app = WorkspaceApp::new();
    let state = workspace_running();

    app.sync_project_root(project.path()).expect("scan project");
    app.open_project_file(Path::new("README.md"))
        .expect("open readme");

    app.handle_action(KeyAction::Insert('!'), &state);
    assert!(app.editor().expect("editor").is_dirty());

    assert_eq!(app.handle_action(KeyAction::Save, &state), AppIntent::None);
    assert_eq!(
        fs::read_to_string(&file).expect("read saved file"),
        "# Gorsee!\n"
    );

    assert_eq!(
        app.handle_action(KeyAction::CloseEditor, &state),
        AppIntent::None
    );
    assert!(!app.is_editor_open());
}

#[test]
fn dirty_editor_requires_second_close_confirmation() {
    let project = temp_project();
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    app.open_project_file(Path::new("README.md"))
        .expect("open readme");
    app.handle_action(KeyAction::Insert('!'), &state);

    assert_eq!(
        app.handle_action(KeyAction::CloseEditor, &state),
        AppIntent::None
    );
    assert!(app.is_editor_open());
    assert!(app.status().unwrap().contains("Ctrl+W еще раз"));

    assert_eq!(
        app.handle_action(KeyAction::CloseEditor, &state),
        AppIntent::None
    );
    assert!(!app.is_editor_open());
}

#[test]
fn editor_enter_inserts_newline_instead_of_submitting() {
    let project = temp_project();
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    app.open_project_file(Path::new("README.md"))
        .expect("open readme");

    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );

    let editor = app.editor().expect("editor");
    assert!(editor.is_dirty());
    assert_eq!(editor.text(), "# Gorsee\n\n");
}
