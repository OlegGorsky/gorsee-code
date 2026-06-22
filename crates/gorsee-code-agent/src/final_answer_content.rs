use gorsee_code_coding_core::FinalAnswerContract;

pub(crate) fn final_answer_contract_feedback(
    answer: &str,
    contract: &FinalAnswerContract,
) -> Option<String> {
    if contract.forbid_full_code_dump && looks_like_full_code_dump(answer) {
        return Some("final answer must summarize changed files, diff, and checks without dumping full source code".into());
    }
    let mut missing = Vec::new();
    if contract.must_reference_changed_files && !mentions_changed_files(answer) {
        missing.push("changed files");
    }
    if contract.must_reference_diff && !mentions_diff(answer) {
        missing.push("diff status");
    }
    if contract.must_reference_verification && !mentions_verification(answer) {
        missing.push("verification/check status");
    }
    (!missing.is_empty()).then(|| {
        format!(
            "final answer must briefly mention {}; do not dump full source code",
            missing.join(", ")
        )
    })
}

fn mentions_changed_files(answer: &str) -> bool {
    contains_any(
        answer,
        &[
            "file",
            "files",
            "changed",
            "updated",
            "created",
            "файл",
            "файлы",
            "измен",
            "обнов",
            "создан",
            "добав",
        ],
    ) || mentions_path(answer)
}

fn mentions_diff(answer: &str) -> bool {
    contains_any(
        answer,
        &[
            "diff",
            "patch",
            "changes",
            "дифф",
            "патч",
            "изменения",
            "изменений",
        ],
    )
}

fn mentions_verification(answer: &str) -> bool {
    contains_any(
        answer,
        &[
            "test",
            "tests",
            "check",
            "checks",
            "verification",
            "run_test",
            "тест",
            "провер",
            "пропущ",
            "не запускал",
            "отклон",
        ],
    )
}

fn looks_like_full_code_dump(answer: &str) -> bool {
    if answer.contains("```") {
        return true;
    }
    let code_like_lines = answer
        .lines()
        .filter(|line| is_code_like_line(line.trim()))
        .count();
    code_like_lines >= 5 || contains_code_construct(answer)
}

fn is_code_like_line(line: &str) -> bool {
    if line.is_empty() {
        return false;
    }
    line.starts_with("use ")
        || line.starts_with("import ")
        || line.starts_with("from ")
        || line.starts_with("fn ")
        || line.starts_with("pub ")
        || line.starts_with("def ")
        || line.starts_with("class ")
        || line.starts_with("const ")
        || line.starts_with("let ")
        || line.starts_with("var ")
        || line.ends_with(';')
        || line.contains(" = ")
        || line.contains("=>")
}

fn contains_code_construct(answer: &str) -> bool {
    let lower = answer.to_lowercase();
    [
        "pub fn ",
        "fn main(",
        "def main(",
        "class ",
        "import ",
        "from ",
        "#!/usr/bin/env",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn contains_any(answer: &str, needles: &[&str]) -> bool {
    let lower = answer.to_lowercase();
    needles.iter().any(|needle| lower.contains(needle))
}

fn mentions_path(answer: &str) -> bool {
    answer.split_whitespace().any(|part| {
        let trimmed = part.trim_matches(|ch: char| {
            matches!(ch, ',' | '.' | ':' | ';' | ')' | '(' | '[' | ']' | '`')
        });
        trimmed.contains('/') || trimmed.rsplit_once('.').is_some_and(has_known_extension)
    })
}

fn has_known_extension((_, extension): (&str, &str)) -> bool {
    matches!(
        extension,
        "rs" | "toml" | "md" | "json" | "yaml" | "yml" | "ts" | "tsx" | "js" | "jsx" | "py" | "go"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_concrete_coding_summary() {
        let feedback = final_answer_contract_feedback(
            "Изменён src/lib.rs. Diff: 1 файл, +4 -1. Проверки: cargo test прошёл.",
            &contract(),
        );

        assert!(feedback.is_none());
    }

    #[test]
    fn rejects_empty_coding_summary() {
        let feedback =
            final_answer_contract_feedback("Готово.", &contract()).expect("policy feedback");

        assert!(feedback.contains("changed files"));
        assert!(feedback.contains("diff status"));
        assert!(feedback.contains("verification"));
    }

    #[test]
    fn rejects_full_code_dump_even_with_summary_words() {
        let feedback = final_answer_contract_feedback(
            "Изменён src/bot.py. Diff: 1 файл. Проверки: пропущены.\n```python\nimport time\nprint('hi')\n```",
            &contract(),
        )
        .expect("policy feedback");

        assert!(feedback.contains("without dumping full source code"));
    }

    fn contract() -> FinalAnswerContract {
        FinalAnswerContract {
            forbid_full_code_dump: true,
            must_reference_changed_files: true,
            must_reference_diff: true,
            must_reference_verification: true,
            raw_output_in_details_only: true,
        }
    }
}
