use std::path::{Component, Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PathPolicyError {
    #[error("path escapes workspace: {0}")]
    EscapesWorkspace(String),
    #[error("path does not exist: {0}")]
    Missing(String),
}

#[derive(Debug, Clone)]
pub struct PathPolicy {
    root: PathBuf,
}

impl PathPolicy {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, PathPolicyError> {
        let root = canonical_existing(root.as_ref())?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn resolve_existing(&self, path: impl AsRef<Path>) -> Result<PathBuf, PathPolicyError> {
        let absolute = self.root.join(path.as_ref());
        let canonical = canonical_existing(&absolute)?;
        self.ensure_inside(canonical)
    }

    pub fn resolve_for_write(&self, path: impl AsRef<Path>) -> Result<PathBuf, PathPolicyError> {
        let relative = normalize_relative(path.as_ref())?;
        let absolute = self.root.join(relative);
        let ancestor = nearest_existing_ancestor(absolute.parent().unwrap_or(&self.root))?;
        self.ensure_inside(ancestor)?;
        Ok(absolute)
    }

    fn ensure_inside(&self, path: PathBuf) -> Result<PathBuf, PathPolicyError> {
        if path.starts_with(&self.root) {
            return Ok(path);
        }
        Err(PathPolicyError::EscapesWorkspace(
            path.display().to_string(),
        ))
    }
}

fn canonical_existing(path: &Path) -> Result<PathBuf, PathPolicyError> {
    path.canonicalize()
        .map_err(|_| PathPolicyError::Missing(path.display().to_string()))
}

fn normalize_relative(path: &Path) -> Result<PathBuf, PathPolicyError> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(PathPolicyError::EscapesWorkspace(
                        path.display().to_string(),
                    ));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(PathPolicyError::EscapesWorkspace(
                    path.display().to_string(),
                ));
            }
        }
    }
    Ok(normalized)
}

fn nearest_existing_ancestor(path: &Path) -> Result<PathBuf, PathPolicyError> {
    let mut ancestor = path;
    loop {
        if ancestor.exists() {
            return canonical_existing(ancestor);
        }
        ancestor = ancestor
            .parent()
            .ok_or_else(|| PathPolicyError::Missing(path.display().to_string()))?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rejects_parent_escape() {
        let temp = tempfile::tempdir().unwrap();
        let policy = PathPolicy::new(temp.path()).unwrap();
        let outside = temp.path().parent().unwrap().join("outside.txt");
        fs::write(&outside, "x").unwrap();

        let error = policy.resolve_existing("../outside.txt").unwrap_err();
        assert!(matches!(error, PathPolicyError::EscapesWorkspace(_)));
        let _ = fs::remove_file(outside);
    }

    #[test]
    fn write_paths_can_create_new_nested_files_inside_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let policy = PathPolicy::new(temp.path()).unwrap();

        let target = policy.resolve_for_write("src/generated/lib.rs").unwrap();

        assert_eq!(target, temp.path().join("src/generated/lib.rs"));
    }
}
