use gorsee_code_safety::{Redactor, RiskClass};
use gorsee_code_session::{
    ApprovalDecision, ApprovalRecord, ApprovalStatus, SessionManifest, SessionStore,
};
use serde_json::json;

#[test]
fn approval_store_appends_reads_and_decides_pending_records() {
    let temp = tempfile::tempdir().unwrap();
    let store = SessionStore::new(temp.path(), Redactor::default());
    let manifest = SessionManifest::new("s1", "/repo", "main");
    store.create(&manifest).unwrap();
    let approval = ApprovalRecord::pending(
        "s1",
        7,
        "coder",
        "propose_patch",
        json!({ "path": "src/lib.rs" }),
        RiskClass::Write,
    );

    store.append_approval(&approval).unwrap();
    let pending = store.pending_approvals("s1").unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].status, ApprovalStatus::Pending);

    store
        .decide_approval("s1", &approval.id, ApprovalDecision::Approved)
        .unwrap();
    let approvals = store.read_approvals("s1").unwrap();

    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0].status, ApprovalStatus::Approved);
    assert!(approvals[0].decided_at.is_some());
}

#[test]
fn create_writes_complete_flight_recorder_layout() {
    let temp = tempfile::tempdir().unwrap();
    let store = SessionStore::new(temp.path(), Redactor::default());
    let manifest = SessionManifest::new("s1", "/repo", "main");

    let dir = store.create(&manifest).unwrap();

    for file in [
        "manifest.json",
        "session.md",
        "events.jsonl",
        "messages.jsonl",
        "tool-calls.jsonl",
        "approvals.jsonl",
        "token-ledger.json",
        "context-map.json",
    ] {
        assert!(dir.join(file).is_file(), "{file} should exist");
    }

    for directory in ["patches", "artifacts"] {
        assert!(dir.join(directory).is_dir(), "{directory} should exist");
    }

    let ledger: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("token-ledger.json")).unwrap())
            .unwrap();
    assert_eq!(ledger, json!({ "records": [] }));

    let context: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("context-map.json")).unwrap())
            .unwrap();
    assert_eq!(
        context,
        json!({
            "repo": "/repo",
            "branch": "main",
            "files": [],
            "symbols": [],
            "decisions": []
        })
    );
}
