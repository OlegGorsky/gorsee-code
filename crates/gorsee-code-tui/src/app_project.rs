use std::path::Path;

use anyhow::{Context, Result};

use crate::{editor::EditorBuffer, project::ProjectEntry, WorkspaceApp};

impl WorkspaceApp {
    pub fn sync_project_root(&mut self, root: impl AsRef<Path>) -> Result<()> {
        self.project.sync_root(root)?;
        self.refresh_completion();
        Ok(())
    }

    pub fn choose_working_folder(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let base = self
            .project
            .root()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| Path::new(".").to_path_buf());
        let next = if path.as_ref().is_absolute() {
            path.as_ref().to_path_buf()
        } else {
            base.join(path.as_ref())
        };
        self.project
            .sync_root(&next)
            .with_context(|| format!("open working folder {}", next.display()))?;
        self.editor = None;
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
