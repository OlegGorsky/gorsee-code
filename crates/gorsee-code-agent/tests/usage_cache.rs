mod support;

use std::fs;

use gorsee_code_agent::TaskRunner;
use gorsee_code_core::{AgentRole, TaskSpec};
use gorsee_code_session::SessionStore;
use serde_json::json;
use support::{
    agent_by_role, artifact_json, final_answer, write_valid_crate, MockClient, MockReply,
};

#[test]
fn cached_prompt_tokens_do_not_inflate_budget_or_usage_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    write_valid_crate(temp.path());
    let client = MockClient::with_replies(vec![MockReply::with_usage(
        final_answer("architect done"),
        json!({
            "prompt_tokens": 100,
            "completion_tokens": 5,
            "total_tokens": 105,
            "prompt_tokens_details": { "cached_tokens": 80 }
        }),
    )]);
    let runner = TaskRunner::new(temp.path().join(".gorsee-code"));
    let mut spec = TaskSpec::new("привет", temp.path().display().to_string());
    spec.budget_tokens = 100;

    let summary = runner
        .run_sequential_with_agents(&spec, &client, vec![agent_by_role(AgentRole::Architect)])
        .unwrap();

    let store = SessionStore::new(
        temp.path().join(".gorsee-code"),
        gorsee_code_safety::Redactor::default(),
    );
    let manifest = store.read_manifest(&summary.session_id).unwrap();
    assert_eq!(manifest.budget.tokens_used, 25);

    let usage = artifact_json(&summary.artifacts, "usage.json");
    assert_eq!(usage["tokens_used"], 25);
    assert_eq!(usage["cached_tokens"], 80);

    let ledger_path = temp
        .path()
        .join(".gorsee-code/sessions")
        .join(&summary.session_id)
        .join("token-ledger.json");
    let ledger: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(ledger_path).unwrap()).unwrap();
    assert_eq!(ledger["records"][0]["input_tokens"], 20);
    assert_eq!(ledger["records"][0]["output_tokens"], 5);
    assert_eq!(ledger["records"][0]["cached_tokens"], 80);
}
