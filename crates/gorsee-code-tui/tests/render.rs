use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use gorsee_code_tui::{
    action_for_key, render_app, render_frame, render_workspace, AppIntent, AttachmentKind,
    CenterPanel, CompletionKind, FocusPane, KeyAction, WorkspaceApp,
};
use gorsee_code_ui_state::{approval_waiting, workspace_running, ToolCallView};
use ratatui::{backend::TestBackend, style::Color, Terminal};

#[test]
fn ratatui_render_surfaces_workspace_shell() {
    let mut terminal = Terminal::new(TestBackend::new(140, 42)).expect("terminal");
    let mut app = WorkspaceApp::new();
    app.set_status("ready");

    terminal
        .draw(|frame| render_frame(frame, &workspace_running(), &app))
        .expect("render");

    let screen = buffer_text(terminal.backend().buffer());
    assert!(screen.contains("Gorsee Code"));
    assert!(screen.contains("МЕНЮ"));
    assert!(screen.contains("ФАЙЛЫ"));
    assert!(screen.contains("Architect"));
    assert!(screen.contains("Лента"));
    assert!(screen.contains("Введите задачу"));
    assert!(screen.contains("Powered by Neurogate"));
    assert_product_output(&screen);
}

#[test]
fn ratatui_sidebar_uses_menu_and_files_without_quick_commands() {
    let mut terminal = Terminal::new(TestBackend::new(140, 42)).expect("terminal");
    let app = WorkspaceApp::new();

    terminal
        .draw(|frame| render_frame(frame, &workspace_running(), &app))
        .expect("render");

    let screen = buffer_text(terminal.backend().buffer());
    for item in [
        "Проект",
        "Лента",
        "Дифф",
        "Сессии",
        "Модели",
        "Инструкции",
        "Скиллы",
        "MCP",
        "Лимиты",
        "ФАЙЛЫ",
    ] {
        assert!(screen.contains(item), "missing menu item {item}\n{screen}");
    }
    for removed in ["БЫСТРЫЕ", "План", "Поиск", "Настройки", "Терминал"]
    {
        assert!(
            !screen.contains(removed),
            "old sidebar item leaked: {removed}"
        );
    }
}

#[test]
fn tab_switches_arrow_focus_between_files_and_menu() {
    let project = temp_project();
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    assert_eq!(app.focus_pane(), FocusPane::Files);

    assert_eq!(
        app.handle_action(KeyAction::FocusNext, &state),
        AppIntent::None
    );
    assert_eq!(app.focus_pane(), FocusPane::Menu);
    assert_eq!(app.selected_menu_label(), "Проект");

    assert_eq!(
        app.handle_action(KeyAction::MoveSelectionDown, &state),
        AppIntent::None
    );
    assert_eq!(app.selected_menu_label(), "Лента");
    assert_eq!(
        app.handle_action(KeyAction::MoveSelectionDown, &state),
        AppIntent::None
    );
    assert_eq!(app.selected_menu_label(), "Дифф");
    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::Command("diff".into())
    );

    assert_eq!(
        app.handle_action(KeyAction::FocusNext, &state),
        AppIntent::None
    );
    assert_eq!(app.focus_pane(), FocusPane::Files);
}

#[test]
fn choosing_working_folder_rescans_files_inside_that_folder() {
    let project = temp_project();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    assert!(app
        .project_entries()
        .iter()
        .any(|entry| entry.path() == Path::new("README.md")));

    app.choose_working_folder(Path::new("src"))
        .expect("choose src as working folder");

    assert_eq!(
        app.working_folder().expect("working folder"),
        project.path().join("src")
    );
    assert!(app
        .project_entries()
        .iter()
        .any(|entry| entry.path() == Path::new("main.rs")));
    assert!(!app
        .project_entries()
        .iter()
        .any(|entry| entry.path() == Path::new("README.md")));
}

#[test]
fn file_focus_left_right_enters_and_leaves_working_folder() {
    let project = temp_project();
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    assert_eq!(app.focus_pane(), FocusPane::Files);
    assert_eq!(app.project_entries()[0].path(), Path::new("src"));

    assert_eq!(
        app.handle_action(KeyAction::MoveRight, &state),
        AppIntent::None
    );
    assert_eq!(
        app.working_folder().expect("working folder"),
        project.path().join("src")
    );
    assert!(app
        .project_entries()
        .iter()
        .any(|entry| entry.path() == Path::new("main.rs")));

    assert_eq!(
        app.handle_action(KeyAction::MoveLeft, &state),
        AppIntent::None
    );
    assert_eq!(
        app.working_folder().expect("working folder"),
        project.path()
    );
}

