use std::{
    cell::RefCell,
    fs,
    net::SocketAddr,
    sync::{Arc, Mutex, OnceLock},
};

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Method, Request, StatusCode},
    routing::post,
    Json, Router,
};
use gorsee_code_agent::{AgentRunError, ChatClient, TaskRunner};
use gorsee_code_core::{default_agent_matrix, AgentProfile, AgentRole, EventKind, TaskSpec};
use gorsee_code_gateway::{app, GatewayState};
use gorsee_code_neurogate::{ChatRequest, ChatResponse};
use gorsee_code_safety::{Redactor, RiskClass};
use gorsee_code_session::{ApprovalRecord, ApprovalStatus, SessionManifest, SessionStore};
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn gateway_requires_live_client_before_deciding_saved_execution() {
    let _env = without_neurogate_env();
    let temp = tempfile::tempdir().unwrap();
    let store = SessionStore::new(temp.path().join(".gorsee-code"), Redactor::default());
    let manifest = SessionManifest::new("s1", temp.path().display().to_string(), "main");
    let session_dir = store.create(&manifest).unwrap();
    let approval = pending_approval("s1", 7);
    store.append_approval(&approval).unwrap();
    fs::write(session_dir.join("execution.json"), "{}").unwrap();

    let router = app(GatewayState::workspace(temp.path()));
    let (status, body) = post_json(
        router,
        "/v1/sessions/s1/approve",
        json!({ "approval_id": approval.id }),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "missing_auth");
    assert_eq!(
        store.read_approvals("s1").unwrap()[0].status,
        ApprovalStatus::Pending
    );
    assert!(store
        .read_events("s1")
        .unwrap()
        .iter()
        .all(|event| event.kind != EventKind::ToolApproved));
}

