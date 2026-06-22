use crate::common::*;

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