#[test]
fn ratatui_render_surfaces_approvals_and_command_output() {
    let mut terminal = Terminal::new(TestBackend::new(120, 36)).expect("terminal");
    let mut app = WorkspaceApp::new();
    app.set_output("agents:\n- architect model=glm-5.1");

    terminal
        .draw(|frame| render_frame(frame, &approval_waiting(), &app))
        .expect("render");

    let screen = buffer_text(terminal.backend().buffer());
    assert!(screen.contains("tool-1"));
    assert!(screen.contains("apply_patch"));
    assert!(screen.contains("agents:"));
    assert!(screen.contains("architect model=glm-5.1"));
    assert_product_output(&screen);
}

#[test]
fn right_panel_surfaces_agent_context_and_limits() {
    let mut terminal = Terminal::new(TestBackend::new(140, 42)).expect("terminal");
    let mut app = WorkspaceApp::new();
    app.set_status("running");

    terminal
        .draw(|frame| render_frame(frame, &workspace_running(), &app))
        .expect("render");

    let first = buffer_text(terminal.backend().buffer());
    assert!(first.contains("Контекст"));
    assert!(first.contains("токен"));
    assert!(first.contains("Лимиты"));
    assert!(first.contains("live-окна"));
    assert!(first.contains("/limits"));
}

#[test]
fn running_indicator_stays_in_timeline_not_context_panel() {
    let mut terminal = Terminal::new(TestBackend::new(140, 42)).expect("terminal");
    let mut app = WorkspaceApp::new();
    app.set_status("running");

    terminal
        .draw(|frame| render_frame(frame, &workspace_running(), &app))
        .expect("render");

    let first = buffer_text(terminal.backend().buffer());
    let first_context = right_panel_text(terminal.backend().buffer());
    assert!(first.contains("думаю..."));
    assert!(!first_context.contains("думаю..."), "{first_context}");

    app.advance_spinner();
    terminal
        .draw(|frame| render_frame(frame, &workspace_running(), &app))
        .expect("render");
    let second = buffer_text(terminal.backend().buffer());
    assert_ne!(
        first, second,
        "thinking indicator should animate between frames"
    );
}

#[test]
fn render_surfaces_workspace_controls_without_staged_language() {
    let output = render_workspace(&workspace_running());

    assert!(output.starts_with("Gorsee Code Workspace\n"));
    assert!(output.contains("Сессия"));
    assert!(output.contains("Repo"));
    assert!(output.contains("Branch"));
    assert!(output.contains("Безопасность"));
    assert!(output.contains("Gateway"));
    assert_product_output(&output);
}

#[test]
fn render_pending_approvals_with_direct_commands() {
    let output = render_workspace(&approval_waiting());

    assert!(output.contains("Подтверждения"));
    assert!(output.contains("tool-1"));
    assert!(output.contains("gcode approve tool-1"));
    assert!(output.contains("gcode deny tool-1"));
    assert_product_output(&output);
}

#[test]
fn render_app_surfaces_live_prompt_and_actions() {
    let mut app = WorkspaceApp::new();
    app.set_status("ready");

    let output = render_app(&workspace_running(), &app);

    assert!(output.contains("Введите задачу"));
    assert!(output.contains(">"));
    assert!(output.contains("Enter запуск"));
    assert!(output.contains("/help команды"));
    assert!(output.contains("a подтвердить"));
    assert!(output.contains("d отклонить"));
    assert!(output.contains("p пауза"));
    assert!(output.contains("r продолжить"));
    assert!(output.contains("q выход"));
    assert!(output.contains("Статус: ready"));
    assert_product_output(&output);
}

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

