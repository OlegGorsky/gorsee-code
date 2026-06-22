use crate::{
    intent::{CodingIntent, IntentDecision},
    turn::{IntentDecisionSnapshot, PlanRisk, PlanStep, PlanStepKind, TurnPlan},
};

#[derive(Debug, Default, Clone, Copy)]
pub struct PlanningEngine;

impl PlanningEngine {
    pub fn plan(
        &self,
        goal: &str,
        decision: &IntentDecision,
        agents: Vec<String>,
    ) -> Option<TurnPlan> {
        if decision.intent == CodingIntent::Chat {
            return None;
        }
        let files = referenced_files(goal);
        Some(TurnPlan {
            goal: goal.trim().to_string(),
            summary: summary_for(decision.intent),
            intent: decision.intent,
            decision: IntentDecisionSnapshot::from(decision),
            steps: steps_for(decision.intent),
            files_to_inspect: inspect_targets(decision.intent, &files),
            files_to_modify: modify_targets(decision.intent, &files),
            verification: verification_for(decision.intent),
            agents,
        })
    }
}

fn summary_for(intent: CodingIntent) -> String {
    match intent {
        CodingIntent::Chat => "ответить в чате".into(),
        CodingIntent::Inspect => "изучить проект без изменений".into(),
        CodingIntent::Edit => {
            "изучить проект, внести изменения через tools, показать diff и проверки".into()
        }
        CodingIntent::Test => "запустить проверки и объяснить результат".into(),
        CodingIntent::Review => "проанализировать текущий diff/git state".into(),
        CodingIntent::Release => {
            "проверить состояние, подготовить commit/release flow через подтверждаемые команды"
                .into()
        }
    }
}

fn steps_for(intent: CodingIntent) -> Vec<PlanStep> {
    match intent {
        CodingIntent::Chat => Vec::new(),
        CodingIntent::Inspect => vec![
            step(
                "inspect_repo",
                PlanStepKind::Read,
                "осмотреть структуру проекта",
                &["repo_map"],
                PlanRisk::Read,
            ),
            step(
                "search_context",
                PlanStepKind::Search,
                "найти релевантные файлы",
                &["search_text"],
                PlanRisk::Read,
            ),
        ],
        CodingIntent::Edit => vec![
            step(
                "inspect_repo",
                PlanStepKind::Read,
                "понять текущую реализацию",
                &["list_files", "read_file", "search_text"],
                PlanRisk::Read,
            ),
            step(
                "apply_changes",
                PlanStepKind::Edit,
                "изменить файлы через patch/write tools",
                &["propose_patch", "apply_patch"],
                PlanRisk::Write,
            ),
            step(
                "review_diff",
                PlanStepKind::Verify,
                "проверить структурный diff",
                &["git_diff", "git_changed_files"],
                PlanRisk::Read,
            ),
            step(
                "verify",
                PlanStepKind::Verify,
                "запустить подходящие проверки",
                &["run_test"],
                PlanRisk::Command,
            ),
        ],
        CodingIntent::Test => vec![
            step(
                "run_checks",
                PlanStepKind::Command,
                "запустить проверки",
                &["run_test"],
                PlanRisk::Command,
            ),
            step(
                "inspect_failures",
                PlanStepKind::Read,
                "разобрать ошибки без записи файлов",
                &["read_file", "search_text"],
                PlanRisk::Read,
            ),
        ],
        CodingIntent::Review => vec![
            step(
                "read_diff",
                PlanStepKind::Read,
                "прочитать измененные файлы и diff",
                &["git_diff", "git_changed_files"],
                PlanRisk::Read,
            ),
            step(
                "summarize_risks",
                PlanStepKind::Verify,
                "выдать review-выводы",
                &[],
                PlanRisk::Read,
            ),
        ],
        CodingIntent::Release => vec![
            step(
                "review_state",
                PlanStepKind::Read,
                "проверить git/diff состояние",
                &["git_status", "git_diff"],
                PlanRisk::Read,
            ),
            step(
                "verify",
                PlanStepKind::Verify,
                "запустить релизные проверки",
                &["run_test"],
                PlanRisk::Command,
            ),
            step(
                "release_commands",
                PlanStepKind::Command,
                "выполнить release-команды после подтверждения",
                &["run_command"],
                PlanRisk::Command,
            ),
        ],
    }
}

fn verification_for(intent: CodingIntent) -> Vec<String> {
    match intent {
        CodingIntent::Edit | CodingIntent::Test | CodingIntent::Release => {
            vec!["auto-detect checks".into()]
        }
        _ => Vec::new(),
    }
}

fn inspect_targets(intent: CodingIntent, files: &[String]) -> Vec<String> {
    match intent {
        CodingIntent::Chat => Vec::new(),
        _ => files.to_vec(),
    }
}

fn modify_targets(intent: CodingIntent, files: &[String]) -> Vec<String> {
    match intent {
        CodingIntent::Edit | CodingIntent::Release => files.to_vec(),
        _ => Vec::new(),
    }
}

fn referenced_files(goal: &str) -> Vec<String> {
    let mut files = Vec::new();
    for token in goal.split_whitespace() {
        let candidate = token
            .trim_matches(|ch: char| {
                matches!(
                    ch,
                    ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}' | '"' | '\'' | '`'
                )
            })
            .trim_start_matches('@')
            .trim();
        if looks_like_file_ref(candidate) && !files.iter().any(|file| file == candidate) {
            files.push(candidate.to_string());
        }
    }
    files
}

fn looks_like_file_ref(value: &str) -> bool {
    if value.is_empty() || value.starts_with("http://") || value.starts_with("https://") {
        return false;
    }
    value.contains('/') || file_extension(value).is_some_and(is_known_file_extension)
}

fn file_extension(value: &str) -> Option<&str> {
    value.rsplit_once('.').map(|(_, extension)| extension)
}

fn is_known_file_extension(extension: &str) -> bool {
    matches!(
        extension,
        "rs" | "toml"
            | "json"
            | "md"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "py"
            | "go"
            | "java"
            | "kt"
            | "swift"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "css"
            | "scss"
            | "html"
            | "vue"
            | "svelte"
            | "yaml"
            | "yml"
            | "sh"
            | "sql"
    )
}

fn step(
    id: &str,
    kind: PlanStepKind,
    description: &str,
    expected_tools: &[&str],
    risk: PlanRisk,
) -> PlanStep {
    PlanStep {
        id: id.into(),
        kind,
        description: description.into(),
        expected_tools: expected_tools.iter().map(|tool| (*tool).into()).collect(),
        risk,
    }
}
