use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Insert(char),
    Backspace,
    MoveLeft,
    MoveRight,
    MoveSelectionUp,
    MoveSelectionDown,
    ScrollUp,
    ScrollDown,
    Newline,
    AcceptCompletion,
    FocusNext,
    Save,
    CloseEditor,
    Submit,
    Approve,
    Deny,
    Pause,
    Resume,
    Quit,
    Ignore,
}

pub fn action_for_key(key: KeyEvent, prompt_empty: bool) -> KeyAction {
    if is_ctrl_c(key) {
        return KeyAction::Quit;
    }
    match key.code {
        KeyCode::Esc => KeyAction::Quit,
        KeyCode::Enter
            if key.modifiers.contains(KeyModifiers::SHIFT)
                || key.modifiers.contains(KeyModifiers::ALT) =>
        {
            KeyAction::Newline
        }
        KeyCode::Enter => KeyAction::Submit,
        KeyCode::Backspace => KeyAction::Backspace,
        KeyCode::Left => KeyAction::MoveLeft,
        KeyCode::Right => KeyAction::MoveRight,
        KeyCode::Up => KeyAction::MoveSelectionUp,
        KeyCode::Down => KeyAction::MoveSelectionDown,
        KeyCode::PageUp => KeyAction::ScrollUp,
        KeyCode::PageDown => KeyAction::ScrollDown,
        KeyCode::Tab => KeyAction::FocusNext,
        KeyCode::Char('s' | 'S') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            KeyAction::Save
        }
        KeyCode::Char('w' | 'W') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            KeyAction::CloseEditor
        }
        KeyCode::Char('j' | 'J') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            KeyAction::Newline
        }
        KeyCode::Char(value) => action_for_char(value, prompt_empty),
        _ => KeyAction::Ignore,
    }
}

pub(crate) fn action_for_byte(byte: u8, prompt_empty: bool) -> KeyAction {
    match byte {
        b'\n' | b'\r' => KeyAction::Submit,
        0x08 | 0x7f => KeyAction::Backspace,
        0x03 | 0x1b => KeyAction::Quit,
        value if value.is_ascii_graphic() || value == b' ' => {
            action_for_char(value as char, prompt_empty)
        }
        _ => KeyAction::Ignore,
    }
}

fn is_ctrl_c(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
}

fn action_for_char(value: char, prompt_empty: bool) -> KeyAction {
    if !prompt_empty {
        return KeyAction::Insert(value);
    }
    match value.to_ascii_lowercase() {
        'q' => KeyAction::Quit,
        _ => KeyAction::Insert(value),
    }
}
