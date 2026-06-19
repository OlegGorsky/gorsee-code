use std::path::Path;

use gorsee_code_artifacts::{ArtifactError, ArtifactRecord, ArtifactStore};
use gorsee_code_core::TaskSpec;
use gorsee_code_safety::Redactor;

use crate::protocol::{AgentAnswer, ToolResult};

pub(crate) fn write_report(
    session_dir: &Path,
    spec: &TaskSpec,
    skill_id: Option<&str>,
    answers: &[AgentAnswer],
    results: &[ToolResult],
) -> Result<ArtifactRecord, ArtifactError> {
    let text = report_text(spec, skill_id, answers, results);
    let text = Redactor::default().redact(&text);
    ArtifactStore::new(session_dir.join("artifacts")).write_named_text(
        "report.md",
        "text/markdown",
        &text,
    )
}

fn report_text(
    spec: &TaskSpec,
    skill_id: Option<&str>,
    answers: &[AgentAnswer],
    results: &[ToolResult],
) -> String {
    let mut text = format!(
        "# Gorsee Code Run Report\n\n- Request: {}\n- Skill: {}\n",
        spec.objective,
        skill_id.unwrap_or("none")
    );
    append_agent_answers(&mut text, answers);
    append_tool_results(&mut text, results);
    text
}

fn append_agent_answers(text: &mut String, answers: &[AgentAnswer]) {
    if answers.is_empty() {
        return;
    }
    text.push_str("\n## Agent Results\n");
    for answer in answers {
        text.push_str(&format!("\n### {}\n\n{}\n", answer.agent_id, answer.answer));
    }
}

fn append_tool_results(text: &mut String, results: &[ToolResult]) {
    if results.is_empty() {
        return;
    }
    text.push_str("\n## Tool Results\n");
    for result in results {
        let status = if result.ok { "ok" } else { "error" };
        text.push_str(&format!(
            "\n### {} / {} ({status})\n\n",
            result.agent_id, result.name
        ));
        text.push_str("```text\n");
        text.push_str(&result.text);
        text.push_str("\n```\n");
    }
}
