use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorBuffer {
    root: PathBuf,
    path: PathBuf,
    text: String,
    cursor: usize,
    scroll: usize,
    dirty: bool,
}

impl EditorBuffer {
    pub fn open(root: impl AsRef<Path>, path: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let path = path.as_ref().to_path_buf();
        let absolute = root.join(&path);
        let text = fs::read_to_string(&absolute)
            .with_context(|| format!("open file {}", absolute.display()))?;
        let cursor = text.trim_end_matches(['\r', '\n']).len();
        Ok(Self {
            root,
            path,
            text,
            cursor,
            scroll: 0,
            dirty: false,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    pub fn insert(&mut self, value: char) {
        self.cursor = clamp_to_boundary(&self.text, self.cursor);
        self.text.insert(self.cursor, value);
        self.cursor += value.len_utf8();
        self.dirty = true;
    }

    pub fn backspace(&mut self) {
        self.cursor = clamp_to_boundary(&self.text, self.cursor);
        if self.cursor == 0 {
            return;
        }
        let previous = previous_boundary(&self.text, self.cursor);
        self.text.replace_range(previous..self.cursor, "");
        self.cursor = previous;
        self.dirty = true;
    }

    pub fn move_left(&mut self) {
        self.cursor = previous_boundary(&self.text, self.cursor);
    }

    pub fn move_right(&mut self) {
        self.cursor = next_boundary(&self.text, self.cursor);
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        let max_scroll = self.text.lines().count().saturating_sub(1);
        self.scroll = (self.scroll + 1).min(max_scroll);
    }

    pub fn save(&mut self) -> Result<()> {
        let absolute = self.root.join(&self.path);
        fs::write(&absolute, &self.text).with_context(|| format!("save {}", absolute.display()))?;
        self.dirty = false;
        Ok(())
    }
}

fn clamp_to_boundary(text: &str, cursor: usize) -> usize {
    let mut cursor = cursor.min(text.len());
    while !text.is_char_boundary(cursor) {
        cursor -= 1;
    }
    cursor
}

fn previous_boundary(text: &str, cursor: usize) -> usize {
    let cursor = clamp_to_boundary(text, cursor);
    text[..cursor]
        .char_indices()
        .last()
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn next_boundary(text: &str, cursor: usize) -> usize {
    let cursor = clamp_to_boundary(text, cursor);
    text[cursor..]
        .chars()
        .next()
        .map(|ch| cursor + ch.len_utf8())
        .unwrap_or(text.len())
}
