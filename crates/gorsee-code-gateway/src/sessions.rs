use std::{
    fs,
    path::{Path, PathBuf},
};

use gorsee_code_core::EventKind;
use gorsee_code_session::{ApprovalDecision, SessionManifest};
use gorsee_code_ui_state::{BudgetView, SessionView};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DiffView {
    pub path: String,
    pub content: String,
}

pub(crate) fn read_session_view(root: &Path, dir: PathBuf) -> Option<SessionView> {
    let text = fs::read_to_string(dir.join("manifest.json")).ok()?;
    let manifest = serde_json::from_str::<SessionManifest>(&text).ok()?;
    Some(session_view(root, &manifest))
}

pub(crate) fn session_view(root: &Path, manifest: &SessionManifest) -> SessionView {
    SessionView {
        id: manifest.id.clone(),
        title: "Gorsee Code Workspace".into(),
        status: manifest.status.clone(),
        repo: session_repo(root, manifest),
        branch: manifest.branch.clone(),
    }
}

pub(crate) fn budget_view(manifest: &SessionManifest) -> BudgetView {
    let percent_used = usage_percent(manifest);
    BudgetView {
        used_tokens: manifest.budget.tokens_used,
        limit_tokens: manifest.budget.tokens_limit,
        percent_used,
        warning: percent_used >= 75.0,
        stopped: percent_used >= 100.0,
    }
}

pub(crate) fn diff_files(dir: impl AsRef<Path>) -> Vec<DiffView> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
        })
        .filter_map(|entry| diff_file(entry.path()))
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    files
}

pub(crate) fn decision_event(decision: ApprovalDecision) -> EventKind {
    match decision {
        ApprovalDecision::Approved => EventKind::ToolApproved,
        ApprovalDecision::Denied => EventKind::ToolDenied,
    }
}

fn session_repo(root: &Path, manifest: &SessionManifest) -> String {
    if manifest.repo.trim().is_empty() {
        root.display().to_string()
    } else {
        manifest.repo.clone()
    }
}

fn usage_percent(manifest: &SessionManifest) -> f64 {
    if manifest.budget.tokens_limit == 0 {
        100.0
    } else {
        manifest.budget.tokens_used as f64 * 100.0 / manifest.budget.tokens_limit as f64
    }
}

fn diff_file(path: PathBuf) -> Option<DiffView> {
    Some(DiffView {
        path: path.file_name()?.to_str()?.to_string(),
        content: fs::read_to_string(path).ok()?,
    })
}
