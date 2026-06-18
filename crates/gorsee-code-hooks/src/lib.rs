use gorsee_code_limits::LimitDecision;
use gorsee_code_safety::Redactor;
use gorsee_code_usage::BudgetStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookPoint {
    BeforeMission,
    AfterMission,
    BeforeModelCall,
    AfterModelCall,
    BeforeTool,
    AfterTool,
    BeforePatch,
    AfterPatch,
    BeforeTest,
    AfterTest,
    OnBudgetWarning,
    OnLimitWarning,
    OnError,
    BeforeSessionExport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookDefinition {
    pub id: String,
    pub point: HookPoint,
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HookContext {
    pub text: Option<String>,
    pub budget: Option<BudgetStatus>,
    pub limit: Option<LimitDecision>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookOutcome {
    pub messages: Vec<String>,
    pub blocked: bool,
    pub redacted_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HookBus {
    redactor: Redactor,
}

impl HookBus {
    pub fn new(redactor: Redactor) -> Self {
        Self { redactor }
    }

    pub fn run(&self, point: HookPoint, context: HookContext) -> HookOutcome {
        match point {
            HookPoint::OnBudgetWarning => budget_warning(context.budget),
            HookPoint::OnLimitWarning => limit_warning(context.limit),
            HookPoint::BeforeSessionExport | HookPoint::BeforeModelCall => {
                self.redact(context.text)
            }
            HookPoint::AfterTest => test_summary(context.text),
            HookPoint::BeforePatch => {
                message("snapshot-before-patch: create checkpoint before writes")
            }
            _ => HookOutcome::default(),
        }
    }

    fn redact(&self, text: Option<String>) -> HookOutcome {
        HookOutcome {
            redacted_text: text.map(|value| self.redactor.redact(&value)),
            ..HookOutcome::default()
        }
    }
}

impl Default for HookBus {
    fn default() -> Self {
        Self::new(Redactor::default())
    }
}

pub fn builtin_hooks() -> Vec<HookDefinition> {
    vec![
        definition(
            "budget-warning",
            HookPoint::OnBudgetWarning,
            "Warn when mission budget is near stop.",
        ),
        definition(
            "limit-warning",
            HookPoint::OnLimitWarning,
            "Warn when NeuroGate limits are near stop.",
        ),
        definition(
            "redact-before-log",
            HookPoint::BeforeModelCall,
            "Redact secrets before logging.",
        ),
        definition(
            "snapshot-before-patch",
            HookPoint::BeforePatch,
            "Checkpoint before write tools.",
        ),
        definition(
            "test-result-summary",
            HookPoint::AfterTest,
            "Summarize test command output.",
        ),
        definition(
            "session-export-redaction",
            HookPoint::BeforeSessionExport,
            "Redact exported sessions.",
        ),
    ]
}

fn budget_warning(budget: Option<BudgetStatus>) -> HookOutcome {
    match budget {
        Some(status) if status.stopped => blocked("budget exceeded"),
        Some(status) if status.warning => {
            message(format!("budget warning: {:.1}% used", status.percent_used))
        }
        _ => HookOutcome::default(),
    }
}

fn limit_warning(limit: Option<LimitDecision>) -> HookOutcome {
    match limit {
        Some(LimitDecision::Stop(label)) => blocked(format!("limit stop: {label}")),
        Some(LimitDecision::Warn(label)) => message(format!("limit warning: {label}")),
        _ => HookOutcome::default(),
    }
}

fn test_summary(text: Option<String>) -> HookOutcome {
    let Some(text) = text else {
        return HookOutcome::default();
    };
    let status = if text.contains("exit_status=exit status: 0") {
        "passed"
    } else {
        "check output"
    };
    message(format!("test summary: {status}"))
}

fn definition(id: &str, point: HookPoint, description: &str) -> HookDefinition {
    HookDefinition {
        id: id.into(),
        point,
        description: description.into(),
    }
}

fn message(text: impl Into<String>) -> HookOutcome {
    HookOutcome {
        messages: vec![text.into()],
        ..HookOutcome::default()
    }
}

fn blocked(text: impl Into<String>) -> HookOutcome {
    HookOutcome {
        messages: vec![text.into()],
        blocked: true,
        redacted_text: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_hook_redacts_secrets() {
        let outcome = HookBus::default().run(
            HookPoint::BeforeSessionExport,
            HookContext {
                text: Some("token=secret".into()),
                ..HookContext::default()
            },
        );
        assert_eq!(outcome.redacted_text, Some("[REDACTED]".into()));
    }
}
