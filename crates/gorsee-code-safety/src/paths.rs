use std::path::{Path, PathBuf};

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
        let absolute = self.root.join(path.as_ref());
        let parent = absolute.parent().unwrap_or(&self.root);
        let parent = canonical_existing(parent)?;
        self.ensure_inside(parent)?;
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
}
