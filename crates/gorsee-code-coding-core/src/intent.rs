use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingIntent {
    Chat,
    Inspect,
    Edit,
    Test,
    Review,
    Release,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentDecision {
    pub intent: CodingIntent,
    pub confidence: f32,
    pub requires_tools: bool,
    pub requires_write: bool,
    pub requires_approval: bool,
    pub reason: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct IntentRouter;

impl IntentRouter {
    pub fn route(&self, input: &str) -> IntentDecision {
        route_intent(input)
    }
}

pub fn route_intent(input: &str) -> IntentDecision {
    let value = normalize(input);
    if value.is_empty() {
        return decision(CodingIntent::Chat, 0.4, false, false, false, "empty input");
    }
    if value.starts_with('/') {
        return decision(
            CodingIntent::Inspect,
            0.8,
            false,
            false,
            false,
            "slash command should be handled by command layer",
        );
    }
    if contains_any(&value, release_words()) {
        return decision(
            CodingIntent::Release,
            0.92,
            true,
            true,
            true,
            "release or publishing request",
        );
    }
    if contains_any(&value, audit_words()) {
        return decision(
            CodingIntent::Review,
            0.86,
            true,
            false,
            false,
            "diff or review request",
        );
    }
    if contains_any(&value, edit_words()) {
        return decision(
            CodingIntent::Edit,
            0.9,
            true,
            true,
            true,
            "workspace edit request",
        );
    }
    if contains_any(&value, review_words()) {
        return decision(
            CodingIntent::Review,
            0.86,
            true,
            false,
            false,
            "diff or review request",
        );
    }
    if contains_any(&value, test_words()) {
        return decision(
            CodingIntent::Test,
            0.84,
            true,
            false,
            false,
            "verification or failing-test request",
        );
    }
    if contains_any(&value, inspect_words()) {
        return decision(
            CodingIntent::Inspect,
            0.78,
            true,
            false,
            false,
            "project inspection request",
        );
    }
    if is_short_conversation(&value) {
        return decision(
            CodingIntent::Chat,
            0.76,
            false,
            false,
            false,
            "short conversational message",
        );
    }
    decision(
        CodingIntent::Inspect,
        0.55,
        true,
        false,
        false,
        "ambiguous request defaults to read-only inspection",
    )
}

fn decision(
    intent: CodingIntent,
    confidence: f32,
    requires_tools: bool,
    requires_write: bool,
    requires_approval: bool,
    reason: &str,
) -> IntentDecision {
    IntentDecision {
        intent,
        confidence,
        requires_tools,
        requires_write,
        requires_approval,
        reason: reason.into(),
    }
}

fn normalize(input: &str) -> String {
    input
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_any(value: &str, words: &[&str]) -> bool {
    words.iter().any(|word| value.contains(word))
}

fn is_short_conversation(value: &str) -> bool {
    if value.len() > 180 || value.lines().count() > 2 {
        return false;
    }
    let words = value.split_whitespace().count();
    words <= 18 || contains_any(value, chat_prefixes())
}

fn chat_prefixes() -> &'static [&'static str] {
    &[
        "привет",
        "здравств",
        "добрый день",
        "доброе утро",
        "добрый вечер",
        "hello",
        "hi",
        "hey",
        "как дела",
        "спасибо",
        "ок",
        "ладно",
        "понял",
    ]
}

fn release_words() -> &'static [&'static str] {
    &[
        "заком",
        "коммит",
        "commit",
        "зарелиз",
        "релиз",
        "release",
        "publish",
        "npm",
        "github",
        "tag",
        "push",
    ]
}

fn review_words() -> &'static [&'static str] {
    &[
        "diff",
        "диф",
        "review",
        "ревью",
        "что измен",
        "покажи изменения",
        "git status",
        "статус git",
    ]
}

fn audit_words() -> &'static [&'static str] {
    &["аудит", "code review", "проведи ревью"]
}

fn edit_words() -> &'static [&'static str] {
    &[
        "добав",
        "исправ",
        "почин",
        "сдел",
        "созда",
        "напиши",
        "реализ",
        "измени",
        "удали",
        "перенеси",
        "обнови",
        "внедр",
        "подключ",
        "зарефактор",
        "refactor",
        "fix",
        "bug",
        "write",
        "create",
        "implement",
        "add",
        "remove",
        "update",
    ]
}

fn test_words() -> &'static [&'static str] {
    &[
        "проверь",
        "запусти тест",
        "тест",
        "падает",
        "ошиб",
        "cargo test",
        "cargo check",
        "clippy",
        "pytest",
        "test",
    ]
}

fn inspect_words() -> &'static [&'static str] {
    &[
        "посмотри",
        "найди",
        "покажи",
        "объясни",
        "изучи",
        "проанализ",
        "папк",
        "файл",
        "find",
        "inspect",
        "explain",
        "analyze",
        "read",
    ]
}
