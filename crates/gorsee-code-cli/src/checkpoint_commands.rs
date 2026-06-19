use std::{
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use gorsee_code_core::{Event, EventKind};
use gorsee_code_safety::Redactor;
use gorsee_code_session::{SessionManifest, SessionStore};
use serde_json::json;

use crate::{commands_extra::session_ids, paths};

pub fn save(root: &Path) -> Result<String> {
    paths::ensure_layout(root)?;
    let store = SessionStore::new(paths::local_dir(root), Redactor::default());
    let id = match session_ids(root)?.into_iter().max() {
        Some(id) => id,
        None => create_session(root, &store)?,
    };

    let mut manifest = store.read_manifest(&id).context("read session manifest")?;
    let events = store.read_events(&id).context("read session events")?;
    let event = Event::new(
        events.len() as u64 + 1,
        &id,
        None,
        EventKind::SessionPaused,
        json!({ "message": "checkpoint saved" }),
    );
    store
        .append_event(&event)
        .context("append checkpoint event")?;
    manifest.status = "paused".into();
    store
        .write_manifest(&manifest)
        .context("write session manifest")?;

    Ok(format!("checkpoint: {id}\nevent: session_paused\n"))
}

fn create_session(root: &Path, store: &SessionStore) -> Result<String> {
    let id = format!("checkpoint-{}", unix_seconds()?);
    let manifest = SessionManifest::new(&id, root.display().to_string(), current_branch(root));
    store
        .create(&manifest)
        .context("create session checkpoint")?;
    Ok(id)
}

fn unix_seconds() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("read system clock")?
        .as_secs())
}

fn current_branch(root: &Path) -> String {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|branch| branch.trim().to_string())
        .filter(|branch| !branch.is_empty())
        .unwrap_or_else(|| "unknown".into())
}
