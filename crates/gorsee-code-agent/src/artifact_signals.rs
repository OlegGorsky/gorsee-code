use gorsee_code_diff::WorkspaceDiff;
use serde_json::Value;

pub(crate) struct DiffSignal {
    pub(crate) emit: bool,
    pub(crate) status: String,
    pub(crate) summary: String,
    pub(crate) files_changed: usize,
    pub(crate) additions: usize,
    pub(crate) deletions: usize,
    pub(crate) artifact: &'static str,
}

pub(crate) struct VerificationSignal {
    pub(crate) emit: bool,
    pub(crate) status: String,
    pub(crate) summary: String,
    pub(crate) command: Option<String>,
    pub(crate) artifact: &'static str,
}

pub(crate) fn diff_signal_ok(diff: &WorkspaceDiff, required: bool) -> DiffSignal {
    let files_changed = diff.summary.files_changed;
    let summary = if files_changed == 0 {
        "diff чистый".into()
    } else {
        format!(
            "diff готов: {} файлов, +{} -{}",
            files_changed, diff.summary.additions, diff.summary.deletions
        )
    };
    DiffSignal {
        emit: required,
        status: if files_changed == 0 { "clean" } else { "ok" }.into(),
        summary,
        files_changed,
        additions: diff.summary.additions,
        deletions: diff.summary.deletions,
        artifact: "diff.json",
    }
}

pub(crate) fn diff_signal_unavailable(error: impl std::fmt::Display, required: bool) -> DiffSignal {
    DiffSignal {
        emit: required,
        status: "unavailable".into(),
        summary: format!("diff недоступен: {error}"),
        files_changed: 0,
        additions: 0,
        deletions: 0,
        artifact: "diff.json",
    }
}

pub(crate) fn diff_signal_not_required() -> DiffSignal {
    DiffSignal {
        emit: false,
        status: "not_required".into(),
        summary: "diff не требуется".into(),
        files_changed: 0,
        additions: 0,
        deletions: 0,
        artifact: "diff.json",
    }
}

pub(crate) fn verification_signal(value: &Value, required: bool) -> VerificationSignal {
    let status = value_text(value, "status").unwrap_or_else(|| "unknown".into());
    let command = value_text(value, "command");
    let reason = value_text(value, "reason");
    let label = command
        .as_deref()
        .or(reason.as_deref())
        .unwrap_or("details");
    let summary = match status.as_str() {
        "passed" => format!("проверки пройдены: {label}"),
        "failed" => format!("проверки не прошли: {label}"),
        "skipped" => format!("проверки пропущены: {label}"),
        _ => format!("проверки: {status}"),
    };
    VerificationSignal {
        emit: required || status != "skipped",
        status,
        summary,
        command,
        artifact: "verification.json",
    }
}

fn value_text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use gorsee_code_diff::{DiffSummary, WorkspaceDiff};

    use super::*;

    #[test]
    fn dirty_workspace_diff_is_not_emitted_for_chat_contract() {
        let signal = diff_signal_ok(
            &WorkspaceDiff {
                summary: DiffSummary {
                    files_changed: 1,
                    additions: 3,
                    deletions: 1,
                },
                files: Vec::new(),
            },
            false,
        );

        assert!(!signal.emit);
        assert_eq!(signal.files_changed, 1);
    }
}
