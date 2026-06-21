use std::{
    io::Write,
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
};

use anyhow::Result;
use gorsee_code_ui_state::WorkspaceState;

use crate::{
    clipboard::write_clipboard_osc52, runtime_draw::draw_app, terminal::TuiTerminal, AppIntent,
    TuiHandlers, WorkspaceApp,
};

pub(crate) type Worker = JoinHandle<Result<String>>;

pub(crate) fn action_root(app: &WorkspaceApp, initial_root: &Path) -> PathBuf {
    app.working_folder()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| initial_root.to_path_buf())
}

pub(crate) fn process_intent(
    intent: AppIntent,
    worker: &mut Option<Worker>,
    handlers: &TuiHandlers,
    app: &mut WorkspaceApp,
    initial_root: &Path,
) -> bool {
    let root = action_root(app, initial_root);
    match intent {
        AppIntent::None => false,
        AppIntent::Quit => handle_quit(worker, app),
        AppIntent::Copy(_) => false,
        AppIntent::Submit(text) => start_job(worker, app, "running", true, {
            let run = handlers.submit.clone();
            move || run(&root, text)
        }),
        AppIntent::Approve(id) => start_job(worker, app, "approving", false, {
            let run = handlers.approve.clone();
            move || run(&root, id)
        }),
        AppIntent::Deny(id) => start_job(worker, app, "denying", false, {
            let run = handlers.deny.clone();
            move || run(&root, id)
        }),
        AppIntent::Pause(id) => start_job(worker, app, "pausing", false, {
            let run = handlers.pause.clone();
            move || run(&root, id)
        }),
        AppIntent::Resume(id) => start_job(worker, app, "resuming", false, {
            let run = handlers.resume.clone();
            move || run(&root, id)
        }),
        AppIntent::Command(line) => start_job(worker, app, "working", false, {
            let run = handlers.command.clone();
            move || run(&root, line)
        }),
    }
}

pub(crate) fn handle_interactive_intent(
    terminal: &mut TuiTerminal,
    intent: AppIntent,
    worker: &mut Option<Worker>,
    handlers: &TuiHandlers,
    app: &mut WorkspaceApp,
    initial_root: &Path,
) -> Result<bool> {
    if let AppIntent::Copy(text) = &intent {
        write_clipboard_osc52(terminal, text)?;
    }
    Ok(process_intent(intent, worker, handlers, app, initial_root))
}

pub(crate) fn finish_worker(worker: &mut Option<Worker>, app: &mut WorkspaceApp) {
    if worker
        .as_ref()
        .map(|job| job.is_finished())
        .unwrap_or(false)
    {
        finish_joined(worker.take().expect("checked worker presence"), app);
    }
}

pub(crate) fn finish_worker_blocking(
    worker: &mut Option<Worker>,
    app: &mut WorkspaceApp,
    stdout: &mut impl Write,
    load_state: &impl Fn(&Path, Option<&str>) -> WorkspaceState,
    initial_root: &Path,
) -> Result<()> {
    if let Some(job) = worker.take() {
        finish_joined(job, app);
        let root = action_root(app, initial_root);
        draw_app(stdout, &load_state(&root, app.active_session_id()), app)?;
    }
    Ok(())
}

pub(crate) fn finish_joined(job: Worker, app: &mut WorkspaceApp) {
    app.clear_pending_prompt();
    match job.join() {
        Ok(Ok(output)) => {
            if let Some(session_id) = completed_session_id(&output) {
                app.active_session_id = Some(session_id.clone());
                app.center_panel = crate::CenterPanel::Timeline;
                app.clear_output();
                app.clear_status();
                return;
            }
            app.set_status(compact_status(&output));
            if output.trim().is_empty() {
                app.clear_output();
            } else {
                app.set_output(output);
            }
        }
        Ok(Err(error)) => {
            let message = display_error(&error);
            app.set_status(message.clone());
            app.set_output(message);
        }
        Err(_) => {
            let message = "Ошибка запуска: действие остановлено".to_string();
            app.set_status(message.clone());
            app.set_output(message);
        }
    }
}

fn handle_quit(worker: &Option<Worker>, app: &mut WorkspaceApp) -> bool {
    if worker.is_none() {
        return true;
    }
    if app.status() == Some("выход подтвержден") {
        return true;
    }
    app.set_status("выход подтвержден");
    app.set_output("Агент еще работает. Нажмите q еще раз, чтобы выйти без ожидания.");
    false
}

fn start_job(
    worker: &mut Option<Worker>,
    app: &mut WorkspaceApp,
    status: &'static str,
    clear_attachments: bool,
    job: impl FnOnce() -> Result<String> + Send + 'static,
) -> bool {
    if worker.is_some() {
        app.restore_pending_prompt();
        app.set_status("занято: дождитесь завершения действия");
        return false;
    }
    if !clear_attachments {
        app.clear_pending_prompt();
    }
    if clear_attachments {
        app.clear_attachments();
    }
    app.set_status(status);
    *worker = Some(thread::spawn(job));
    false
}

fn compact_status(output: &str) -> String {
    output
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .unwrap_or_else(|| "complete".into())
}

fn display_error(error: &anyhow::Error) -> String {
    let raw = error.to_string();
    if raw.starts_with("invalid model response:") {
        return "Ошибка ответа модели: не удалось разобрать ответ. Попробуйте повторить запрос или сформулировать задачу подробнее.".into();
    }
    format!("Ошибка запуска: {raw}")
}

fn completed_session_id(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        line.strip_prefix("run: completed session=")
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned)
    })
}
