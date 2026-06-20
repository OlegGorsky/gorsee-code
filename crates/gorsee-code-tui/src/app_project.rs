use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::{editor::EditorBuffer, project::ProjectEntry, WorkspaceApp};

impl WorkspaceApp {
    pub fn sync_project_root(&mut self, root: impl AsRef<Path>) -> Result<()> {
        self.project.sync_root(root)?;
        self.refresh_completion();
        Ok(())
    }

    pub fn choose_working_folder(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let next = resolve_working_folder(self.project.root(), path.as_ref());
        self.project
            .sync_root(&next)
            .with_context(|| format!("open working folder {}", next.display()))?;
        self.editor = None;
        self.active_session_id = None;
        self.sessions.clear();
        self.selected_session = 0;
        self.confirm_close_editor = false;
        self.clear_output();
        self.completion = None;
        self.set_status(format!("рабочая папка: {}", next.display()));
        Ok(())
    }

    pub fn working_folder(&self) -> Option<&Path> {
        self.project.root()
    }

    pub(crate) fn choose_selected_folder(&mut self) -> Result<()> {
        let Some(entry) = self.project.entry(self.project.selected()).cloned() else {
            return Ok(());
        };
        if entry.is_dir() {
            self.choose_working_folder(entry.path())?;
        }
        Ok(())
    }

    pub(crate) fn choose_parent_folder(&mut self) -> Result<()> {
        let Some(root) = self.project.root().map(Path::to_path_buf) else {
            return Ok(());
        };
        let Some(parent) = root.parent().map(Path::to_path_buf) else {
            return Ok(());
        };
        self.project
            .sync_root(&parent)
            .with_context(|| format!("open working folder {}", parent.display()))?;
        self.editor = None;
        self.active_session_id = None;
        self.sessions.clear();
        self.selected_session = 0;
        self.confirm_close_editor = false;
        self.clear_output();
        self.completion = None;
        self.set_status(format!("рабочая папка: {}", parent.display()));
        Ok(())
    }

    pub fn project_entries(&self) -> &[ProjectEntry] {
        self.project.entries()
    }

    pub fn project_selected(&self) -> usize {
        self.project.selected()
    }

    pub fn project_scroll(&self) -> usize {
        self.project.scroll()
    }

    pub fn open_project_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let root = self
            .project
            .root()
            .ok_or_else(|| anyhow::anyhow!("project root is not ready"))?;
        self.editor = Some(EditorBuffer::open(root, path.as_ref())?);
        self.confirm_close_editor = false;
        self.clear_output();
        self.completion = None;
        self.set_status(format!("opened {}", path.as_ref().display()));
        Ok(())
    }

    pub fn editor(&self) -> Option<&EditorBuffer> {
        self.editor.as_ref()
    }

    pub fn is_editor_open(&self) -> bool {
        self.editor.is_some()
    }
}

fn resolve_working_folder(current_root: Option<&Path>, input: &Path) -> PathBuf {
    let input = expand_home(input);
    if input.is_absolute() {
        return input;
    }
    for candidate in folder_candidates(current_root, &input) {
        if candidate.is_dir() {
            return candidate;
        }
    }
    current_root.map(|root| root.join(&input)).unwrap_or(input)
}

fn folder_candidates(current_root: Option<&Path>, input: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(root) = current_root {
        candidates.push(root.join(input));
        if let Some(parent) = root.parent() {
            candidates.push(parent.join(input));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(input));
    }
    if let Some(home) = home_dir() {
        candidates.push(home.join(input));
    }
    candidates.push(input.to_path_buf());
    candidates
}

fn expand_home(input: &Path) -> PathBuf {
    let Some(text) = input.to_str() else {
        return input.to_path_buf();
    };
    if text == "~" {
        return home_dir().unwrap_or_else(|| input.to_path_buf());
    }
    if let Some(rest) = text.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    input.to_path_buf()
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
