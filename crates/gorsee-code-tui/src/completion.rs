use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::{command_specs::command_specs, project::path_label};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Commands,
    Files,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    label: String,
    insert_text: String,
    detail: String,
}

impl CompletionItem {
    pub fn new(
        label: impl Into<String>,
        insert_text: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            insert_text: insert_text.into(),
            detail: detail.into(),
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn insert_text(&self) -> &str {
        &self.insert_text
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionMenu {
    kind: CompletionKind,
    items: Vec<CompletionItem>,
    selected: usize,
    replace: Range<usize>,
}

impl CompletionMenu {
    pub fn kind(&self) -> CompletionKind {
        self.kind
    }

    pub fn items(&self) -> &[CompletionItem] {
        &self.items
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn selected_item(&self) -> Option<&CompletionItem> {
        self.items.get(self.selected)
    }

    pub fn select(&mut self, index: usize) {
        self.selected = index.min(self.items.len().saturating_sub(1));
    }

    pub fn would_change_input(&self, input: &str) -> bool {
        self.selected_item()
            .map(|item| input.get(self.replace.clone()) != Some(item.insert_text()))
            .unwrap_or(false)
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn select_next(&mut self) {
        if self.selected + 1 < self.items.len() {
            self.selected += 1;
        }
    }

    pub fn accept(&self, input: &mut String, cursor: &mut usize) -> bool {
        let Some(item) = self.selected_item() else {
            return false;
        };
        input.replace_range(self.replace.clone(), item.insert_text());
        *cursor = self.replace.start + item.insert_text().len();
        true
    }
}

pub fn completion_for(input: &str, cursor: usize, files: &[PathBuf]) -> Option<CompletionMenu> {
    let cursor = cursor.min(input.len());
    let token = current_token(input, cursor)?;
    if token.text.starts_with('/') {
        return command_completion(token, files);
    }
    if token.text.starts_with('@') {
        return file_completion(token, files);
    }
    None
}

struct Token<'a> {
    text: &'a str,
    range: Range<usize>,
}

fn current_token(input: &str, cursor: usize) -> Option<Token<'_>> {
    let start = input[..cursor]
        .char_indices()
        .rev()
        .find(|(_, ch)| ch.is_whitespace())
        .map(|(index, ch)| index + ch.len_utf8())
        .unwrap_or(0);
    let text = &input[start..cursor];
    (!text.is_empty()).then_some(Token {
        text,
        range: start..cursor,
    })
}

fn command_completion(token: Token<'_>, _files: &[PathBuf]) -> Option<CompletionMenu> {
    let query = token.text.trim_start_matches('/');
    let items = command_specs()
        .iter()
        .filter(|spec| spec.name.starts_with(query))
        .map(|spec| {
            CompletionItem::new(
                format!("/{}", spec.name),
                format!("/{}", spec.name),
                spec.description,
            )
        })
        .collect::<Vec<_>>();
    menu(CompletionKind::Commands, items, token.range)
}

fn file_completion(token: Token<'_>, files: &[PathBuf]) -> Option<CompletionMenu> {
    let query = token.text.trim_start_matches('@');
    let items = files
        .iter()
        .filter(|path| matches_query(path, query))
        .map(|path| {
            let label = path_label(path);
            CompletionItem::new(label.clone(), format!("@{label}"), "файл проекта")
        })
        .collect::<Vec<_>>();
    menu(CompletionKind::Files, items, token.range)
}

fn matches_query(path: &Path, query: &str) -> bool {
    query.is_empty() || path_label(path).contains(query)
}

fn menu(
    kind: CompletionKind,
    items: Vec<CompletionItem>,
    replace: Range<usize>,
) -> Option<CompletionMenu> {
    (!items.is_empty()).then_some(CompletionMenu {
        kind,
        items,
        selected: 0,
        replace,
    })
}
