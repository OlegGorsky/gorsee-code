use std::{fs, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use gorsee_code_core::{Event, EventKind};
use gorsee_code_gateway::{app, GatewayState};
use gorsee_code_safety::{Redactor, RiskClass};
use gorsee_code_session::{ApprovalRecord, ApprovalStatus, SessionManifest, SessionStore};
use serde_json::json;
use tower::ServiceExt;

#[test]
fn gateway_reads_events_for_requested_session_from_workspace_store() {
    let temp = tempfile::tempdir().unwrap();
    let session = temp.path().join(".gorsee-code").join("sessions").join("s1");
    fs::create_dir_all(&session).unwrap();
    let event = Event::new(
        1,
        "s1",
        Some("coder".into()),
        EventKind::ToolFinished,
        json!({ "name": "run_test", "output": "ok" }),
    );
    fs::write(session.join("events.jsonl"), format!("{}\n", json!(event))).unwrap();

    let state = GatewayState::workspace(temp.path());
    let events = state.session_events("s1").unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].session_id, "s1");
    assert_eq!(events[0].kind, EventKind::ToolFinished);
}

#[tokio::test]
async fn gateway_exposes_session_command_endpoints() {
    let temp = tempfile::tempdir().unwrap();
    let store = SessionStore::new(temp.path().join(".gorsee-code"), Redactor::default());
    let manifest = SessionManifest::new("s1", temp.path().display().to_string(), "main");
    let dir = store.create(&manifest).unwrap();
    fs::write(
        dir.join("patches").join("0001.diff"),
        "diff --git a/lib b/lib\n",
    )
    .unwrap();

    let router = app(GatewayState::workspace(temp.path()));

    let session = get_json(router.clone(), "/v1/sessions/s1").await;
    assert_eq!(session["data"]["id"], "s1");

    let message = post_json(
        router.clone(),
        "/v1/sessions/s1/message",
        json!({ "message": "continue" }),
    )
    .await;
    assert_eq!(message["data"]["kind"], "agent_message");

    let paused = post_json(router.clone(), "/v1/sessions/s1/pause", json!({})).await;
    assert_eq!(paused["data"]["status"], "paused");
    assert_eq!(paused["data"]["mode"], "status_only");

    let resumed = post_json(router.clone(), "/v1/sessions/s1/resume", json!({})).await;
    assert_eq!(resumed["data"]["status"], "running");
    assert_eq!(resumed["data"]["mode"], "status_only");

    let stream = get_text(router.clone(), "/v1/sessions/s1/events").await;
    assert!(stream.contains("event: gorsee.snapshot"));

    let usage = get_json(router.clone(), "/v1/sessions/s1/usage").await;
    assert_eq!(usage["data"]["limit_tokens"], 80_000);

    let limits = get_json(router.clone(), "/v1/sessions/s1/limits").await;
    assert!(limits["data"].is_array());

    let diff = get_json(router, "/v1/sessions/s1/diff").await;
    assert_eq!(diff["data"][0]["path"], "0001.diff");

    let events = store.read_events("s1").unwrap();
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::AgentMessage));
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::SessionPaused));
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::SessionResumed));
}

#[tokio::test]
async fn gateway_records_approval_decisions() {
    let temp = tempfile::tempdir().unwrap();
    let store = SessionStore::new(temp.path().join(".gorsee-code"), Redactor::default());
    let manifest = SessionManifest::new("s1", temp.path().display().to_string(), "main");
    store.create(&manifest).unwrap();
    let approve = ApprovalRecord::pending(
        "s1",
        7,
        "coder",
        "apply_patch",
        json!({ "path": "src/lib.rs" }),
        RiskClass::Write,
    );
    let deny = ApprovalRecord::pending(
        "s1",
        8,
        "coder",
        "run_command",
        json!({ "command": "cargo test" }),
        RiskClass::Command,
    );
    store.append_approval(&approve).unwrap();
    store.append_approval(&deny).unwrap();

    let router = app(GatewayState::workspace(temp.path()));
    let approved = post_json(
        router.clone(),
        "/v1/sessions/s1/approve",
        json!({ "approval_id": approve.id }),
    )
    .await;
    let denied = post_json(
        router,
        "/v1/sessions/s1/deny",
        json!({ "approval_id": deny.id }),
    )
    .await;

    assert_eq!(approved["data"]["status"], "approved");
    assert_eq!(approved["data"]["mode"], "record_only");
    assert_eq!(denied["data"]["status"], "denied");
    assert_eq!(denied["data"]["mode"], "record_only");

    let approvals = store.read_approvals("s1").unwrap();
    assert_eq!(approvals[0].status, ApprovalStatus::Approved);
    assert_eq!(approvals[1].status, ApprovalStatus::Denied);

    let events = store.read_events("s1").unwrap();
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::ToolApproved));
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::ToolDenied));
}

async fn get_json(router: axum::Router, uri: &str) -> serde_json::Value {
    request_json(router, Method::GET, uri, None).await
}

async fn get_text(router: axum::Router, uri: &str) -> String {
    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = tokio::time::timeout(
        Duration::from_secs(1),
        to_bytes(response.into_body(), usize::MAX),
    )
    .await
    .unwrap()
    .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

async fn post_json(router: axum::Router, uri: &str, body: serde_json::Value) -> serde_json::Value {
    request_json(router, Method::POST, uri, Some(body)).await
}

async fn request_json(
    router: axum::Router,
    method: Method,
    uri: &str,
    body: Option<serde_json::Value>,
) -> serde_json::Value {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(match body {
            Some(value) => Body::from(value.to_string()),
            None => Body::empty(),
        })
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
