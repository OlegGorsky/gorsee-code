use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

const MAX_PROJECT_ENTRIES: usize = 500;
const SKIPPED_DIRS: &[&str] = &[".git", ".gorsee-code", "target", "node_modules"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectEntry {
    path: PathBuf,
    depth: usize,
    is_dir: bool,
    expanded: bool,
}

impl ProjectEntry {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    pub fn is_dir(&self) -> bool {
        self.is_dir
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProjectTree {
    root: Option<PathBuf>,
    entries: Vec<ProjectEntry>,
    files: Vec<PathBuf>,
    expanded: BTreeSet<PathBuf>,
    selected: usize,
    scroll: usize,
}

impl ProjectTree {
    pub fn sync_root(&mut self, root: impl AsRef<Path>) -> Result<()> {
        let root = root.as_ref();
        if self.root.as_deref() != Some(root) {
            self.root = Some(root.to_path_buf());
            self.expanded.clear();
            self.expanded.insert(PathBuf::new());
            self.expanded.insert(PathBuf::from("src"));
            self.selected = 0;
            self.scroll = 0;
        }
        self.refresh()
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub fn entries(&self) -> &[ProjectEntry] {
        &self.entries
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
    }

    pub fn select_next(&mut self) {
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }

    pub fn ensure_selected_visible(&mut self, height: usize) {
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if height > 0 && self.selected >= self.scroll + height {
            self.scroll = self.selected + 1 - height;
        }
    }

    pub fn entry(&self, index: usize) -> Option<&ProjectEntry> {
        self.entries.get(index)
    }

    pub fn toggle_dir(&mut self, path: &Path) -> Result<()> {
        if self.expanded.contains(path) {
            self.expanded.remove(path);
        } else {
            self.expanded.insert(path.to_path_buf());
        }
        self.refresh()
    }

    fn refresh(&mut self) -> Result<()> {
        self.entries.clear();
        self.files.clear();
        let Some(root) = self.root.clone() else {
            return Ok(());
        };
        collect_entries(
            &root,
            Path::new(""),
            0,
            &self.expanded,
            &mut self.entries,
            &mut self.files,
        )?;
        self.entries.truncate(MAX_PROJECT_ENTRIES);
        self.files.truncate(MAX_PROJECT_ENTRIES);
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        Ok(())
    }
}

fn collect_entries(
    root: &Path,
    relative_dir: &Path,
    depth: usize,
    expanded: &BTreeSet<PathBuf>,
    entries: &mut Vec<ProjectEntry>,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    if entries.len() >= MAX_PROJECT_ENTRIES {
        return Ok(());
    }
    let dir = root.join(relative_dir);
    let mut children = fs::read_dir(&dir)
        .with_context(|| format!("read project directory {}", dir.display()))?
        .flatten()
        .collect::<Vec<_>>();
    children.sort_by_key(|entry| {
        let is_file = entry.file_type().map(|kind| kind.is_file()).unwrap_or(true);
        (is_file, entry.file_name())
    });

    for child in children {
        let file_type = match child.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        let name = child.file_name();
        let name = name.to_string_lossy();
        if file_type.is_dir() && SKIPPED_DIRS.contains(&name.as_ref()) {
            continue;
        }
        let relative = relative_dir.join(name.as_ref());
        if file_type.is_dir() {
            let is_expanded = expanded.contains(&relative);
            entries.push(ProjectEntry {
                path: relative.clone(),
                depth,
                is_dir: true,
                expanded: is_expanded,
            });
            if is_expanded {
                collect_entries(root, &relative, depth + 1, expanded, entries, files)?;
            }
        } else if file_type.is_file() {
            files.push(relative.clone());
            entries.push(ProjectEntry {
                path: relative,
                depth,
                is_dir: false,
                expanded: false,
            });
        }
    }
    Ok(())
}

pub fn path_label(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
