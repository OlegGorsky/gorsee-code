use std::{
    io::{self, IsTerminal, Write},
    time::Duration,
};

use anyhow::{anyhow, Result};
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::Print,
    terminal::{disable_raw_mode, enable_raw_mode},
};

const ENABLE_MOUSE_CAPTURE: &str = "\x1b[?1000h\x1b[?1002h\x1b[?1006h";
const DISABLE_MOUSE_CAPTURE: &str = "\x1b[?1006l\x1b[?1002l\x1b[?1000l\x1b[?1015l\x1b[?1003l";

pub(crate) fn run() -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(anyhow!("mouse-debug requires an interactive terminal"));
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, Hide, Print(ENABLE_MOUSE_CAPTURE))?;
    let _guard = MouseDebugGuard;

    writeln!(
        stdout,
        "gcode mouse-debug\r\nTERM={}\r\nClick, drag, or scroll inside this terminal. Press q or Esc to exit.\r\n",
        std::env::var("TERM").unwrap_or_else(|_| "<unset>".into())
    )?;
    stdout.flush()?;

    let mut count = 0_u64;
    loop {
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        match event::read()? {
            Event::Mouse(mouse) => {
                count += 1;
                writeln!(
                    stdout,
                    "#{count:04} mouse kind={:?} column={} row={} modifiers={:?}\r",
                    mouse.kind, mouse.column, mouse.row, mouse.modifiers
                )?;
                stdout.flush()?;
            }
            Event::Key(key)
                if key.kind == KeyEventKind::Press
                    && matches!(key.code, KeyCode::Char('q' | 'Q') | KeyCode::Esc) =>
            {
                break;
            }
            Event::Resize(width, height) => {
                writeln!(stdout, "resize width={width} height={height}\r")?;
                stdout.flush()?;
            }
            _ => {}
        }
    }

    Ok(())
}

pub(crate) fn doctor_report() -> String {
    let term = std::env::var("TERM").unwrap_or_else(|_| "<unset>".into());
    let support = if term == "dumb" || term == "<unset>" {
        "warning mouse_reporting=unlikely"
    } else {
        "check mouse_reporting=run_gcode_mouse-debug"
    };
    format!("terminal: term={term} {support}\n")
}

struct MouseDebugGuard;

impl Drop for MouseDebugGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, Print(DISABLE_MOUSE_CAPTURE), Show);
        let _ = disable_raw_mode();
    }
}
