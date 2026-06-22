use crate::common::*;

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
