use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    intent::CodingIntent,
    turn::{PlanRisk, PlanStepKind, TurnPlan},
};

#[derive(Debug, Default, Clone, Copy)]
pub struct ExecutionEngine;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionContract {
    pub intent: CodingIntent,
    pub requires_plan: bool,
    pub required_tools: Vec<String>,
    pub step_gates: Vec<ExecutionGate>,
    pub diff_required: bool,
    pub verification_required: bool,
    pub final_answer: FinalAnswerContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionGate {
    pub step_id: String,
    pub kind: PlanStepKind,
    pub risk: PlanRisk,
    pub must_call_one_of: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalAnswerContract {
    pub forbid_full_code_dump: bool,
    pub must_reference_changed_files: bool,
    pub must_reference_diff: bool,
    pub must_reference_verification: bool,
    pub raw_output_in_details_only: bool,
}

impl ExecutionEngine {
    pub fn contract(&self, plan: Option<&TurnPlan>) -> ExecutionContract {
        let Some(plan) = plan else {
            return chat_contract();
        };
        let required_tools = required_tools(plan);
        let diff_required = plan.intent == CodingIntent::Edit
            || plan.intent == CodingIntent::Review
            || required_tools.iter().any(|tool| tool.starts_with("git_"));
        let verification_required = plan.intent == CodingIntent::Edit
            || plan.intent == CodingIntent::Test
            || plan.intent == CodingIntent::Release
            || required_tools.iter().any(|tool| tool == "run_test");
        ExecutionContract {
            intent: plan.intent,
            requires_plan: true,
            required_tools,
            step_gates: plan
                .steps
                .iter()
                .map(|step| ExecutionGate {
                    step_id: step.id.clone(),
                    kind: step.kind,
                    risk: step.risk,
                    must_call_one_of: step.expected_tools.clone(),
                })
                .collect(),
            diff_required,
            verification_required,
            final_answer: FinalAnswerContract {
                forbid_full_code_dump: matches!(
                    plan.intent,
                    CodingIntent::Edit | CodingIntent::Release
                ),
                must_reference_changed_files: matches!(
                    plan.intent,
                    CodingIntent::Edit | CodingIntent::Review | CodingIntent::Release
                ),
                must_reference_diff: diff_required,
                must_reference_verification: verification_required,
                raw_output_in_details_only: true,
            },
        }
    }
}

fn chat_contract() -> ExecutionContract {
    ExecutionContract {
        intent: CodingIntent::Chat,
        requires_plan: false,
        required_tools: Vec::new(),
        step_gates: Vec::new(),
        diff_required: false,
        verification_required: false,
        final_answer: FinalAnswerContract {
            forbid_full_code_dump: false,
            must_reference_changed_files: false,
            must_reference_diff: false,
            must_reference_verification: false,
            raw_output_in_details_only: true,
        },
    }
}

fn required_tools(plan: &TurnPlan) -> Vec<String> {
    plan.steps
        .iter()
        .flat_map(|step| step.expected_tools.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{route_intent, ExecutionEngine, PlanningEngine};

    #[test]
    fn edit_contract_requires_tools_diff_verification_and_no_code_dump() {
        let decision = route_intent("создай файл src/main.rs");
        let plan = PlanningEngine
            .plan("создай файл src/main.rs", &decision, vec!["coder".into()])
            .unwrap();

        let contract = ExecutionEngine.contract(Some(&plan));

        assert!(contract.requires_plan);
        assert!(contract.required_tools.contains(&"apply_patch".into()));
        assert!(contract.required_tools.contains(&"git_diff".into()));
        assert!(contract.required_tools.contains(&"run_test".into()));
        assert!(contract.diff_required);
        assert!(contract.verification_required);
        assert!(contract.final_answer.forbid_full_code_dump);
        assert!(contract.final_answer.must_reference_changed_files);
    }

    #[test]
    fn chat_contract_has_no_required_tools() {
        let contract = ExecutionEngine.contract(None);

        assert!(!contract.requires_plan);
        assert!(contract.required_tools.is_empty());
        assert!(!contract.final_answer.forbid_full_code_dump);
    }
}
