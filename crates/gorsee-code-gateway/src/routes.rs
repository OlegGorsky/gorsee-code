use std::{convert::Infallible, time::Duration};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event as SseEvent, KeepAlive, Sse},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_stream::{self as stream, Stream};

use crate::{approval_actions, GatewayState};
use gorsee_code_session::ApprovalDecision;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub started_at: String,
}

pub async fn health(State(state): State<GatewayState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        started_at: state.started_at.to_rfc3339(),
    })
}

pub async fn sessions(State(state): State<GatewayState>) -> Json<Value> {
    Json(json!({ "data": state.sessions() }))
}

pub async fn create_session(
    State(state): State<GatewayState>,
    Json(request): Json<CreateSessionRequest>,
) -> ApiResult {
    let id = request.id.unwrap_or_else(default_session_id);
    let repo = request.repo.unwrap_or_default();
    let branch = request.branch.unwrap_or_else(|| "unknown".into());
    state
        .create_session(id, repo, branch)
        .map(|session| ok(json!(session)))
        .unwrap_or_else(error)
}

pub async fn session(Path(id): Path<String>, State(state): State<GatewayState>) -> ApiResult {
    match state.session(&id) {
        Ok(Some(session)) => ok(json!(session)),
        Ok(None) => not_found(),
        Err(error) => self::error(error),
    }
}

pub async fn capabilities(State(state): State<GatewayState>) -> Json<Value> {
    Json(json!({ "data": state.capabilities }))
}

pub async fn tools(State(state): State<GatewayState>) -> Json<Value> {
    Json(json!({ "data": state.tools }))
}

pub async fn skills(State(state): State<GatewayState>) -> Json<Value> {
    Json(json!({ "data": state.skills }))
}

pub async fn hooks(State(state): State<GatewayState>) -> Json<Value> {
    Json(json!({ "data": state.hooks }))
}

pub async fn usage(State(state): State<GatewayState>) -> Json<Value> {
    Json(json!({ "data": state.usage() }))
}

pub async fn limits(State(state): State<GatewayState>) -> Json<Value> {
    Json(json!({ "data": state.limits }))
}

pub async fn artifacts(State(state): State<GatewayState>) -> Json<Value> {
    Json(json!({ "data": state.artifacts }))
}

pub async fn post_message(
    Path(id): Path<String>,
    State(state): State<GatewayState>,
    Json(request): Json<MessageRequest>,
) -> ApiResult {
    match state.record_message(&id, request.message) {
        Ok(Some(event)) => ok(json!(event)),
        Ok(None) => not_found(),
        Err(error) => self::error(error),
    }
}

pub async fn approve(
    Path(id): Path<String>,
    State(state): State<GatewayState>,
    Json(request): Json<ApprovalRequest>,
) -> ApiResult {
    approval_action(state, id, request.approval_id, ApprovalDecision::Approved).await
}

pub async fn deny(
    Path(id): Path<String>,
    State(state): State<GatewayState>,
    Json(request): Json<ApprovalRequest>,
) -> ApiResult {
    approval_action(state, id, request.approval_id, ApprovalDecision::Denied).await
}

pub async fn pause(Path(id): Path<String>, State(state): State<GatewayState>) -> ApiResult {
    session_action_with_mode(state.pause_session(&id), "status_only")
}

pub async fn resume(Path(id): Path<String>, State(state): State<GatewayState>) -> ApiResult {
    session_action_with_mode(state.resume_session(&id), "status_only")
}

pub async fn session_usage(Path(id): Path<String>, State(state): State<GatewayState>) -> ApiResult {
    session_action(state.session_usage(&id))
}

pub async fn session_limits(
    Path(id): Path<String>,
    State(state): State<GatewayState>,
) -> ApiResult {
    session_action(state.session_limits(&id))
}

pub async fn session_diff(Path(id): Path<String>, State(state): State<GatewayState>) -> ApiResult {
    session_action(state.session_diff(&id))
}

pub async fn session_events(
    Path(id): Path<String>,
    State(state): State<GatewayState>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let events = state
        .session_events(&id)
        .unwrap_or_default()
        .into_iter()
        .map(|event| {
            let payload = serde_json::to_string(&event).unwrap_or_else(|_| "{}".into());
            Ok(SseEvent::default().event("gorsee.snapshot").data(payload))
        });
    Sse::new(stream::iter(events)).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

type ApiResult = (StatusCode, Json<Value>);

#[derive(Debug, Clone, Deserialize)]
pub struct CreateSessionRequest {
    pub id: Option<String>,
    pub repo: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageRequest {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApprovalRequest {
    pub approval_id: String,
}

fn session_action<T: Serialize>(result: Result<Option<T>, impl std::fmt::Display>) -> ApiResult {
    match result {
        Ok(Some(value)) => ok(json!(value)),
        Ok(None) => not_found(),
        Err(error) => self::error(error),
    }
}

fn session_action_with_mode<T: Serialize>(
    result: Result<Option<T>, impl std::fmt::Display>,
    mode: &str,
) -> ApiResult {
    match result {
        Ok(Some(value)) => ok(with_mode(json!(value), mode)),
        Ok(None) => not_found(),
        Err(error) => self::error(error),
    }
}

async fn approval_action(
    state: GatewayState,
    session_id: String,
    approval_id: String,
    decision: ApprovalDecision,
) -> ApiResult {
    match approval_actions::decide(state, session_id, approval_id, decision).await {
        Ok(action) => ok(action.into_value()),
        Err(error) => api_error(error.status, error.code, error.message),
    }
}

fn with_mode(mut value: Value, mode: &str) -> Value {
    if let Value::Object(map) = &mut value {
        map.insert("mode".into(), json!(mode));
        value
    } else {
        json!({ "value": value, "mode": mode })
    }
}

fn ok(data: Value) -> ApiResult {
    (StatusCode::OK, Json(json!({ "data": data })))
}

fn not_found() -> ApiResult {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": { "message": "session not found" } })),
    )
}

fn error(error: impl std::fmt::Display) -> ApiResult {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": { "message": error.to_string() } })),
    )
}

fn api_error(status: StatusCode, code: &str, message: String) -> ApiResult {
    (
        status,
        Json(json!({ "error": { "code": code, "message": message } })),
    )
}

fn default_session_id() -> String {
    Utc::now().format("%Y-%m-%dT%H-%M-%SZ").to_string()
}
