use std::{convert::Infallible, time::Duration};

use axum::{
    extract::{Path, State},
    response::sse::{Event as SseEvent, KeepAlive, Sse},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_stream::{self as stream, Stream};

use crate::GatewayState;

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

pub async fn session_events(
    Path(_id): Path<String>,
    State(state): State<GatewayState>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let events = state.mission.timeline.into_iter().map(|event| {
        let payload = serde_json::to_string(&event).unwrap_or_else(|_| "{}".into());
        Ok(SseEvent::default().event("gorsee.event").data(payload))
    });
    Sse::new(stream::iter(events)).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
