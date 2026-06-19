use std::io::{self, IsTerminal, Write};

pub fn read_api_key() -> io::Result<String> {
    if prompt_mode(io::stdin().is_terminal(), io::stdout().is_terminal()) == PromptMode::Hidden {
        return rpassword::prompt_password("NeuroGate API key: ");
    }
    read_visible_pipe()
}

fn read_visible_pipe() -> io::Result<String> {
    print!("NeuroGate API key: ");
    io::stdout().flush()?;

    let mut key = String::new();
    io::stdin().read_line(&mut key)?;
    Ok(key)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptMode {
    Hidden,
    VisiblePipe,
}

fn prompt_mode(stdin_tty: bool, stdout_tty: bool) -> PromptMode {
    if stdin_tty && stdout_tty {
        PromptMode::Hidden
    } else {
        PromptMode::VisiblePipe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_prompt_hides_input() {
        assert_eq!(prompt_mode(true, true), PromptMode::Hidden);
    }

    #[test]
    fn piped_prompt_keeps_automation_mode() {
        assert_eq!(prompt_mode(false, true), PromptMode::VisiblePipe);
        assert_eq!(prompt_mode(true, false), PromptMode::VisiblePipe);
    }
}
