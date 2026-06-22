use gorsee_code_core::{AgentProfile, AgentRole};
use serde::{Deserialize, Serialize};

use crate::{
    intent::{route_intent, CodingIntent, IntentDecision},
    planning::PlanningEngine,
    turn::{TurnPlan, TurnRequest},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptPolicy {
    ChatOnly,
    Coding,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrchestrationPlan {
    pub request: TurnRequest,
    pub intent: IntentDecision,
    pub agents: Vec<AgentProfile>,
    pub plan: Option<TurnPlan>,
    pub transcript_policy: TranscriptPolicy,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CodingOrchestrator;

impl CodingOrchestrator {
    pub fn plan_turn(
        &self,
        request: TurnRequest,
        profiles: Vec<AgentProfile>,
    ) -> OrchestrationPlan {
        let intent = route_intent(&request.message);
        let agents = select_agents_for_request(profiles, &intent, &request.message);
        let agent_ids = agents.iter().map(|agent| agent.id().to_string()).collect();
        let plan = PlanningEngine.plan(&request.message, &intent, agent_ids);
        let transcript_policy = match intent.intent {
            CodingIntent::Chat => TranscriptPolicy::ChatOnly,
            _ => TranscriptPolicy::Coding,
        };
        OrchestrationPlan {
            request,
            intent,
            agents,
            plan,
            transcript_policy,
        }
    }
}

pub fn select_agents_for_intent(
    profiles: Vec<AgentProfile>,
    decision: &IntentDecision,
) -> Vec<AgentProfile> {
    select_agents(
        profiles,
        agent_roles_for_intent(decision.intent),
        decision.intent,
    )
}

fn select_agents_for_request(
    profiles: Vec<AgentProfile>,
    decision: &IntentDecision,
    message: &str,
) -> Vec<AgentProfile> {
    let roles = agent_roles_for_request(decision.intent, message);
    select_agents(profiles, roles, decision.intent)
}

fn select_agents(
    profiles: Vec<AgentProfile>,
    roles: Vec<AgentRole>,
    intent: CodingIntent,
) -> Vec<AgentProfile> {
    let mut selected = roles
        .iter()
        .filter_map(|role| {
            profiles
                .iter()
                .find(|profile| &profile.role == role)
                .cloned()
                .map(|profile| configure_for_intent(profile, intent))
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        selected.extend(profiles.into_iter().take(1));
    }
    selected
}

pub fn agent_roles_for_intent(intent: CodingIntent) -> Vec<AgentRole> {
    match intent {
        CodingIntent::Chat => vec![AgentRole::Architect],
        CodingIntent::Inspect => vec![AgentRole::Architect, AgentRole::Scout],
        CodingIntent::Edit => vec![AgentRole::Architect, AgentRole::Coder, AgentRole::Validator],
        CodingIntent::Test => vec![AgentRole::Validator],
        CodingIntent::Review => vec![AgentRole::Validator, AgentRole::Summarizer],
        CodingIntent::Release => vec![AgentRole::Validator, AgentRole::Summarizer],
    }
}

fn agent_roles_for_request(intent: CodingIntent, message: &str) -> Vec<AgentRole> {
    if intent == CodingIntent::Edit && is_simple_file_edit(message) {
        return vec![AgentRole::Coder, AgentRole::Validator];
    }
    agent_roles_for_intent(intent)
}

fn configure_for_intent(mut profile: AgentProfile, intent: CodingIntent) -> AgentProfile {
    match intent {
        CodingIntent::Chat => {
            profile.tools.clear();
            profile.reasoning = "low".into();
            profile.budget_tokens = profile.budget_tokens.min(8_000);
            profile.temperature = profile.temperature.min(0.2);
        }
        CodingIntent::Edit if matches!(profile.role, AgentRole::Coder | AgentRole::Validator) => {
            profile.reasoning = "low".into();
            profile.budget_tokens = profile.budget_tokens.min(12_000);
            profile.temperature = profile.temperature.min(0.1);
        }
        _ => {}
    }
    profile
}

fn is_simple_file_edit(message: &str) -> bool {
    let normalized = message
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.len() > 220 {
        return false;
    }
    let create_file = ["создай файл", "создать файл", "create file", "write file"]
        .iter()
        .any(|needle| normalized.contains(needle));
    let has_content = ["с текст", "текстом", "with text", "containing"]
        .iter()
        .any(|needle| normalized.contains(needle));
    let complex = [
        "refactor",
        "рефактор",
        "тест",
        "test",
        "исправ",
        "почин",
        "реализ",
        "implement",
        "проект",
        "архитект",
    ]
    .iter()
    .any(|needle| normalized.contains(needle));
    create_file && has_content && !complex
}

#[cfg(test)]
mod tests {
    use gorsee_code_core::{default_agent_matrix, AgentRole};

    use super::*;
    use crate::route_intent;

    #[test]
    fn chat_uses_one_toolless_primary_agent() {
        let agents = select_agents_for_intent(default_agent_matrix(), &route_intent("привет"));
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].role, AgentRole::Architect);
        assert!(agents[0].tools.is_empty());
        assert!(agents[0].budget_tokens <= 8_000);
    }

    #[test]
    fn edit_uses_coding_lifecycle_agents() {
        let agents =
            select_agents_for_intent(default_agent_matrix(), &route_intent("создай модуль diff"));
        let roles = agents
            .into_iter()
            .map(|agent| agent.role)
            .collect::<Vec<_>>();
        assert_eq!(
            roles,
            vec![AgentRole::Architect, AgentRole::Coder, AgentRole::Validator]
        );
    }

    #[test]
    fn simple_file_create_skips_architect_and_uses_small_budgets() {
        let request = TurnRequest {
            workspace: crate::WorkspaceRef {
                root: ".".into(),
                branch: None,
                session_id: None,
            },
            message: "создай файл smoke.txt с текстом hello".into(),
            user_id: None,
        };
        let plan = CodingOrchestrator.plan_turn(request, default_agent_matrix());
        let roles = plan
            .agents
            .iter()
            .map(|agent| agent.role.clone())
            .collect::<Vec<_>>();

        assert_eq!(roles, vec![AgentRole::Coder, AgentRole::Validator]);
        assert!(plan.agents.iter().all(|agent| agent.reasoning == "low"));
        assert!(plan
            .agents
            .iter()
            .all(|agent| agent.budget_tokens <= 12_000));
    }

    #[test]
    fn review_does_not_run_full_matrix() {
        let agents = select_agents_for_intent(default_agent_matrix(), &route_intent("покажи diff"));
        let roles = agents
            .into_iter()
            .map(|agent| agent.role)
            .collect::<Vec<_>>();
        assert_eq!(roles, vec![AgentRole::Validator, AgentRole::Summarizer]);
    }
}
