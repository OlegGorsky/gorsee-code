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
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
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
        Ok("force")
    )
}

fn capture_error(first_error: &mut Option<io::Error>, result: io::Result<()>) {
    if first_error.is_none() {
        if let Err(error) = result {
            *first_error = Some(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn keyboard_protocol_true_is_ignored_for_terminal_compatibility() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = env::var("GORSEE_TUI_KEYBOARD_PROTOCOL").ok();
        env::set_var("GORSEE_TUI_KEYBOARD_PROTOCOL", "true");

        assert!(!keyboard_enhancement_requested());

        restore_env(previous);
    }

    #[test]
    fn keyboard_protocol_force_is_explicit_opt_in() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = env::var("GORSEE_TUI_KEYBOARD_PROTOCOL").ok();
        env::set_var("GORSEE_TUI_KEYBOARD_PROTOCOL", "force");

        assert!(keyboard_enhancement_requested());

        restore_env(previous);
    }

    fn restore_env(previous: Option<String>) {
        match previous {
            Some(value) => env::set_var("GORSEE_TUI_KEYBOARD_PROTOCOL", value),
            None => env::remove_var("GORSEE_TUI_KEYBOARD_PROTOCOL"),
        }
    }
}
