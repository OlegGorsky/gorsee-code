use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use gorsee_code_tui::{
    action_for_key, render_app, render_workspace, AppIntent, KeyAction, WorkspaceApp,
};
use gorsee_code_ui_state::{approval_waiting, workspace_running, ToolCallView};

#[test]
fn render_surfaces_workspace_controls_without_staged_language() {
    let output = render_workspace(&workspace_running());

    assert!(output.starts_with("Gorsee Code Workspace\n"));
    assert!(output.contains("Session"));
    assert!(output.contains("Repo"));
    assert!(output.contains("Branch"));
    assert!(output.contains("Security"));
    assert!(output.contains("Gateway"));
    assert_product_output(&output);
}

#[test]
fn render_pending_approvals_with_direct_commands() {
    let output = render_workspace(&approval_waiting());

    assert!(output.contains("Approvals"));
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

    assert!(output.contains("What should Gorsee Code do?"));
    assert!(output.contains(">"));
    assert!(output.contains("Enter run"));
    assert!(output.contains("/help commands"));
    assert!(output.contains("a approve"));
    assert!(output.contains("d deny"));
    assert!(output.contains("p pause"));
    assert!(output.contains("r resume"));
    assert!(output.contains("q quit"));
    assert!(output.contains("Status: ready"));
    assert_product_output(&output);
}

#[test]
fn key_mapping_keeps_typing_natural_until_prompt_is_empty() {
    assert_eq!(action_for_key(char_key('a'), true), KeyAction::Approve);
    assert_eq!(action_for_key(char_key('d'), true), KeyAction::Deny);
    assert_eq!(action_for_key(char_key('p'), true), KeyAction::Pause);
    assert_eq!(action_for_key(char_key('r'), true), KeyAction::Resume);
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

    assert_eq!(app.status(), Some("ready"));
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
        ("/timeline", "timeline:"),
        ("/approvals", "approvals:"),
    ] {
        let mut app = WorkspaceApp::new();

        assert_eq!(submit_line(&mut app, input, &state), AppIntent::None);

        let output = app.output().unwrap();
        assert_eq!(app.status(), Some("ready"));
        assert!(output.contains(expected));
        assert_product_output(output);
    }
}

#[test]
fn app_routes_external_commands_to_cli_handler() {
    let state = workspace_running();

    for (input, command) in [
        ("/capabilities", "capabilities"),
        ("/doctor", "doctor"),
        ("/hooks", "hooks"),
        ("/models", "models"),
        ("/skills", "skills"),
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
