use std::{
    io::{self, IsTerminal, Read, Write},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event},
    execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use gorsee_code_ui_state::WorkspaceState;

use crate::{
    action_for_key, input::action_for_byte, render_app, render_workspace, AppIntent, TuiHandlers,
    WorkspaceApp,
};

type Worker = JoinHandle<Result<String>>;

pub fn run_app(load_state: impl Fn() -> WorkspaceState, handlers: TuiHandlers) -> Result<()> {
    let mut stdout = io::stdout();
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    let _terminal = TerminalSession::enter(&mut stdout)?;
    let mut app = WorkspaceApp::new();
    let mut worker = None;

    if interactive {
        run_interactive_loop(&mut stdout, &mut app, &mut worker, &handlers, &load_state)
    } else {
        run_scripted_loop(&mut stdout, &mut app, &mut worker, &handlers, &load_state)
    }
}

pub fn run_workspace(state: &WorkspaceState) -> Result<()> {
    let mut stdout = io::stdout();
    let _terminal = TerminalSession::enter(&mut stdout)?;
    draw_workspace(&mut stdout, state)?;
    read_until_quit()
}

fn run_interactive_loop(
    stdout: &mut impl Write,
    app: &mut WorkspaceApp,
    worker: &mut Option<Worker>,
    handlers: &TuiHandlers,
    load_state: &impl Fn() -> WorkspaceState,
) -> Result<()> {
    loop {
        let state = load_state();
        finish_worker(worker, app);
        draw_app(stdout, &state, app)?;
        if event::poll(Duration::from_millis(250))? {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            let action = action_for_key(key, app.input().is_empty());
            if process_intent(app.handle_action(action, &state), worker, handlers, app) {
                return Ok(());
            }
        }
    }
}

fn run_scripted_loop(
    stdout: &mut impl Write,
    app: &mut WorkspaceApp,
    worker: &mut Option<Worker>,
    handlers: &TuiHandlers,
    load_state: &impl Fn() -> WorkspaceState,
) -> Result<()> {
    draw_app(stdout, &load_state(), app)?;
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut byte = [0_u8; 1];
    loop {
        match stdin.read(&mut byte)? {
            0 => return finish_worker_blocking(worker, app, stdout, load_state),
            _ => {
                let state = load_state();
                let action = action_for_byte(byte[0], app.input().is_empty());
                if process_intent(app.handle_action(action, &state), worker, handlers, app) {
                    return Ok(());
                }
                finish_worker(worker, app);
                draw_app(stdout, &state, app)?;
            }
        }
    }
}

fn process_intent(
    intent: AppIntent,
    worker: &mut Option<Worker>,
    handlers: &TuiHandlers,
    app: &mut WorkspaceApp,
) -> bool {
    match intent {
        AppIntent::None => false,
        AppIntent::Quit => true,
        AppIntent::Submit(text) => start_job(worker, app, "running", {
            let run = handlers.submit.clone();
            move || run(text)
        }),
        AppIntent::Approve(id) => start_job(worker, app, "approving", {
            let run = handlers.approve.clone();
            move || run(id)
        }),
        AppIntent::Deny(id) => start_job(worker, app, "denying", {
            let run = handlers.deny.clone();
            move || run(id)
        }),
        AppIntent::Pause(id) => start_job(worker, app, "pausing", {
            let run = handlers.pause.clone();
            move || run(id)
        }),
        AppIntent::Resume(id) => start_job(worker, app, "resuming", {
            let run = handlers.resume.clone();
            move || run(id)
        }),
        AppIntent::Command(line) => start_job(worker, app, "working", {
            let run = handlers.command.clone();
            move || run(line)
        }),
    }
}

fn start_job(
    worker: &mut Option<Worker>,
    app: &mut WorkspaceApp,
    status: &'static str,
    job: impl FnOnce() -> Result<String> + Send + 'static,
) -> bool {
    if worker.is_some() {
        app.set_status("busy");
        return false;
    }
    app.set_status(status);
    *worker = Some(thread::spawn(job));
    false
}

fn finish_worker(worker: &mut Option<Worker>, app: &mut WorkspaceApp) {
    if worker
        .as_ref()
        .map(|job| job.is_finished())
        .unwrap_or(false)
    {
        finish_joined(worker.take().expect("checked worker presence"), app);
    }
}

fn finish_worker_blocking(
    worker: &mut Option<Worker>,
    app: &mut WorkspaceApp,
    stdout: &mut impl Write,
    load_state: &impl Fn() -> WorkspaceState,
) -> Result<()> {
    if let Some(job) = worker.take() {
        finish_joined(job, app);
        draw_app(stdout, &load_state(), app)?;
    }
    Ok(())
}

fn finish_joined(job: Worker, app: &mut WorkspaceApp) {
    match job.join() {
        Ok(Ok(output)) => {
            app.set_status(compact_status(&output));
            if output.trim().is_empty() {
                app.clear_output();
            } else {
                app.set_output(output);
            }
        }
        Ok(Err(error)) => app.set_status(format!("error: {error}")),
        Err(_) => app.set_status("error: action stopped"),
    }
}

fn compact_status(output: &str) -> String {
    output
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .unwrap_or_else(|| "complete".into())
}

struct TerminalSession {
    raw_enabled: bool,
}

impl TerminalSession {
    fn enter(stdout: &mut impl Write) -> Result<Self> {
        let raw_enabled = io::stdin().is_terminal() && terminal::enable_raw_mode().is_ok();
        execute!(stdout, EnterAlternateScreen, Hide)?;
        Ok(Self { raw_enabled })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, Show, LeaveAlternateScreen);
        if self.raw_enabled {
            let _ = terminal::disable_raw_mode();
        }
    }
}

fn draw_app(stdout: &mut impl Write, state: &WorkspaceState, app: &WorkspaceApp) -> Result<()> {
    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    write!(stdout, "{}", render_app(state, app))?;
    stdout.flush()?;
    Ok(())
}

fn draw_workspace(stdout: &mut impl Write, state: &WorkspaceState) -> Result<()> {
    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    write!(
        stdout,
        "{}\nq quit | Esc close | Ctrl-C cancel\n",
        render_workspace(state)
    )?;
    stdout.flush()?;
    Ok(())
}

fn read_until_quit() -> Result<()> {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut byte = [0_u8; 1];
    loop {
        if stdin.read(&mut byte)? == 0 {
            return Ok(());
        }
        if matches!(byte[0], b'q' | b'Q' | 0x1b | 0x03) {
            return Ok(());
        }
    }
}
