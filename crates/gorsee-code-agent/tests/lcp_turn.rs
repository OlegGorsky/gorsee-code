mod support;

use std::fs;

use gorsee_code_agent::TaskRunner;
use gorsee_code_coding_core::{CodingIntent, TranscriptEventKind};
use gorsee_code_core::AgentRole;
use serde_json::json;
use support::{
    agent_by_role, final_answer, init_git, only_session_id, turn_request, write_valid_crate,
    MockClient, MockReply,
};

#[test]
fn lcp_turn_waiting_approval_returns_response_instead_of_error() {
    let temp = tempfile::tempdir().unwrap();
    write_valid_crate(temp.path());
    let client = MockClient::new(vec![json!({
        "message": "готовлю изменение",
        "tool_calls": [
            {
                "name": "read_file",
                "args": { "path": "src/lib.rs" }
            },
            {
                "name": "apply_patch",
                "args": {
                    "path": "src/lib.rs",
                    "content": "pub fn shipped() -> bool { true }\n"
                }
            }
        ]
    })
    .to_string()]);
    let output = TaskRunner::new(temp.path().join(".gorsee-code"))
        .run_lcp_turn(
            turn_request(temp.path(), "измени src/lib.rs"),
            &client,
            vec![agent_by_role(AgentRole::Coder)],
        )
        .unwrap();

    assert_eq!(output.response.intent, CodingIntent::Edit);
    assert_eq!(output.response.status, "waiting_approval");
    assert_eq!(output.summary.session_id, output.response.session_id);
    assert_eq!(only_session_id(temp.path()), output.response.session_id);
    assert_eq!(output.response.approvals.len(), 1);
    assert_eq!(output.response.approvals[0].tool_name, "apply_patch");
    assert_eq!(output.response.approvals[0].status, "pending");
    assert!(output
        .response
        .transcript
        .iter()
        .any(|event| event.kind == TranscriptEventKind::ApprovalNeeded
            && event.summary == "требуется подтверждение: apply_patch"));
    assert!(!output
        .response
        .transcript
        .iter()
        .any(|event| event.kind == TranscriptEventKind::AssistantMessage));
}

#[test]
fn lcp_turn_reports_cached_tokens_separately_from_used_tokens() {
    let temp = tempfile::tempdir().unwrap();
    write_valid_crate(temp.path());
    let client = MockClient::with_replies(vec![MockReply::with_usage(
        final_answer("Привет! Чем помочь?"),
        json!({
            "prompt_tokens": 100,
            "completion_tokens": 5,
            "total_tokens": 105,
            "prompt_tokens_details": { "cached_tokens": 80 }
        }),
    )]);

    let output = TaskRunner::new(temp.path().join(".gorsee-code"))
        .run_lcp_turn(
            turn_request(temp.path(), "привет"),
            &client,
            vec![agent_by_role(AgentRole::Architect)],
        )
        .unwrap();

    assert_eq!(output.response.usage.tokens_used, 25);
    assert_eq!(output.response.usage.cached_tokens, 80);
    assert_eq!(output.response.usage.tokens_limit, 80_000);
    assert!(output.response.usage.percent_used < 1.0);
}

#[test]
fn chat_turn_does_not_surface_existing_workspace_diff() {
    let temp = tempfile::tempdir().unwrap();
    write_valid_crate(temp.path());
    init_git(temp.path());
    fs::write(
        temp.path().join("src/lib.rs"),
        "pub fn shipped() -> bool { true }\n",
    )
    .unwrap();
    let client = MockClient::new(vec![final_answer("Привет! Чем помочь?")]);

    let output = TaskRunner::new(temp.path().join(".gorsee-code"))
        .run_lcp_turn(
            turn_request(temp.path(), "привет"),
            &client,
            vec![agent_by_role(AgentRole::Architect)],
        )
        .unwrap();

    assert_eq!(output.response.intent, CodingIntent::Chat);
    assert!(output.response.diff.is_none());
    assert!(!output
        .summary
        .artifacts
        .iter()
        .any(|artifact| artifact.path.ends_with("diff.patch")));
    assert!(!output
        .response
        .transcript
        .iter()
        .any(|event| event.kind == TranscriptEventKind::DiffReady));
}
