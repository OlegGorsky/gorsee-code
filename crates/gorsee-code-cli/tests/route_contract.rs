use gorsee_code_cli::{run_with_options, CliOptions};

mod support;
use support::assert_product_output;

#[test]
fn route_chat_uses_single_toolless_primary_agent() {
    let temp = tempfile::tempdir().unwrap();

    let output = run_with_options(
        ["gcode", "route", "привет"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    assert!(output.contains("intent: chat"));
    assert!(output.contains("- architect "));
    assert!(!output.contains("- coder "));
    assert!(!output.contains("- validator "));
    assert_product_output(&output);
}

#[test]
fn route_edit_uses_coding_lifecycle_agents() {
    let temp = tempfile::tempdir().unwrap();

    let output = run_with_options(
        ["gcode", "route", "refactor auth and run tests"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    assert!(output.contains("intent: edit"));
    assert!(output.contains("- architect "));
    assert!(output.contains("- coder "));
    assert!(output.contains("- validator "));
    assert!(!output.contains("- summarizer "));
    assert_product_output(&output);
}

#[test]
fn route_simple_file_create_uses_fast_coding_path() {
    let temp = tempfile::tempdir().unwrap();

    let output = run_with_options(
        ["gcode", "route", "создай файл smoke.txt с текстом hello"],
        CliOptions::for_root(temp.path()),
    )
    .unwrap();

    assert!(output.contains("intent: edit"));
    assert!(!output.contains("- architect "));
    assert!(output.contains("- coder "));
    assert!(output.contains("- validator "));
    assert!(output.contains("reasoning=low"));
    assert!(output.contains("budget_tokens=12000"));
    assert_product_output(&output);
}