#[test]
fn external_command_outputs_open_diff_and_terminal_panels() {
    let state = workspace_running();
    let mut diff = WorkspaceApp::new();

    assert_eq!(
        submit_line(&mut diff, "/diff", &state),
        AppIntent::Command("diff".into())
    );
    assert_eq!(diff.center_panel(), CenterPanel::Diff);
    diff.set_output("diff --git a/src/main.rs b/src/main.rs\n+added");

    let mut terminal = Terminal::new(TestBackend::new(120, 30)).expect("terminal");
    terminal
        .draw(|frame| render_frame(frame, &state, &diff))
        .expect("render");
    let screen = buffer_text(terminal.backend().buffer());
    assert!(screen.contains("Diff"));
    assert!(screen.contains("+added"));

    let mut shell = WorkspaceApp::new();
    assert_eq!(
        submit_line(&mut shell, "/terminal printf ok", &state),
        AppIntent::Command("terminal printf ok".into())
    );
    assert_eq!(shell.center_panel(), CenterPanel::Terminal);
    shell.set_output("$ printf ok\nok\n");

    let mut terminal = Terminal::new(TestBackend::new(120, 30)).expect("terminal");
    terminal
        .draw(|frame| render_frame(frame, &state, &shell))
        .expect("render");
    let screen = buffer_text(terminal.backend().buffer());
    assert!(screen.contains("Терминал"));
    assert!(screen.contains("$ printf ok"));
}

