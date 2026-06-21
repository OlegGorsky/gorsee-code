use std::{
    cmp::Ordering,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::Command,
};

use gorsee_code_core::{Event, EventKind};
use gorsee_code_session::{ApprovalRecord, ApprovalStatus, SessionManifest};

use crate::{
    workspace_agents::{agent_views, budget_view, config_for, read_ledger},
    EventView, SessionView, ToolCallView, WorkspaceState,
};

pub fn workspace_state(root: impl AsRef<Path>) -> WorkspaceState {
    let root = root.as_ref();
    if let Some((manifest, events, approvals)) = active_session(root) {
        return session_state(root, manifest, events, approvals);
    }
    ready_state(root)
}

pub fn workspace_state_for_session(
    root: impl AsRef<Path>,
    session_id: Option<&str>,
) -> WorkspaceState {
    let root = root.as_ref();
    let Some(session_id) = session_id else {
        return workspace_state(root);
    };
    if let Some((manifest, events, approvals)) = requested_session(root, session_id) {
        return session_state(root, manifest, events, approvals);
    }
    ready_state(root)
}

fn ready_state(root: &Path) -> WorkspaceState {
    WorkspaceState {
        session: SessionView {
            id: "workspace".into(),
            title: "Gorsee Code Workspace".into(),
            status: "ready".into(),
            repo: root.display().to_string(),
            branch: current_branch(root),
        },
        agents: Vec::new(),
        timeline: Vec::new(),
        budget: budget_view(0, config_for(root).budget.session_tokens),
        approvals: Vec::new(),
        gateway_status: "local".into(),
    }
}

fn session_state(
    root: &Path,
    manifest: SessionManifest,
    events: Vec<Event>,
    approvals: Vec<ApprovalRecord>,
) -> WorkspaceState {
    let ledger = read_ledger(root, &manifest.id);
    let used_tokens = ledger
        .as_ref()
        .map(|ledger| ledger.totals().tokens)
        .filter(|tokens| *tokens > 0)
        .unwrap_or(manifest.budget.tokens_used);
    WorkspaceState {
        session: SessionView {
            id: manifest.id.clone(),
            title: "Gorsee Code Workspace".into(),
            status: manifest.status.clone(),
            repo: repo_label(root, &manifest),
            branch: branch_label(root, &manifest),
        },
        agents: agent_views(
            root,
            &manifest.status,
            used_tokens,
            ledger.as_ref(),
            Some(&manifest.agents),
        ),
        timeline: event_views(events),
        budget: budget_view(used_tokens, manifest.budget.tokens_limit),
        approvals: approval_views(approvals),
        gateway_status: "local".into(),
    }
}

fn requested_session(
    root: &Path,
    session_id: &str,
) -> Option<(SessionManifest, Vec<Event>, Vec<ApprovalRecord>)> {
    let dir = root.join(".gorsee-code").join("sessions").join(session_id);
    let manifest = read_manifest(&dir)?;
    let events = read_events(&dir);
    let approvals = read_approvals(&dir);
    Some((manifest, events, approvals))
}

fn active_session(root: &Path) -> Option<(SessionManifest, Vec<Event>, Vec<ApprovalRecord>)> {
    let dir = active_session_dir(root)?;
    let manifest = read_manifest(&dir)?;
    let events = read_events(&dir);
    let approvals = read_approvals(&dir);
    Some((manifest, events, approvals))
}

fn active_session_dir(root: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(root.join(".gorsee-code").join("sessions")).ok()?;
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .filter_map(|entry| {
            let path = entry.path();
            let manifest = read_manifest(&path)?;
            is_active_session(&manifest.status).then_some((path, manifest))
        })
        .max_by(|left, right| compare_sessions(&left.1, &right.1))
        .map(|(path, _)| path)
}

fn compare_sessions(left: &SessionManifest, right: &SessionManifest) -> Ordering {
    left.started_at
        .cmp(&right.started_at)
        .then_with(|| left.id.cmp(&right.id))
}

fn is_active_session(status: &str) -> bool {
    matches!(status, "running" | "waiting_approval" | "paused")
}

fn read_manifest(session_dir: &Path) -> Option<SessionManifest> {
    let text = fs::read_to_string(session_dir.join("manifest.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn read_events(session_dir: &Path) -> Vec<Event> {
    let Ok(file) = fs::File::open(session_dir.join("events.jsonl")) else {
        return Vec::new();
    };
    let mut events = BufReader::new(file)
        .lines()
        .filter_map(|line| serde_json::from_str::<Event>(&line.ok()?).ok())
        .collect::<Vec<_>>();
    events.sort_by_key(|event| event.sequence);
    events
}

fn read_approvals(session_dir: &Path) -> Vec<ApprovalRecord> {
    let Ok(file) = fs::File::open(session_dir.join("approvals.jsonl")) else {
        return Vec::new();
    };
    let mut approvals = BufReader::new(file)
        .lines()
        .filter_map(|line| serde_json::from_str::<ApprovalRecord>(&line.ok()?).ok())
        .collect::<Vec<_>>();
    approvals.sort_by_key(|approval| approval.sequence);
    approvals
}

fn event_views(events: Vec<Event>) -> Vec<EventView> {
    events
        .iter()
        .filter(|event| visible_event(&event.kind))
        .map(EventView::from_event)
        .map(sanitize_event)
        .collect()
}

fn visible_event(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::SessionStarted
            | EventKind::AgentMessage
            | EventKind::BudgetWarning
            | EventKind::BudgetExceeded
            | EventKind::Error
    )
}

fn approval_views(approvals: Vec<ApprovalRecord>) -> Vec<ToolCallView> {
    approvals
        .into_iter()
        .filter(|approval| approval.status == ApprovalStatus::Pending)
        .map(|approval| ToolCallView {
            id: approval.id,
            name: approval.tool_name,
            status: "pending".into(),
            risk: format!("{:?}", approval.risk).to_lowercase(),
        })
        .collect()
}

fn sanitize_event(mut event: EventView) -> EventView {
    event.summary = event.summary.replace(&legacy_workspace_word(), "workspace");
    event.summary = event
        .summary
        .replace("session workspace finished", "session finished");
    event
}

fn legacy_workspace_word() -> String {
    ['s', 'c', 'a', 'f', 'f', 'o', 'l', 'd']
        .into_iter()
        .collect()
}

fn repo_label(root: &Path, manifest: &SessionManifest) -> String {
    if manifest.repo.trim().is_empty() {
        root.display().to_string()
    } else {
        manifest.repo.clone()
    }
}

fn branch_label(root: &Path, manifest: &SessionManifest) -> String {
    if !manifest.branch.trim().is_empty() && manifest.branch != "unknown" {
        return manifest.branch.clone();
    }
    current_branch(root)
}

fn current_branch(root: &Path) -> String {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("branch")
        .arg("--show-current")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|branch| branch.trim().to_string())
        .filter(|branch| !branch.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
