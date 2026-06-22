use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use agent_client_protocol::schema::v1::{
    AgentCapabilities, AgentResponse, ContentBlock, Error, JsonRpcMessage, McpCapabilities,
    PromptCapabilities, Request, RequestId, Response, SessionAdditionalDirectoriesCapabilities,
    SessionCapabilities, SessionCloseCapabilities, SessionInfo, SessionListCapabilities,
    SessionResumeCapabilities,
};
use gorsee_code_safety::Redactor;
use gorsee_code_session::{SessionManifest, SessionStore};
use serde_json::{json, Value};

pub(crate) fn parse_request(line: &str) -> Result<Request<Value>, Error> {
    serde_json::from_str::<JsonRpcMessage<Request<Value>>>(line)
        .map(JsonRpcMessage::into_inner)
        .map_err(|error| Error::invalid_request().data(json!({ "error": error.to_string() })))
}

pub(crate) fn params_as<T: serde::de::DeserializeOwned>(
    params: Option<Value>,
) -> Result<T, String> {
    serde_json::from_value(params.unwrap_or_else(|| json!({}))).map_err(|error| error.to_string())
}

pub(crate) fn serialize_result(
    id: RequestId,
    result: AgentResponse,
) -> Result<String, serde_json::Error> {
    serde_json::to_string(&JsonRpcMessage::wrap(Response::new(id, Ok(result))))
}

pub(crate) fn serialize_params_error(
    id: RequestId,
    error: String,
) -> Result<String, serde_json::Error> {
    serialize_error(id, Error::invalid_params().data(json!({ "error": error })))
}

pub(crate) fn serialize_error(id: RequestId, error: Error) -> Result<String, serde_json::Error> {
    serde_json::to_string(&JsonRpcMessage::wrap(Response::<AgentResponse>::new(
        id,
        Err(error),
    )))
}

pub(crate) fn runner_error(error: &str) -> Error {
    if error.contains("missing_auth") {
        return Error::auth_required().data(json!({ "error": error }));
    }
    Error::internal_error().data(json!({ "error": error }))
}

pub(crate) fn agent_capabilities() -> AgentCapabilities {
    AgentCapabilities::new()
        .load_session(true)
        .prompt_capabilities(PromptCapabilities::new().embedded_context(true).image(true))
        .mcp_capabilities(McpCapabilities::new().http(true).sse(true))
        .session_capabilities(
            SessionCapabilities::new()
                .list(SessionListCapabilities::new())
                .additional_directories(SessionAdditionalDirectoriesCapabilities::new())
                .resume(SessionResumeCapabilities::new())
                .close(SessionCloseCapabilities::new()),
        )
}

pub(crate) fn create_ready_session(root: &Path, session_id: &str) -> Result<(), String> {
    let store = SessionStore::new(root.join(".gorsee-code"), Redactor::default());
    let mut manifest = SessionManifest::new(session_id, root.display().to_string(), "unknown");
    manifest.status = "ready".into();
    store.create(&manifest).map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn list_sessions(root: &Path) -> Vec<SessionInfo> {
    let dir = root.join(".gorsee-code").join("sessions");
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut sessions = entries
        .filter_map(|entry| read_session_info(&entry.ok()?.path()))
        .collect::<Vec<_>>();
    sessions.sort_by(|left, right| left.session_id.0.cmp(&right.session_id.0));
    sessions
}

pub(crate) fn prompt_text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn new_acp_session_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("acp-{millis}")
}

fn read_session_info(dir: &Path) -> Option<SessionInfo> {
    let manifest = serde_json::from_str::<SessionManifest>(
        &fs::read_to_string(dir.join("manifest.json")).ok()?,
    )
    .ok()?;
    Some(
        SessionInfo::new(manifest.id.clone(), manifest.repo.clone())
            .title(Some(manifest.id))
            .updated_at(Some(manifest.started_at.to_rfc3339())),
    )
}
