use std::{cmp::Ordering, fs, path::Path};

use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionItem {
    id: Option<String>,
    label: String,
    detail: String,
}

impl SessionItem {
    pub(crate) fn new_session() -> Self {
        Self {
            id: None,
            label: "Новая сессия".into(),
            detail: "ввести задачу и создать запуск".into(),
        }
    }

    pub(crate) fn stored(id: String, meta: SessionMeta) -> Self {
        Self {
            id: Some(id.clone()),
            label: id,
            detail: format!("{} · {}", status_label(&meta.status), meta.started_at),
        }
    }

    pub(crate) fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) fn detail(&self) -> &str {
        &self.detail
    }
}

pub(crate) fn session_items(root: &Path) -> Result<Vec<SessionItem>> {
    let mut items = vec![SessionItem::new_session()];
    items.extend(
        stored_sessions(root)?
            .into_iter()
            .map(|(id, meta)| SessionItem::stored(id, meta)),
    );
    Ok(items)
}

fn stored_sessions(root: &Path) -> Result<Vec<(String, SessionMeta)>> {
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
    Ok(sessions)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionMeta {
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

fn status_label(status: &str) -> &'static str {
    match status {
        "running" => "идет",
        "waiting_approval" => "ждет подтверждения",
        "paused" => "пауза",
        "finished" => "завершена",
        "failed" => "ошибка",
        _ => "готова",
    }
}
