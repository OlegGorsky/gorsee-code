use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

use gorsee_code_core::Event;
use gorsee_code_safety::Redactor;
use thiserror::Error;

use crate::SessionManifest;

#[derive(Debug, Error)]
pub enum SessionStoreError {
    #[error("session io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("session json failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    root: PathBuf,
    redactor: Redactor,
}

impl SessionStore {
    pub fn new(root: impl AsRef<Path>, redactor: Redactor) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            redactor,
        }
    }

    pub fn create(&self, manifest: &SessionManifest) -> Result<PathBuf, SessionStoreError> {
        let dir = self.session_dir(&manifest.id);
        fs::create_dir_all(dir.join("artifacts"))?;
        fs::create_dir_all(dir.join("patches"))?;
        self.write_manifest(manifest)?;
        Ok(dir)
    }

    pub fn write_manifest(&self, manifest: &SessionManifest) -> Result<(), SessionStoreError> {
        let dir = self.session_dir(&manifest.id);
        fs::write(
            dir.join("manifest.json"),
            serde_json::to_string_pretty(manifest)?,
        )?;
        Ok(())
    }

    pub fn append_event(&self, event: &Event) -> Result<(), SessionStoreError> {
        let path = self.session_dir(&event.session_id).join("events.jsonl");
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        let encoded = serde_json::to_string(event)?;
        writeln!(file, "{}", self.redactor.redact(&encoded))?;
        Ok(())
    }

    pub fn read_events(&self, session_id: &str) -> Result<Vec<Event>, SessionStoreError> {
        let path = self.session_dir(session_id).join("events.jsonl");
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        BufReader::new(file)
            .lines()
            .map(|line| Ok(serde_json::from_str(&line?)?))
            .collect()
    }

    pub fn session_dir(&self, session_id: &str) -> PathBuf {
        self.root.join("sessions").join(session_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gorsee_code_core::EventKind;
    use serde_json::json;

    #[test]
    fn appends_and_reads_jsonl_events() {
        let temp = tempfile::tempdir().unwrap();
        let store = SessionStore::new(temp.path(), Redactor::default());
        let manifest = SessionManifest::new("s1", "/repo", "main");
        store.create(&manifest).unwrap();
        let event = gorsee_code_core::Event::new(
            1,
            "s1",
            None,
            EventKind::AgentMessage,
            json!({"text": "ok"}),
        );

        store.append_event(&event).unwrap();

        let events = store.read_events("s1").unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 1);
    }
}
