use crate::{route_intent, PlanningEngine};

#[test]
fn chat_has_no_structured_plan() {
    assert!(PlanningEngine
        .plan("привет", &route_intent("привет"), vec!["architect".into()])
        .is_none());
}

#[test]
fn edit_plan_requires_patch_diff_and_verification_steps() {
    let plan = PlanningEngine
        .plan(
            "создай файл",
            &route_intent("создай файл"),
            vec!["architect".into(), "coder".into(), "validator".into()],
        )
        .unwrap();
    let steps = plan
        .steps
        .iter()
        .map(|step| step.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        steps,
        ["inspect_repo", "apply_changes", "review_diff", "verify"]
    );
}

#[test]
fn plan_extracts_file_references_for_edit_targets() {
    let plan = PlanningEngine
        .plan(
            "исправь @src/main.rs и crates/app/Cargo.toml",
            &route_intent("исправь @src/main.rs и crates/app/Cargo.toml"),
            vec!["coder".into()],
        )
        .unwrap();

    assert_eq!(
        plan.files_to_inspect,
        ["src/main.rs", "crates/app/Cargo.toml"]
    );
    assert_eq!(
        plan.files_to_modify,
        ["src/main.rs", "crates/app/Cargo.toml"]
    );
}
