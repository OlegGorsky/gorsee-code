use crate::common::*;

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