#[test]
fn center_output_scrolls_with_page_keys() {
    let state = workspace_running();
    let mut app = WorkspaceApp::new();
    assert_eq!(
        submit_line(&mut app, "/diff", &state),
        AppIntent::Command("diff".into())
    );
    app.set_output(
        (0..50)
            .map(|index| format!("line-{index:02}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    let mut terminal = Terminal::new(TestBackend::new(120, 30)).expect("terminal");
    terminal
        .draw(|frame| render_frame(frame, &state, &app))
        .expect("render");
    let first = buffer_text(terminal.backend().buffer());
    assert!(first.contains("line-00"));

    for _ in 0..6 {
        app.handle_action(KeyAction::ScrollDown, &state);
    }
    terminal
        .draw(|frame| render_frame(frame, &state, &app))
        .expect("render");
    let second = buffer_text(terminal.backend().buffer());
    assert!(!second.contains("line-00"));
    assert!(second.contains("line-06"));
}

#[test]
fn paste_image_path_creates_attachment_chip() {
    let project = temp_project();
    let image = project.path().join("assets/screenshot.png");
    fs::create_dir_all(image.parent().expect("assets parent")).expect("assets dir");
    fs::write(&image, b"not a real png but enough for path metadata").expect("image file");
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    app.paste_text(image.to_str().expect("utf8 path"), &state);

    assert_eq!(app.attachments().len(), 1);
    assert_eq!(app.attachments()[0].kind(), AttachmentKind::Image);
    assert!(app.input().contains("@screenshot.png"));

    let mut terminal = Terminal::new(TestBackend::new(120, 30)).expect("terminal");
    terminal
        .draw(|frame| render_frame(frame, &state, &app))
        .expect("render");
    let screen = buffer_text(terminal.backend().buffer());
    assert!(screen.contains("Вложения"));
    assert!(screen.contains("screenshot.png"));
}

#[test]
fn attachments_are_included_in_submitted_objective() {
    let project = temp_project();
    let image = project.path().join("assets/screenshot.png");
    fs::create_dir_all(image.parent().expect("assets parent")).expect("assets dir");
    fs::write(&image, b"image").expect("image file");
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    app.paste_text(image.to_str().expect("utf8 path"), &state);
    for value in "проверь".chars() {
        app.handle_action(KeyAction::Insert(value), &state);
    }

    match app.handle_action(KeyAction::Submit, &state) {
        AppIntent::Submit(objective) => {
            assert!(objective.contains("Вложения:"));
            assert!(objective.contains(image.to_str().expect("utf8 path")));
        }
        other => panic!("expected submit, got {other:?}"),
    }
}

#[test]
fn app_turns_input_and_workspace_hotkeys_into_intents() {
    let mut app = WorkspaceApp::new();
    let state = approval_waiting();

    assert_eq!(
        app.handle_action(KeyAction::Insert('f'), &state),
        AppIntent::None
    );
    assert_eq!(
        app.handle_action(KeyAction::Insert('i'), &state),
        AppIntent::None
    );
    assert_eq!(app.input(), "fi");
    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::Submit("fi".into())
    );
    assert_eq!(app.input(), "");
    assert_eq!(app.output(), None);
    assert_eq!(app.status(), Some("running"));
    assert_eq!(
        app.handle_action(KeyAction::Approve, &state),
        AppIntent::Approve("tool-1".into())
    );
    assert_eq!(
        app.handle_action(KeyAction::Deny, &state),
        AppIntent::Deny("tool-1".into())
    );
    assert_eq!(
        app.handle_action(KeyAction::Pause, &state),
        AppIntent::Pause("approval-waiting".into())
    );
    assert_eq!(
        app.handle_action(KeyAction::Resume, &state),
        AppIntent::Resume("approval-waiting".into())
    );
    assert_eq!(app.handle_action(KeyAction::Quit, &state), AppIntent::Quit);
}

#[test]
fn app_targets_first_pending_approval_by_default() {
    let mut state = approval_waiting();
    state.approvals.push(ToolCallView {
        id: "tool-2".into(),
        name: "run_command".into(),
        status: "waiting_approval".into(),
        risk: "command".into(),
    });

    assert_eq!(
        WorkspaceApp::new().handle_action(KeyAction::Approve, &state),
        AppIntent::Approve("tool-1".into())
    );
    assert_eq!(
        WorkspaceApp::new().handle_action(KeyAction::Deny, &state),
        AppIntent::Deny("tool-1".into())
    );
    assert_eq!(
        submit_line(&mut WorkspaceApp::new(), "/approve", &state),
        AppIntent::Approve("tool-1".into())
    );
    assert_eq!(
        submit_line(&mut WorkspaceApp::new(), "/deny", &state),
        AppIntent::Deny("tool-1".into())
    );
}

#[test]
fn app_displays_workspace_command_output_inline() {
    let mut app = WorkspaceApp::new();
    let state = workspace_running();

    assert_eq!(submit_line(&mut app, "/agents", &state), AppIntent::None);

    assert_eq!(app.status(), Some("результат команды"));
    assert!(app.output().unwrap().contains("agents:"));
}

#[test]
fn app_keeps_workspace_overview_commands_inline() {
    let state = approval_waiting();

    for (input, expected) in [
        ("/budget", "budget:"),
        ("/usage", "budget:"),
        ("/route", "route:"),
        ("/context", "context:"),
        ("/approvals", "approvals:"),
    ] {
        let mut app = WorkspaceApp::new();

        assert_eq!(submit_line(&mut app, input, &state), AppIntent::None);

        let output = app.output().unwrap();
        assert_eq!(app.status(), Some("результат команды"));
        assert!(output.contains(expected));
        assert_product_output(output);
    }

    let mut timeline = WorkspaceApp::new();
    assert_eq!(
        submit_line(&mut timeline, "/timeline", &state),
        AppIntent::None
    );
    assert_eq!(timeline.center_panel(), CenterPanel::Timeline);
    assert_eq!(timeline.output(), None);
}

#[test]
fn app_routes_external_commands_to_cli_handler() {
    let state = workspace_running();

    for (input, command) in [
        ("/capabilities", "capabilities"),
        ("/doctor", "doctor"),
        ("/hooks", "hooks"),
        ("/skills show review", "skills show review"),
    ] {
        let mut app = WorkspaceApp::new();

        assert_eq!(
            submit_line(&mut app, input, &state),
            AppIntent::Command(command.into())
        );

        assert_eq!(app.status(), Some("working"));
        assert_eq!(app.output(), None);
    }
}

#[test]
fn app_opens_project_setting_panels_without_chat_output() {
    let project = temp_project();
    let state = workspace_running();
    let mut app = WorkspaceApp::new();
    app.sync_project_root(project.path()).expect("scan project");

    assert_eq!(submit_line(&mut app, "/project", &state), AppIntent::None);
    assert_eq!(app.center_panel(), CenterPanel::Project);
    assert_eq!(app.output(), None);

    assert_eq!(
        submit_line(&mut app, "/instructions", &state),
        AppIntent::None
    );
    assert_eq!(app.center_panel(), CenterPanel::Instructions);
    assert_eq!(app.output(), None);

    assert_eq!(submit_line(&mut app, "/skills", &state), AppIntent::None);
    assert_eq!(app.center_panel(), CenterPanel::Skills);
    assert_eq!(app.output(), None);

    assert_eq!(submit_line(&mut app, "/mcp", &state), AppIntent::None);
    assert_eq!(app.center_panel(), CenterPanel::Mcp);
    assert_eq!(app.output(), None);

    assert_eq!(submit_line(&mut app, "/limits", &state), AppIntent::None);
    assert_eq!(app.center_panel(), CenterPanel::Limits);
    assert_eq!(app.output(), None);

    assert_eq!(submit_line(&mut app, "/sessions", &state), AppIntent::None);
    assert_eq!(app.center_panel(), CenterPanel::Sessions);
    assert_eq!(app.output(), None);

    assert_eq!(submit_line(&mut app, "/models", &state), AppIntent::None);
    assert_eq!(app.center_panel(), CenterPanel::Models);
    assert_eq!(app.output(), None);
}

#[test]
fn project_setting_panels_open_editable_files() {
    let project = temp_project();
    let state = workspace_running();
    let mut app = WorkspaceApp::new();
    app.sync_project_root(project.path()).expect("scan project");

    assert_eq!(
        submit_line(&mut app, "/instructions", &state),
        AppIntent::None
    );
    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );
    assert_eq!(app.editor().unwrap().path(), Path::new("AGENTS.md"));
    assert!(project.path().join("AGENTS.md").exists());
    assert_eq!(
        app.handle_action(KeyAction::CloseEditor, &state),
        AppIntent::None
    );

    assert_eq!(submit_line(&mut app, "/skills", &state), AppIntent::None);
    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );
    assert_eq!(
        app.editor().unwrap().path(),
        Path::new(".gorsee-code/skills/repo-audit.md")
    );
    assert!(project
        .path()
        .join(".gorsee-code/skills/repo-audit.md")
        .exists());
}

