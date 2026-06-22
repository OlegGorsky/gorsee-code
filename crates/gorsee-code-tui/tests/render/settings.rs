use crate::common::*;

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
