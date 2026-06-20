use std::{
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Result;
use crossterm::event::{self, Event};
use gorsee_code_ui_state::WorkspaceState;
use ratatui::layout::Rect;

use crate::{
    action_for_key,
    input::action_for_byte,
    runtime_draw::{draw_app, draw_interactive_app, draw_workspace},
    runtime_jobs::{
        action_root, finish_worker, finish_worker_blocking, handle_interactive_intent,
        process_intent, Worker,
    },
    scripted_terminal::{read_until_quit, TerminalSession},
    terminal::{restore_terminal, setup_terminal, TuiTerminal},
    TuiHandlers, WorkspaceApp,
};

#[cfg(test)]
use crate::runtime_jobs::finish_joined;

pub fn run_app(
    initial_root: PathBuf,
    load_state: impl Fn(&Path, Option<&str>) -> WorkspaceState,
    handlers: TuiHandlers,
) -> Result<()> {
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    let mut app = WorkspaceApp::new();
    let mut worker = None;

    if interactive {
        let mut terminal = setup_terminal()?;
        let result = run_interactive_loop(
            &mut terminal,
            &mut app,
            &mut worker,
            &handlers,
            &load_state,
            &initial_root,
        );
        let restore_result = restore_terminal(&mut terminal);
        result.and(restore_result)
    } else {
        let mut stdout = io::stdout();
        let _terminal = TerminalSession::enter(&mut stdout)?;
        run_scripted_loop(
            &mut stdout,
            &mut app,
            &mut worker,
            &handlers,
            &load_state,
            &initial_root,
        )
    }
}

pub fn run_workspace(state: &WorkspaceState) -> Result<()> {
    let mut stdout = io::stdout();
    let _terminal = TerminalSession::enter(&mut stdout)?;
    draw_workspace(&mut stdout, state)?;
    read_until_quit()
}

fn run_interactive_loop(
    terminal: &mut TuiTerminal,
    app: &mut WorkspaceApp,
    worker: &mut Option<Worker>,
    handlers: &TuiHandlers,
    load_state: &impl Fn(&Path, Option<&str>) -> WorkspaceState,
    initial_root: &Path,
) -> Result<()> {
    loop {
        let root = action_root(app, initial_root);
        let state = load_state(&root, app.active_session_id());
        sync_project_root(app, &root);
        finish_worker(worker, app);
        if worker.is_some() {
            app.advance_spinner();
        }
        draw_interactive_app(terminal, &state, app)?;
        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) => {
                    let action = action_for_key(key, app.input().is_empty());
                    if process_intent(
                        app.handle_action(action, &state),
                        worker,
                        handlers,
                        app,
                        &root,
                    ) {
                        return Ok(());
                    }
                }
                Event::Mouse(mouse) => {
                    let size = terminal.size()?;
                    let area = Rect::new(0, 0, size.width, size.height);
                    let intent = app.handle_mouse(mouse, area, &state);
                    if handle_interactive_intent(terminal, intent, worker, handlers, app, &root)? {
                        return Ok(());
                    }
                }
                Event::Paste(text) => {
                    handle_paste(app, &state, &text);
                }
                _ => {}
            }
        }
    }
}

fn run_scripted_loop(
    stdout: &mut impl Write,
    app: &mut WorkspaceApp,
    worker: &mut Option<Worker>,
    handlers: &TuiHandlers,
    load_state: &impl Fn(&Path, Option<&str>) -> WorkspaceState,
    initial_root: &Path,
) -> Result<()> {
    sync_project_root(app, initial_root);
    draw_app(stdout, &load_state(initial_root, None), app)?;
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut byte = [0_u8; 1];
    loop {
        match stdin.read(&mut byte)? {
            0 => return finish_worker_blocking(worker, app, stdout, load_state, initial_root),
            _ => {
                let root = action_root(app, initial_root);
                let state = load_state(&root, app.active_session_id());
                let action = action_for_byte(byte[0], app.input().is_empty());
                if process_intent(
                    app.handle_action(action, &state),
                    worker,
                    handlers,
                    app,
                    &root,
                ) {
                    return Ok(());
                }
                finish_worker(worker, app);
                draw_app(stdout, &state, app)?;
            }
        }
    }
}

fn sync_project_root(app: &mut WorkspaceApp, root: &Path) {
    if let Err(error) = app.sync_project_root(root) {
        app.set_status(format!("project scan failed: {error}"));
    }
}

fn handle_paste(app: &mut WorkspaceApp, state: &WorkspaceState, text: &str) {
    app.paste_text(text, state);
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
