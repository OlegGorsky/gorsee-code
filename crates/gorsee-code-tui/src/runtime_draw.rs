use std::io::Write;

use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    execute,
    terminal::{Clear, ClearType},
};
use gorsee_code_ui_state::WorkspaceState;

use crate::{
    render_app, render_frame, render_workspace, scripted_terminal::terminal_screen,
    terminal::TuiTerminal, WorkspaceApp,
};

pub(crate) fn draw_app(
    stdout: &mut impl Write,
    state: &WorkspaceState,
    app: &WorkspaceApp,
) -> Result<()> {
    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    write!(stdout, "{}", terminal_screen(&render_app(state, app)))?;
    stdout.flush()?;
    Ok(())
}

pub(crate) fn draw_interactive_app(
    terminal: &mut TuiTerminal,
    state: &WorkspaceState,
    app: &WorkspaceApp,
) -> Result<()> {
    terminal.draw(|frame| render_frame(frame, state, app))?;
    Ok(())
}

pub(crate) fn draw_workspace(stdout: &mut impl Write, state: &WorkspaceState) -> Result<()> {
    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    let screen = format!(
        "{}\nq quit | Esc close | Ctrl-C cancel\n",
        render_workspace(state)
    );
    write!(stdout, "{}", terminal_screen(&screen))?;
    stdout.flush()?;
    Ok(())
}