#[test]
fn opening_setting_panel_resets_item_selection() {
    let project = temp_project();
    let state = workspace_running();
    let mut app = WorkspaceApp::new();

    app.sync_project_root(project.path()).expect("scan project");
    assert_eq!(submit_line(&mut app, "/project", &state), AppIntent::None);
    app.handle_action(KeyAction::MoveSelectionDown, &state);
    app.handle_action(KeyAction::MoveSelectionDown, &state);

    assert_eq!(
        submit_line(&mut app, "/instructions", &state),
        AppIntent::None
    );
    assert_eq!(
        app.handle_action(KeyAction::Submit, &state),
        AppIntent::None
    );
    assert_eq!(app.editor().unwrap().path(), Path::new("AGENTS.md"));
}

#[test]
fn app_routes_actionable_workspace_commands_to_cli_handler() {
    let state = workspace_running();

    for (input, command) in [
        ("/budget set --session 100k", "budget set --session 100k"),
        ("/route refactor auth", "route refactor auth"),
        ("/checkpoint", "checkpoint"),
    ] {
        let mut app = WorkspaceApp::new();

        assert_eq!(
            submit_line(&mut app, input, &state),
            AppIntent::Command(command.into())
        );

        assert_eq!(app.status(), Some("working"));
        assert_eq!(app.output(), None);
    }
}

#[test]
fn app_turns_slash_action_commands_into_intents() {
    assert_eq!(
        submit_line(&mut WorkspaceApp::new(), "/approve", &approval_waiting()),
        AppIntent::Approve("tool-1".into())
    );
    assert_eq!(
        submit_line(&mut WorkspaceApp::new(), "/pause", &approval_waiting()),
        AppIntent::Pause("approval-waiting".into())
    );
    assert_eq!(
        submit_line(&mut WorkspaceApp::new(), "/quit", &workspace_running()),
        AppIntent::Quit
    );
}

fn assert_product_output(output: &str) {
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

fn char_key(value: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(value), KeyModifiers::NONE)
}

fn left_click(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn left_drag(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn left_release(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn scroll_down(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
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

fn right_panel_text(buffer: &ratatui::buffer::Buffer) -> String {
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

fn buffer_has_bg(buffer: &ratatui::buffer::Buffer, bg: Color) -> bool {
    buffer.content().iter().any(|cell| cell.bg == bg)
}

fn submit_line(
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

struct TempProject {
    path: PathBuf,
}

impl TempProject {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn temp_project() -> TempProject {
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

fn write_session(root: &Path, id: &str, started_at: &str, status: &str) {
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
