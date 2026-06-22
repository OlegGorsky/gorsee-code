use super::*;
use crate::{AppIntent, KeyAction};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

#[test]
fn failed_worker_sets_visible_output() {
    let job = thread::spawn(|| -> Result<String> { Err(anyhow::anyhow!("missing auth")) });
    let mut app = WorkspaceApp::new();

    finish_joined(job, &mut app);

    assert!(app.status().unwrap().contains("missing auth"));
    assert!(app.output().unwrap().contains("missing auth"));
}

#[test]
fn completed_submit_selects_created_session_without_fake_output() {
    let job = thread::spawn(|| -> Result<String> {
        Ok("run: completed session=session-123\nevents=4\nagents=architect".into())
    });
    let mut app = WorkspaceApp::new();

    finish_joined(job, &mut app);

    assert_eq!(app.active_session_id(), Some("session-123"));
    assert_eq!(app.output(), None);
    assert_eq!(app.status(), None);
}

#[test]
fn submitted_turn_selects_session_from_lcp_output() {
    let job = thread::spawn(|| -> Result<String> {
        Ok("run: session=session-456\nstatus=waiting_approval\nintent=Edit".into())
    });
    let mut app = WorkspaceApp::new();

    finish_joined(job, &mut app);

    assert_eq!(app.active_session_id(), Some("session-456"));
    assert_eq!(app.output(), None);
    assert_eq!(app.status(), None);
}

#[test]
fn invalid_model_response_is_not_dumped_into_chat() {
    let job = thread::spawn(|| -> Result<String> {
        Err(anyhow::anyhow!(
            "invalid model response: invalid json: expected `,` or `}}`: {{\"message\":\"raw\"}}"
        ))
    });
    let mut app = WorkspaceApp::new();

    finish_joined(job, &mut app);

    let output = app.output().expect("visible error");
    assert!(output.contains("Ошибка ответа модели"));
    assert!(!output.contains("{\"message\""));
}

#[test]
fn command_handler_uses_selected_working_folder() {
    let temp = tempfile::tempdir().unwrap();
    let child = temp.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let calls = Arc::new(Mutex::new(Vec::<(PathBuf, String)>::new()));
    let command_calls = calls.clone();
    let handlers = TuiHandlers::new(
        noop_submit_handler,
        noop_handler,
        noop_handler,
        noop_handler,
        noop_handler,
        move |root, line| {
            command_calls
                .lock()
                .unwrap()
                .push((root.to_path_buf(), line));
            Ok("ok".into())
        },
    );
    let mut app = WorkspaceApp::new();
    let mut worker = None;

    app.sync_project_root(temp.path()).unwrap();
    app.choose_working_folder(&child).unwrap();
    process_intent(
        AppIntent::Command("diff".into()),
        &mut worker,
        &handlers,
        &mut app,
        temp.path(),
    );
    finish_joined(worker.take().expect("worker"), &mut app);

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &[(child, "diff".to_string())]
    );
}

#[test]
fn submit_handler_receives_active_session_id() {
    let temp = tempfile::tempdir().unwrap();
    let calls = Arc::new(Mutex::new(Vec::<Option<String>>::new()));
    let submit_calls = calls.clone();
    let handlers = TuiHandlers::new(
        move |_root, session_id, _line| {
            submit_calls
                .lock()
                .unwrap()
                .push(session_id.map(ToOwned::to_owned));
            Ok("run: completed session=session-123".into())
        },
        noop_handler,
        noop_handler,
        noop_handler,
        noop_handler,
        noop_handler,
    );
    let mut app = WorkspaceApp::new();
    app.active_session_id = Some("session-123".into());
    let mut worker = None;

    process_intent(
        AppIntent::Submit("следующий turn".into()),
        &mut worker,
        &handlers,
        &mut app,
        temp.path(),
    );
    finish_joined(worker.take().expect("worker"), &mut app);

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &[Some("session-123".into())]
    );
}

#[test]
fn busy_worker_restores_submitted_prompt() {
    let handlers = TuiHandlers::new(
        noop_submit_handler,
        noop_handler,
        noop_handler,
        noop_handler,
        noop_handler,
        noop_handler,
    );
    let state = gorsee_code_ui_state::workspace_running();
    let mut app = WorkspaceApp::new();
    let mut worker = Some(thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(50));
        Ok("first".into())
    }));

    for value in "second".chars() {
        app.handle_action(KeyAction::Insert(value), &state);
    }
    let intent = app.handle_action(KeyAction::Submit, &state);
    assert_eq!(app.input(), "");

    assert!(!process_intent(
        intent,
        &mut worker,
        &handlers,
        &mut app,
        Path::new("."),
    ));

    assert_eq!(app.input(), "second");
    assert_eq!(app.status(), Some("занято: дождитесь завершения действия"));
    finish_joined(worker.take().expect("worker"), &mut app);
}

#[test]
fn quit_while_worker_is_running_requires_second_quit() {
    let handlers = TuiHandlers::new(
        noop_submit_handler,
        noop_handler,
        noop_handler,
        noop_handler,
        noop_handler,
        noop_handler,
    );
    let mut app = WorkspaceApp::new();
    let mut worker = Some(thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(50));
        Ok("done".into())
    }));

    assert!(!process_intent(
        AppIntent::Quit,
        &mut worker,
        &handlers,
        &mut app,
        Path::new("."),
    ));
    assert_eq!(app.status(), Some("выход подтвержден"));
    assert!(process_intent(
        AppIntent::Quit,
        &mut worker,
        &handlers,
        &mut app,
        Path::new("."),
    ));
    finish_joined(worker.take().expect("worker"), &mut app);
}

fn noop_handler(_root: &Path, _line: String) -> Result<String> {
    Ok(String::new())
}

fn noop_submit_handler(_root: &Path, _session_id: Option<&str>, _line: String) -> Result<String> {
    Ok(String::new())
}
