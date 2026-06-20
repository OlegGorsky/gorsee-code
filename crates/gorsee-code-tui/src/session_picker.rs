use std::{cmp::Ordering, fs, path::Path};

use anyhow::{Context, Result};

pub(crate) fn session_ids(root: &Path) -> Result<Vec<String>> {
    let dir = root.join(".gorsee-code").join("sessions");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).with_context(|| format!("read {}", dir.display())),
    };
    let mut sessions = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .filter_map(|entry| {
            let id = entry.file_name().into_string().ok()?;
            let meta = session_meta(&entry.path(), &id);
            Some((id, meta))
        })
        .collect::<Vec<_>>();
    sessions.sort_by(|left, right| compare_sessions(left, right).reverse());
    Ok(sessions.into_iter().map(|(id, _)| id).collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionMeta {
    status: String,
    started_at: String,
}

fn session_meta(dir: &Path, id: &str) -> SessionMeta {
    let value = fs::read_to_string(dir.join("manifest.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .unwrap_or_default();
    SessionMeta {
        status: value
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        started_at: value
            .get("started_at")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(id)
            .to_string(),
    }
}

fn compare_sessions(left: &(String, SessionMeta), right: &(String, SessionMeta)) -> Ordering {
    active_rank(&left.1.status)
        .cmp(&active_rank(&right.1.status))
        .then_with(|| left.1.started_at.cmp(&right.1.started_at))
        .then_with(|| left.0.cmp(&right.0))
}

fn active_rank(status: &str) -> u8 {
    match status {
        "running" | "waiting_approval" | "paused" => 1,
        _ => 0,
    }
}
