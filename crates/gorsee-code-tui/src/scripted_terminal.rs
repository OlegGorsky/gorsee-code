use std::io::{self, IsTerminal, Read, Write};

use anyhow::Result;
use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};

pub(crate) struct TerminalSession {
    raw_enabled: bool,
}

impl TerminalSession {
    pub(crate) fn enter(stdout: &mut impl Write) -> Result<Self> {
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

pub(crate) fn terminal_screen(screen: &str) -> String {
    screen.replace('\n', "\r\n")
}

pub(crate) fn read_until_quit() -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_screen_uses_carriage_returns_for_newlines() {
        assert_eq!(terminal_screen("one\ntwo\n"), "one\r\ntwo\r\n");
    }
}
