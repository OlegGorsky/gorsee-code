use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("artifact io failed: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub id: String,
    pub path: String,
    pub mime: String,
}

#[derive(Debug, Clone)]
pub struct ArtifactStore {
    root: PathBuf,
}

impl ArtifactStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn write_text(
        &self,
        mime: impl Into<String>,
        text: &str,
    ) -> Result<ArtifactRecord, ArtifactError> {
        fs::create_dir_all(&self.root)?;
        let id = Uuid::new_v4().to_string();
        let mime = mime.into();
        let path = self
            .root
            .join(format!("{id}.{}", extension_for_mime(&mime)));
        fs::write(&path, text)?;
        Ok(ArtifactRecord {
            id,
            path: path.display().to_string(),
            mime,
        })
    }

    pub fn write_named_text(
        &self,
        name: &str,
        mime: impl Into<String>,
        text: &str,
    ) -> Result<ArtifactRecord, ArtifactError> {
        fs::create_dir_all(&self.root)?;
        let mime = mime.into();
        let path = self.root.join(name);
        fs::write(&path, text)?;
        Ok(ArtifactRecord {
            id: artifact_id(&path),
            path: path.display().to_string(),
            mime,
        })
    }
}

fn extension_for_mime(mime: &str) -> &'static str {
    match mime {
        "text/markdown" => "md",
        "application/json" => "json",
        "application/x-ndjson" => "jsonl",
        "text/x-diff" => "patch",
        _ => "txt",
    }
}

fn artifact_id(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_text_artifact() {
        let temp = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(temp.path());
        let record = store.write_text("text/plain", "hello").unwrap();
        assert!(Path::new(&record.path).exists());
        assert!(record.path.ends_with(".txt"));
    }

    #[test]
    fn writes_markdown_artifact_with_markdown_extension() {
        let temp = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(temp.path());
        let record = store.write_text("text/markdown", "# hello").unwrap();
        assert!(Path::new(&record.path).exists());
        assert!(record.path.ends_with(".md"));
    }

    #[test]
    fn writes_named_text_artifact() {
        let temp = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(temp.path());
        let record = store
            .write_named_text("usage.json", "application/json", "{}")
            .unwrap();

        assert_eq!(record.id, "usage");
        assert!(Path::new(&record.path).exists());
        assert!(record.path.ends_with("usage.json"));
    }
}
