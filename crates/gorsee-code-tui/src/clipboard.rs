use anyhow::Result;
use crossterm::{execute, style::Print};

use crate::terminal::TuiTerminal;

pub(crate) fn write_clipboard_osc52(terminal: &mut TuiTerminal, text: &str) -> Result<()> {
    execute!(
        terminal.backend_mut(),
        Print(format!("\x1b]52;c;{}\x07", base64_bytes(text.as_bytes())))
    )?;
    Ok(())
}

pub(crate) fn base64_bytes(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encoder_supports_clipboard_payloads() {
        assert_eq!(
            base64_bytes("Скопировано!".as_bytes()),
            "0KHQutC+0L/QuNGA0L7QstCw0L3QviE="
        );
    }
}
