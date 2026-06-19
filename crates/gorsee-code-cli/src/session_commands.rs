use std::path::Path;

use anyhow::Result;
use gorsee_code_core::{Event, EventKind};
use gorsee_code_safety::Redactor;
use gorsee_code_session::SessionStore;
use serde_json::json;

use crate::{
    args::SessionIdArgs,
    commands_extra::{read_manifest, resolve_session_id},
    paths,
};

pub fn pause(root: &Path, args: SessionIdArgs) -> Result<String> {
    transition(root, args, "paused", EventKind::SessionPaused, "pause")
}

pub fn resume(root: &Path, args: SessionIdArgs) -> Result<String> {
    transition(root, args, "running", EventKind::SessionResumed, "resume")
}

fn transition(
    root: &Path,
    args: SessionIdArgs,
    status: &str,
    kind: EventKind,
    label: &str,
) -> Result<String> {
    let id = resolve_session_id(root, args.session_id)?;
    let store = SessionStore::new(paths::local_dir(root), Redactor::default());
    let events = store.read_events(&id)?;
    let mut manifest = read_manifest(root, &id)?;
    let event_name = event_label(&kind);
    let event = Event::new(
        events.len() as u64 + 1,
        &id,
        None,
        kind,
        json!({ "message": event_name }),
    );
    store.append_event(&event)?;
    manifest.status = status.into();
    store.write_manifest(&manifest)?;
    Ok(format!("{label}: {id}\nevent: {event_name}\n"))
}

fn event_label(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::SessionPaused => "session_paused",
        EventKind::SessionResumed => "session_resumed",
        _ => "session_event",
    }
}
