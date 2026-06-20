use std::{
    env, io,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use crossterm::{
    cursor::{Hide, Show},
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, Terminal};

pub type TuiTerminal = Terminal<CrosstermBackend<io::Stdout>>;

static KEYBOARD_ENHANCEMENT_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn setup_terminal() -> Result<TuiTerminal> {
    let keyboard_enhancement = keyboard_enhancement_requested();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if keyboard_enhancement {
        execute!(
            stdout,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
    }
    KEYBOARD_ENHANCEMENT_ENABLED.store(keyboard_enhancement, Ordering::Relaxed);
    execute!(
        stdout,
        EnterAlternateScreen,
        Hide,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    Ok(terminal)
}

pub fn restore_terminal(terminal: &mut TuiTerminal) -> Result<()> {
    let mut first_error: Option<io::Error> = None;
    if KEYBOARD_ENHANCEMENT_ENABLED.swap(false, Ordering::Relaxed) {
        capture_error(
            &mut first_error,
            execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags),
        );
    }
    capture_error(
        &mut first_error,
        execute!(
            terminal.backend_mut(),
            DisableMouseCapture,
            DisableBracketedPaste,
            Show,
            LeaveAlternateScreen
        ),
    );
    capture_error(&mut first_error, disable_raw_mode());
    capture_error(&mut first_error, terminal.show_cursor());
    match first_error {
        Some(error) => Err(error.into()),
        None => Ok(()),
    }
}

fn keyboard_enhancement_requested() -> bool {
    matches!(
        env::var("GORSEE_TUI_KEYBOARD_PROTOCOL").as_deref(),
        Ok("1" | "true")
    ) && supports_keyboard_enhancement().unwrap_or(false)
}

fn capture_error(first_error: &mut Option<io::Error>, result: io::Result<()>) {
    if first_error.is_none() {
        if let Err(error) = result {
            *first_error = Some(error);
        }
    }
}