#[tokio::test]
async fn gateway_approval_resumes_saved_execution_with_live_client() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"sample\"\n",
    )
    .unwrap();
    let initial_client = MockClient::new(vec![json!({
        "message": "prepare approved change",
        "tool_calls": [{
            "name": "apply_patch",
            "args": {
                "path": "src/lib.rs",
                "content": "pub fn shipped() -> bool { true }\n"
            }
        }]
    })
    .to_string()]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new("ship approved change", temp.path().display().to_string());
    let AgentRunError::WaitingApproval(approval_id) = runner
        .run_sequential_with_agents(
            &spec,
            &initial_client,
            vec![agent_by_role(AgentRole::Coder)],
        )
        .unwrap_err()
    else {
        panic!("expected waiting approval");
    };
    let session_id = only_session_id(temp.path());
    let endpoint = start_chat_server(vec![final_answer("coder continued")]).await;
    write_gateway_live_config(temp.path(), &endpoint);

    let router = app(GatewayState::workspace(temp.path()));
    let (status, body) = post_json(
        router,
        &format!("/v1/sessions/{session_id}/approve"),
        json!({ "approval_id": approval_id }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["mode"], "executed");
    assert_eq!(body["data"]["status"], "approved");
    assert_eq!(body["data"]["summary"]["session_id"], session_id);
    assert_eq!(
        fs::read_to_string(temp.path().join("src/lib.rs")).unwrap(),
        "pub fn shipped() -> bool { true }\n"
    );
    assert!(!temp
        .path()
        .join(".gorsee-code")
        .join("sessions")
        .join(&session_id)
        .join("execution.json")
        .exists());
}

async fn post_json(
    router: axum::Router,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let request = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn pending_approval(session_id: &str, sequence: u64) -> ApprovalRecord {
    ApprovalRecord::pending(
        session_id,
        sequence,
        "coder",
        "apply_patch",
        json!({ "path": "src/lib.rs" }),
        RiskClass::Write,
    )
}

async fn start_chat_server(replies: Vec<String>) -> String {
    let replies = Arc::new(Mutex::new(replies.into_iter().rev().collect::<Vec<_>>()));
    let app = Router::new()
        .route("/chat/completions", post(chat_completion))
        .with_state(replies);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

async fn chat_completion(State(replies): State<Arc<Mutex<Vec<String>>>>) -> Json<Value> {
    let content = replies.lock().unwrap().pop().unwrap();
    Json(json!({
        "id": "mock",
        "object": "chat.completion",
        "choices": [{ "message": { "content": content } }]
    }))
}

fn write_gateway_live_config(root: &std::path::Path, endpoint: &str) {
    fs::create_dir_all(root.join(".gorsee-code")).unwrap();
    fs::write(
        root.join(".gorsee-code").join("auth.json"),
        json!({ "api_key": "test-key" }).to_string(),
    )
    .unwrap();
    fs::write(
        root.join("gorsee-code.toml"),
        format!(
            r#"[project]
name = "sample"
guidance_files = []
protected_paths = []

[neurogate]
endpoint = "{endpoint}"
auth_source = "local_file"

[budget]
session_tokens = 80000
session_usd = 2.0
warn_at_percent = 75
stop_at_percent = 100

[agents]
"#
        ),
    )
    .unwrap();
}

fn only_session_id(root: &std::path::Path) -> String {
    let mut sessions = fs::read_dir(root.join(".gorsee-code").join("sessions"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    sessions.sort();
    assert_eq!(sessions.len(), 1);
    sessions.pop().unwrap()
}

fn final_answer(answer: &str) -> String {
    json!({ "message": answer, "final_answer": answer }).to_string()
}

fn agent_by_role(role: AgentRole) -> AgentProfile {
    default_agent_matrix()
        .into_iter()
        .find(|agent| agent.role == role)
        .unwrap()
}

struct MockClient {
    replies: RefCell<Vec<String>>,
}

impl MockClient {
    fn new(replies: Vec<String>) -> Self {
        Self {
            replies: RefCell::new(replies.into_iter().rev().collect()),
        }
    }
}

impl ChatClient for MockClient {
    fn complete(&self, _request: &ChatRequest) -> Result<ChatResponse, AgentRunError> {
        Ok(ChatResponse {
            id: Some("mock".into()),
            object: Some("chat.completion".into()),
            choices: Some(vec![json!({
                "message": { "content": self.replies.borrow_mut().pop().unwrap() }
            })]),
            usage: None,
        })
    }
}

fn without_neurogate_env() -> EnvGuard {
    let _guard = env_lock().lock().unwrap();
    let old_primary = std::env::var_os("NEUROGATE_API_KEY");
    let old_fallback = std::env::var_os("GORSEE_NEUROGATE_API_KEY");
    let old_auth_home = std::env::var_os("GORSEE_CODE_AUTH_HOME");
    let auth_home = tempfile::tempdir().unwrap();
    std::env::remove_var("NEUROGATE_API_KEY");
    std::env::remove_var("GORSEE_NEUROGATE_API_KEY");
    std::env::set_var("GORSEE_CODE_AUTH_HOME", auth_home.path());
    EnvGuard {
        _guard,
        old_primary,
        old_fallback,
        old_auth_home,
        _auth_home: auth_home,
    }
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvGuard {
    _guard: std::sync::MutexGuard<'static, ()>,
    old_primary: Option<std::ffi::OsString>,
    old_fallback: Option<std::ffi::OsString>,
    old_auth_home: Option<std::ffi::OsString>,
    _auth_home: tempfile::TempDir,
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        restore_env("NEUROGATE_API_KEY", self.old_primary.take());
        restore_env("GORSEE_NEUROGATE_API_KEY", self.old_fallback.take());
        restore_env("GORSEE_CODE_AUTH_HOME", self.old_auth_home.take());
    }
}

fn restore_env(name: &str, value: Option<std::ffi::OsString>) {
    match value {
        Some(value) => std::env::set_var(name, value),
        None => std::env::remove_var(name),
    }
}
