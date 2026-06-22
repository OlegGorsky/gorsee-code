mod support;

use std::fs;

use gorsee_code_agent::{AgentRunError, TaskRunner};
use gorsee_code_coding_core::{
    CodingIntent, LcpTurnResponseInput, LcpUsageSnapshot, LocalCodingProtocol, TranscriptEventKind,
};
use gorsee_code_core::{AgentRole, EventKind, TaskSpec};
use gorsee_code_safety::Redactor;
use gorsee_code_session::{ApprovalDecision, SessionStore};
use serde_json::json;
use support::{
    agent_by_role, artifact_json, final_answer, init_git, only_session_id, turn_request,
    write_valid_crate, MockClient,
};

#[test]
fn edit_turn_approvals_finish_with_diff_verification_and_clean_lcp_response() {
    let temp = tempfile::tempdir().unwrap();
    write_valid_crate(temp.path());
    init_git(temp.path());
    let client = MockClient::new(vec![
        json!({
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
                        "content": "pub fn shipped() -> bool { true }\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn shipped_is_true() {\n        assert!(super::shipped());\n    }\n}\n"
                    }
                }
            ]
        })
        .to_string(),
        final_answer("Изменён src/lib.rs. Diff и проверки переданы Validator."),
        json!({
            "message": "проверяю diff и тесты",
            "tool_calls": [
                { "name": "git_diff", "args": {} },
                { "name": "run_test", "args": { "command": ["cargo", "test", "--workspace"] } }
            ],
            "final_answer": "Готово."
        })
        .to_string(),
        final_answer(
            "Изменён src/lib.rs. Diff: 1 файл с добавлением реализации и теста. Проверки: cargo test --workspace прошёл.",
        ),
    ]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let spec = TaskSpec::new(
        "измени src/lib.rs и запусти тесты",
        temp.path().display().to_string(),
    );
    let agents = vec![
        agent_by_role(AgentRole::Coder),
        agent_by_role(AgentRole::Validator),
    ];

    let error = runner
        .run_sequential_with_agents(&spec, &client, agents.clone())
        .unwrap_err();
    let AgentRunError::WaitingApproval(write_approval) = error else {
        panic!("expected write approval, got {error:?}");
    };
    let session_id = only_session_id(temp.path());
    let second_error = runner
        .resume_after_decision(
            &session_id,
            &write_approval,
            ApprovalDecision::Approved,
            &client,
        )
        .unwrap_err();
    let AgentRunError::WaitingApproval(test_approval) = second_error else {
        panic!("expected test approval, got {second_error:?}");
    };

    let summary = runner
        .resume_after_decision(
            &session_id,
            &test_approval,
            ApprovalDecision::Approved,
            &client,
        )
        .unwrap();

    let store = SessionStore::new(temp.path().join(".gorsee-code"), Redactor::default());
    let manifest = store.read_manifest(&session_id).unwrap();
    let events = store.read_events(&session_id).unwrap();
    let diff = artifact_json(&summary.artifacts, "diff.json");
    let verification = artifact_json(&summary.artifacts, "verification.json");
    let response = LocalCodingProtocol::default().turn_response(
        LocalCodingProtocol::default()
            .plan_turn(turn_request(temp.path(), &spec.objective), agents)
            .orchestration,
        LcpTurnResponseInput {
            session_id: &session_id,
            turn_id: None,
            status: &manifest.status,
            events: &events,
            usage: LcpUsageSnapshot::new(manifest.budget.tokens_used, manifest.budget.tokens_limit),
            approvals: Vec::new(),
        },
    );

    assert_eq!(manifest.status, "ready");
    assert!(fs::read_to_string(temp.path().join("src/lib.rs"))
        .unwrap()
        .contains("shipped_is_true"));
    assert_eq!(diff["status"], "ok");
    assert!(diff["diff"]["summary"]["files_changed"]
        .as_u64()
        .is_some_and(|changed| changed >= 1));
    assert_eq!(verification["status"], "passed");
    assert_eq!(verification["command"], "cargo test --workspace");
    assert!(events
        .iter()
        .any(|event| event.kind == EventKind::DiffReady && event.payload["status"] == "ok"));
    assert!(events.iter().any(|event| {
        event.kind == EventKind::TestFinished && event.payload["status"] == "passed"
    }));
    assert_eq!(response.intent, CodingIntent::Edit);
    assert_eq!(response.status, "ready");
    assert_eq!(response.diff.as_ref().unwrap().status, "ok");
    assert_eq!(response.verification.as_ref().unwrap().status, "passed");
    let transcript_kinds = response
        .transcript
        .iter()
        .map(|event| &event.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        transcript_kinds.first(),
        Some(&&TranscriptEventKind::UserMessage)
    );
    assert_eq!(
        transcript_kinds.last(),
        Some(&&TranscriptEventKind::AssistantMessage)
    );
    assert!(transcript_kinds.contains(&&TranscriptEventKind::DiffReady));
    assert!(transcript_kinds.contains(&&TranscriptEventKind::VerificationResult));
    assert!(transcript_kinds.contains(&&TranscriptEventKind::ApprovalNeeded));
    assert_eq!(
        response.transcript.last().unwrap().summary,
        "Изменён src/lib.rs. Diff: 1 файл с добавлением реализации и теста. Проверки: cargo test --workspace прошёл."
    );
    assert!(!response
        .transcript
        .iter()
        .any(|event| event.summary == "Готово."));
}
