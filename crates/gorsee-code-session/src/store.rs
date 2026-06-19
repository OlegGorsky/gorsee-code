use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use gorsee_code_core::Event;
use gorsee_code_safety::Redactor;
use serde_json::json;
use thiserror::Error;

use crate::{ApprovalDecision, ApprovalRecord, ApprovalStatus, SessionManifest};

#[derive(Debug, Error)]
pub enum SessionStoreError {
    #[error("session io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("session json failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("approval not found: {0}")]
    ApprovalNotFound(String),
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
        self.write_session_note(manifest)?;
        self.touch_jsonl(&manifest.id, "events.jsonl")?;
        self.touch_jsonl(&manifest.id, "messages.jsonl")?;
        self.touch_jsonl(&manifest.id, "approvals.jsonl")?;
        self.touch_jsonl(&manifest.id, "tool-calls.jsonl")?;
        self.write_json_file(&manifest.id, "token-ledger.json", &json!({ "records": [] }))?;
        self.write_json_file(
            &manifest.id,
            "context-map.json",
            &json!({
                "repo": manifest.repo,
                "branch": manifest.branch,
                "files": [],
                "symbols": [],
                "decisions": []
            }),
        )?;
        Ok(dir)
    }

    pub fn write_manifest(&self, manifest: &SessionManifest) -> Result<(), SessionStoreError> {
        let dir = self.session_dir(&manifest.id);
        write_atomic(
            &dir.join("manifest.json"),
            serde_json::to_string_pretty(manifest)?,
        )?;
        Ok(())
    }

    pub fn read_manifest(&self, session_id: &str) -> Result<SessionManifest, SessionStoreError> {
        let path = self.session_dir(session_id).join("manifest.json");
        Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
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
        read_jsonl(path)
    }

    pub fn append_approval(&self, approval: &ApprovalRecord) -> Result<(), SessionStoreError> {
        let path = self
            .session_dir(&approval.session_id)
            .join("approvals.jsonl");
        self.append_jsonl(path, approval)
    }

    pub fn read_approvals(
        &self,
        session_id: &str,
    ) -> Result<Vec<ApprovalRecord>, SessionStoreError> {
        read_jsonl(self.session_dir(session_id).join("approvals.jsonl"))
    }

    pub fn pending_approvals(
        &self,
        session_id: &str,
    ) -> Result<Vec<ApprovalRecord>, SessionStoreError> {
        Ok(self
            .read_approvals(session_id)?
            .into_iter()
            .filter(|approval| approval.status == ApprovalStatus::Pending)
            .collect())
    }

    pub fn decide_approval(
        &self,
        session_id: &str,
        approval_id: &str,
        decision: ApprovalDecision,
    ) -> Result<ApprovalRecord, SessionStoreError> {
        let mut approvals = self.read_approvals(session_id)?;
        let approval = approvals
            .iter_mut()
            .find(|approval| approval.id == approval_id)
            .ok_or_else(|| SessionStoreError::ApprovalNotFound(approval_id.into()))?;
        approval.decide(decision);
        let decided = approval.clone();
        self.write_approvals(session_id, &approvals)?;
        Ok(decided)
    }

    pub fn session_dir(&self, session_id: &str) -> PathBuf {
        self.root.join("sessions").join(session_id)
    }

    fn append_jsonl<T: serde::Serialize>(
        &self,
        path: PathBuf,
        value: &T,
    ) -> Result<(), SessionStoreError> {
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        let encoded = serde_json::to_string(value)?;
        writeln!(file, "{}", self.redactor.redact(&encoded))?;
        Ok(())
    }

    fn touch_jsonl(&self, session_id: &str, name: &str) -> Result<(), SessionStoreError> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.session_dir(session_id).join(name))?;
        Ok(())
    }

    fn write_session_note(&self, manifest: &SessionManifest) -> Result<(), SessionStoreError> {
        let content = format!(
            "# Gorsee Code Session\n\nRepository: {}\nBranch: {}\nStatus: {}\n",
            manifest.repo, manifest.branch, manifest.status
        );
        write_atomic(&self.session_dir(&manifest.id).join("session.md"), content)?;
        Ok(())
    }

    fn write_json_file<T: serde::Serialize>(
        &self,
        session_id: &str,
        name: &str,
        value: &T,
    ) -> Result<(), SessionStoreError> {
        let path = self.session_dir(session_id).join(name);
        write_atomic(&path, serde_json::to_string_pretty(value)?)?;
        Ok(())
    }

    fn write_approvals(
        &self,
        session_id: &str,
        approvals: &[ApprovalRecord],
    ) -> Result<(), SessionStoreError> {
        let path = self.session_dir(session_id).join("approvals.jsonl");
        let mut content = String::new();
        for approval in approvals {
            let encoded = serde_json::to_string(approval)?;
            content.push_str(&self.redactor.redact(&encoded));
            content.push('\n');
        }
        write_atomic(&path, content)?;
        Ok(())
    }
}

fn write_atomic(path: &Path, content: impl AsRef<str>) -> std::io::Result<()> {
    let temp = temp_path(path);
    if let Err(error) = fs::write(&temp, content.as_ref()) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    Ok(())
}

fn temp_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    parent.join(format!(".{name}.tmp-{}-{now}", std::process::id()))
}

fn read_jsonl<T: serde::de::DeserializeOwned>(path: PathBuf) -> Result<Vec<T>, SessionStoreError> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    BufReader::new(file)
        .lines()
        .filter(|line| !line.as_ref().is_ok_and(|line| line.trim().is_empty()))
        .map(|line| Ok(serde_json::from_str(&line?)?))
        .collect()
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

    #[test]
    fn atomic_write_replaces_file_without_leaving_tempfile() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("manifest.json");
        fs::write(&path, "old").unwrap();

        write_atomic(&path, "new").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        let leftovers = fs::read_dir(temp.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
            .count();
        assert_eq!(leftovers, 0);
    }
}
