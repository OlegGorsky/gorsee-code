use std::fs;

use gorsee_code_ui_state::workspace_state;
use serde_json::json;

#[test]
fn workspace_state_surfaces_pending_approvals_from_latest_session() {
    let temp = tempfile::tempdir().unwrap();
    let session_id = "2026-06-19T00-00-00_ui-approval";
    let session = temp
        .path()
        .join(".gorsee-code")
        .join("sessions")
        .join(session_id);
    fs::create_dir_all(&session).unwrap();
    fs::write(session.join("events.jsonl"), "").unwrap();
    fs::write(
        session.join("manifest.json"),
        serde_json::json!({
            "id": session_id,
            "repo": temp.path().display().to_string(),
            "branch": "main",
            "started_at": "2026-06-19T00:00:00Z",
            "status": "waiting_approval",
            "agents": ["architect", "scout", "coder", "validator"],
            "budget": { "tokens_limit": 80000, "tokens_used": 1000 }
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        session.join("approvals.jsonl"),
        serde_json::json!({
            "id": "appr_ui",
            "session_id": session_id,
            "sequence": 3,
            "agent_id": "coder",
            "tool_name": "apply_patch",
            "args": {},
            "risk": "write",
            "status": "pending",
            "created_at": "2026-06-19T00:00:00Z",
            "decided_at": null
        })
        .to_string()
            + "\n",
    )
    .unwrap();

    let state = workspace_state(temp.path());

    assert_eq!(state.session.status, "waiting_approval");
    assert_eq!(state.approvals.len(), 1);
    assert_eq!(state.approvals[0].id, "appr_ui");
    assert_eq!(state.approvals[0].status, "pending");
}

#[test]
fn workspace_state_prefers_waiting_session_over_newer_finished_session() {
    let temp = tempfile::tempdir().unwrap();
    write_session(
        temp.path(),
        "2026-06-19T00-00-00_waiting",
        "2026-06-19T00:00:00Z",
        "waiting_approval",
        true,
    );
    write_session(
        temp.path(),
        "2026-06-19T01-00-00_finished",
        "2026-06-19T01:00:00Z",
        "finished",
        false,
    );

    let state = workspace_state(temp.path());

    assert_eq!(state.session.id, "2026-06-19T00-00-00_waiting");
    assert_eq!(state.session.status, "waiting_approval");
    assert_eq!(state.approvals.len(), 1);
    assert_eq!(state.approvals[0].id, "appr_ui");
}

fn write_session(
    root: &std::path::Path,
    session_id: &str,
    started_at: &str,
    status: &str,
    pending_approval: bool,
) {
    let session = root.join(".gorsee-code").join("sessions").join(session_id);
    fs::create_dir_all(&session).unwrap();
    fs::write(session.join("events.jsonl"), "").unwrap();
    fs::write(
        session.join("manifest.json"),
        json!({
            "id": session_id,
            "repo": root.display().to_string(),
            "branch": "main",
            "started_at": started_at,
            "status": status,
            "agents": ["architect", "scout", "coder", "validator"],
            "budget": { "tokens_limit": 80000, "tokens_used": 1000 }
        })
        .to_string(),
    )
    .unwrap();
    let approval = if pending_approval {
        json!({
            "id": "appr_ui",
            "session_id": session_id,
            "sequence": 3,
            "agent_id": "coder",
            "tool_name": "apply_patch",
            "args": {},
            "risk": "write",
            "status": "pending",
            "created_at": "2026-06-19T00:00:00Z",
            "decided_at": null
        })
        .to_string()
            + "\n"
    } else {
        String::new()
    };
    fs::write(session.join("approvals.jsonl"), approval).unwrap();
}
