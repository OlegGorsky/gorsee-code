use gorsee_code_coding_core::{CodingIntent, ExecutionContract, FinalAnswerContract};
use gorsee_code_core::{AgentProfile, AgentRole};
use gorsee_code_safety::RiskClass;
use gorsee_code_tool_runtime::{ToolManifest, ToolOutput};

use crate::protocol::ToolResult;

use super::*;

#[test]
fn edit_final_answer_without_patch_gets_policy_retry() {
    let feedback = final_answer_policy_retry(
        &coder(),
        "напиши простой бот",
        &[apply_patch_manifest()],
        &[],
        &[],
        "```python\nprint('hi')\n```",
        &edit_contract(),
    )
    .expect("policy feedback");

    assert_eq!(feedback.name, "execution_policy");
    assert!(!feedback.ok);
    assert!(feedback.text.contains("apply_patch"));
}

#[test]
fn coder_edit_final_answer_without_patch_gets_policy_retry_even_without_code_dump() {
    let feedback = final_answer_policy_retry(
        &coder(),
        "измени файл",
        &[apply_patch_manifest()],
        &[],
        &[],
        "Готово.",
        &edit_contract(),
    )
    .expect("policy feedback");

    assert_eq!(feedback.name, "execution_policy");
    assert!(feedback.text.contains("must change files"));
}

#[test]
fn edit_final_answer_after_patch_is_allowed() {
    let feedback = final_answer_policy_retry(
        &coder(),
        "напиши простой бот",
        &[apply_patch_manifest()],
        &[],
        &[ToolResult::output(
            "coder",
            "apply_patch",
            true,
            ToolOutput::text("wrote file"),
        )],
        "Готово: файл создан.",
        &edit_contract(),
    );

    assert!(feedback.is_none());
}

#[test]
fn coder_after_patch_can_handoff_without_running_validator_gates() {
    let feedback = final_answer_policy_retry(
        &coder(),
        "измени файл",
        &[
            apply_patch_manifest(),
            git_diff_manifest(),
            run_test_manifest(),
        ],
        &[],
        &[ToolResult::output(
            "coder",
            "apply_patch",
            true,
            ToolOutput::text("wrote file"),
        )],
        "Изменён src/lib.rs. Передаю diff и проверки Validator.",
        &edit_contract(),
    );

    assert!(feedback.is_none());
}

#[test]
fn validator_final_answer_requires_diff_and_verification_gates() {
    let feedback = final_answer_policy_retry(
        &validator(),
        "измени файл",
        &[git_diff_manifest(), run_test_manifest()],
        &[ToolResult::output(
            "coder",
            "apply_patch",
            true,
            ToolOutput::text("wrote file"),
        )],
        &[],
        "Готово.",
        &edit_contract(),
    )
    .expect("policy feedback");

    assert_eq!(feedback.name, "execution_policy");
    assert!(feedback.text.contains("git_diff"));
}

#[test]
fn validator_final_answer_requires_concrete_summary_after_diff_and_verification() {
    let feedback = final_answer_policy_retry(
        &validator(),
        "измени файл",
        &[git_diff_manifest(), run_test_manifest()],
        &[ToolResult::output(
            "coder",
            "apply_patch",
            true,
            ToolOutput::text("wrote file"),
        )],
        &[
            ToolResult::output("validator", "git_diff", true, ToolOutput::text("diff")),
            ToolResult::output("validator", "run_test", true, ToolOutput::text("ok")),
        ],
        "Готово.",
        &edit_contract(),
    )
    .expect("policy feedback");

    assert_eq!(feedback.name, "execution_policy");
    assert!(feedback.text.contains("changed files"));
    assert!(feedback.text.contains("diff status"));
    assert!(feedback.text.contains("verification"));
}

#[test]
fn validator_final_answer_allowed_with_files_diff_and_verification_summary() {
    let feedback = final_answer_policy_retry(
        &validator(),
        "измени файл",
        &[git_diff_manifest(), run_test_manifest()],
        &[ToolResult::output(
            "coder",
            "apply_patch",
            true,
            ToolOutput::text("wrote file"),
        )],
        &[
            ToolResult::output("validator", "git_diff", true, ToolOutput::text("diff")),
            ToolResult::output("validator", "run_test", true, ToolOutput::text("ok")),
        ],
        "Изменён src/lib.rs. Diff: 1 файл, +4 -1. Проверки: cargo test прошёл.",
        &edit_contract(),
    );

    assert!(feedback.is_none());
}

#[test]
fn validator_final_answer_rejects_code_dump_after_required_gates() {
    let feedback = final_answer_policy_retry(
        &validator(),
        "напиши простой бот",
        &[git_diff_manifest(), run_test_manifest()],
        &[ToolResult::output(
            "coder",
            "apply_patch",
            true,
            ToolOutput::text("wrote file"),
        )],
        &[
            ToolResult::output("validator", "git_diff", true, ToolOutput::text("diff")),
            ToolResult::output("validator", "run_test", true, ToolOutput::text("ok")),
        ],
        "Изменён bot.py. Diff: 1 файл. Проверки: cargo test прошёл.\n```python\nprint('hi')\n```",
        &edit_contract(),
    )
    .expect("policy feedback");

    assert_eq!(feedback.name, "execution_policy");
    assert!(feedback.text.contains("without dumping full source code"));
}

fn coder() -> AgentProfile {
    AgentProfile {
        role: AgentRole::Coder,
        model: "test".into(),
        reasoning: "low".into(),
        tools: vec!["propose_patch".into()],
        budget_tokens: 1000,
        temperature: 0.0,
    }
}

fn validator() -> AgentProfile {
    AgentProfile {
        role: AgentRole::Validator,
        model: "test".into(),
        reasoning: "low".into(),
        tools: vec!["diff".into(), "run_test".into()],
        budget_tokens: 1000,
        temperature: 0.0,
    }
}

fn apply_patch_manifest() -> ToolManifest {
    ToolManifest {
        name: "apply_patch".into(),
        description: "write files".into(),
        risk: RiskClass::Write,
        capabilities: vec!["files:write".into()],
    }
}

fn git_diff_manifest() -> ToolManifest {
    ToolManifest {
        name: "git_diff".into(),
        description: "diff".into(),
        risk: RiskClass::Read,
        capabilities: vec!["git:diff".into()],
    }
}

fn run_test_manifest() -> ToolManifest {
    ToolManifest {
        name: "run_test".into(),
        description: "tests".into(),
        risk: RiskClass::Command,
        capabilities: vec!["tests:run".into()],
    }
}

fn edit_contract() -> ExecutionContract {
    ExecutionContract {
        intent: CodingIntent::Edit,
        requires_plan: true,
        required_tools: vec![],
        step_gates: vec![],
        diff_required: true,
        verification_required: true,
        final_answer: FinalAnswerContract {
            forbid_full_code_dump: true,
            must_reference_changed_files: true,
            must_reference_diff: true,
            must_reference_verification: true,
            raw_output_in_details_only: true,
        },
    }
}
