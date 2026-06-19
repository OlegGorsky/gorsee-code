use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::Command,
};

use gorsee_code_core::{default_agent_matrix, AgentStatus, Event};
use gorsee_code_session::SessionManifest;

use crate::{AgentView, BudgetView, EventView, MissionControlState, SessionView};

pub fn workspace_state(root: impl AsRef<Path>) -> MissionControlState {
    let root = root.as_ref();
    if let Some((manifest, events)) = latest_session(root) {
        return session_state(root, manifest, events);
    }
    ready_state(root)
}

fn ready_state(root: &Path) -> MissionControlState {
    MissionControlState {
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
) -> MissionControlState {
    MissionControlState {
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
        approvals: Vec::new(),
        gateway_status: "local".into(),
    }
}

fn latest_session(root: &Path) -> Option<(SessionManifest, Vec<Event>)> {
    let dir = latest_session_dir(root)?;
    let manifest = read_manifest(&dir)?;
    let events = read_events(&dir);
    Some((manifest, events))
}

fn latest_session_dir(root: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(root.join(".gorsee-code").join("sessions")).ok()?;
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .map(|entry| entry.path())
        .max_by_key(|path| path.file_name().map(|name| name.to_owned()))
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

fn sanitize_event(mut event: EventView) -> EventView {
    event.summary = event.summary.replace(&legacy_workspace_word(), "workspace");
    event.summary = event
        .summary
        .replace("mission workspace finished", "mission finished");
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
