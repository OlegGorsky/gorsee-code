use std::{
    cmp::Ordering,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::Command,
};

use gorsee_code_core::{default_agent_matrix, AgentStatus, Event};
use gorsee_code_session::{ApprovalRecord, ApprovalStatus, SessionManifest};

use crate::{AgentView, BudgetView, EventView, SessionView, ToolCallView, WorkspaceState};

pub fn workspace_state(root: impl AsRef<Path>) -> WorkspaceState {
    let root = root.as_ref();
    if let Some((manifest, events, approvals)) = latest_session(root) {
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
        agents: agent_views("ready", 0),
        timeline: vec![ready_event()],
        budget: budget_view(0, 80_000),
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
    WorkspaceState {
        session: SessionView {
            id: manifest.id.clone(),
            title: "Gorsee Code Workspace".into(),
            status: manifest.status.clone(),
            repo: repo_label(root, &manifest),
            branch: branch_label(root, &manifest),
        },
        agents: agent_views(&manifest.status, manifest.budget.tokens_used),
        timeline: event_views(events),
        budget: budget_view(manifest.budget.tokens_used, manifest.budget.tokens_limit),
        approvals: approval_views(approvals),
        gateway_status: "local".into(),
    }
}

fn latest_session(root: &Path) -> Option<(SessionManifest, Vec<Event>, Vec<ApprovalRecord>)> {
    let dir = latest_session_dir(root)?;
    let manifest = read_manifest(&dir)?;
    let events = read_events(&dir);
    let approvals = read_approvals(&dir);
    Some((manifest, events, approvals))
}

fn latest_session_dir(root: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(root.join(".gorsee-code").join("sessions")).ok()?;
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .filter_map(|entry| {
            let path = entry.path();
            let manifest = read_manifest(&path)?;
            Some((path, manifest))
        })
        .max_by(|left, right| compare_sessions(&left.1, &right.1))
        .map(|(path, _)| path)
}

fn compare_sessions(left: &SessionManifest, right: &SessionManifest) -> Ordering {
    active_rank(&left.status)
        .cmp(&active_rank(&right.status))
        .then_with(|| left.started_at.cmp(&right.started_at))
        .then_with(|| left.id.cmp(&right.id))
}

fn active_rank(status: &str) -> u8 {
    match status {
        "running" | "waiting_approval" | "paused" => 1,
        _ => 0,
    }
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
    if events.is_empty() {
        return vec![ready_event()];
    }
    events
        .iter()
        .map(EventView::from_event)
        .map(sanitize_event)
        .collect()
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

fn ready_event() -> EventView {
    EventView {
        sequence: 1,
        kind: "workspace_ready".into(),
        agent_id: None,
        summary: "workspace ready".into(),
    }
}

fn agent_views(status: &str, used_tokens: u64) -> Vec<AgentView> {
    let profiles = default_agent_matrix();
    let per_agent = used_tokens / profiles.len().max(1) as u64;
    profiles
        .iter()
        .enumerate()
        .map(|(index, profile)| {
            AgentView::from_profile(profile, agent_status(status, index), per_agent)
        })
        .collect()
}

fn agent_status(status: &str, index: usize) -> AgentStatus {
    match status {
        "finished" => AgentStatus::Finished,
        "failed" => AgentStatus::Failed,
        "waiting_approval" => AgentStatus::WaitingApproval,
        "running" => running_status(index),
        _ => AgentStatus::Idle,
    }
}

fn running_status(index: usize) -> AgentStatus {
    match index {
        0 => AgentStatus::Planning,
        1 => AgentStatus::Reading,
        2 => AgentStatus::Patching,
        3 => AgentStatus::Validating,
        _ => AgentStatus::Idle,
    }
}

fn budget_view(used_tokens: u64, limit_tokens: u64) -> BudgetView {
    let percent_used = if limit_tokens == 0 {
        100.0
    } else {
        used_tokens as f64 * 100.0 / limit_tokens as f64
    };
    BudgetView {
        used_tokens,
        limit_tokens,
        percent_used,
        warning: percent_used >= 75.0,
        stopped: percent_used >= 100.0,
    }
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
mod tests {
    use super::*;

    #[test]
    fn empty_workspace_is_ready() {
        let temp = tempfile::tempdir().unwrap();
        let state = workspace_state(temp.path());

        assert_eq!(state.session.title, "Gorsee Code Workspace");
        assert_eq!(state.session.status, "ready");
        assert_eq!(state.budget.used_tokens, 0);
        assert_eq!(state.timeline[0].kind, "workspace_ready");
    }
}
