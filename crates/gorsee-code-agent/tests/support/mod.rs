#![allow(dead_code)]

use std::{cell::RefCell, fs, path::Path, process::Command};

use gorsee_code_agent::ChatClient;
use gorsee_code_coding_core::{TurnRequest, WorkspaceRef};
use gorsee_code_core::{default_agent_matrix, AgentProfile, AgentRole};
use gorsee_code_neurogate::{ChatRequest, ChatResponse};
use serde_json::{json, Value};

pub(crate) fn only_session_id(root: &Path) -> String {
    let mut sessions = fs::read_dir(root.join(".gorsee-code").join("sessions"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    sessions.sort();
    assert_eq!(sessions.len(), 1);
    sessions.pop().unwrap()
}

pub(crate) fn write_valid_crate(root: &Path) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub fn shipped() -> bool { false }\n",
    )
    .unwrap();
}

pub(crate) fn init_git(root: &Path) {
    run_git(root, &["init"]);
    run_git(root, &["add", "."]);
    run_git(
        root,
        &[
            "-c",
            "user.email=test@example.com",
            "-c",
            "user.name=Test",
            "commit",
            "-m",
            "initial",
        ],
    );
}

pub(crate) fn turn_request(root: &Path, objective: &str) -> TurnRequest {
    TurnRequest {
        workspace: WorkspaceRef {
            root: root.display().to_string(),
            branch: None,
            session_id: None,
        },
        message: objective.into(),
        user_id: Some("test".into()),
    }
}

fn run_git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(root)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

pub(crate) fn agent_by_role(role: AgentRole) -> AgentProfile {
    default_agent_matrix()
        .into_iter()
        .find(|agent| agent.role == role)
        .unwrap()
}

pub(crate) fn final_answer(answer: &str) -> String {
    json!({
        "message": answer,
        "final_answer": answer
    })
    .to_string()
}

#[derive(Debug, Clone)]
pub(crate) struct MockReply {
    content: String,
    usage: Option<Value>,
}

impl MockReply {
    pub(crate) fn content(content: String) -> Self {
        Self {
            content,
            usage: None,
        }
    }

    pub(crate) fn with_usage(content: String, usage: Value) -> Self {
        Self {
            content,
            usage: Some(usage),
        }
    }
}

pub(crate) struct MockClient {
    replies: RefCell<Vec<MockReply>>,
}

impl MockClient {
    pub(crate) fn new(replies: Vec<String>) -> Self {
        Self::with_replies(replies.into_iter().map(MockReply::content).collect())
    }

    pub(crate) fn with_replies(replies: Vec<MockReply>) -> Self {
        Self {
            replies: RefCell::new(replies.into_iter().rev().collect()),
        }
    }
}

impl ChatClient for MockClient {
    fn complete(
        &self,
        request: &ChatRequest,
    ) -> Result<ChatResponse, gorsee_code_agent::AgentRunError> {
        let reply = self.replies.borrow_mut().pop().unwrap_or_else(|| {
            panic!(
                "mock replies exhausted for model={} prompt={}",
                request.model,
                request
                    .messages
                    .last()
                    .map(|message| message.content.as_str())
                    .unwrap_or_default()
            )
        });
        Ok(ChatResponse {
            id: Some("mock".into()),
            object: Some("chat.completion".into()),
            choices: Some(vec![json!({ "message": { "content": reply.content } })]),
            usage: reply.usage,
        })
    }
}

pub(crate) fn artifact_json(
    artifacts: &[gorsee_code_artifacts::ArtifactRecord],
    name: &str,
) -> Value {
    let artifact = artifacts
        .iter()
        .find(|artifact| {
            Path::new(&artifact.path)
                .file_name()
                .is_some_and(|file| file == name)
        })
        .unwrap_or_else(|| panic!("missing {name}"));
    serde_json::from_str(&fs::read_to_string(&artifact.path).unwrap()).unwrap()
}
