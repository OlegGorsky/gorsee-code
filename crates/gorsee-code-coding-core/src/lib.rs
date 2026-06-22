mod execution_contract;
mod execution_steps;
mod intent;
#[cfg(test)]
mod intent_tests;
mod lcp;
mod lcp_response;
mod orchestrator;
mod planning;
#[cfg(test)]
mod planning_tests;
mod transcript;
#[cfg(test)]
mod transcript_tests;
mod turn;

pub use execution_contract::{
    ExecutionContract, ExecutionEngine, ExecutionGate, FinalAnswerContract,
};
pub use intent::{route_intent, CodingIntent, IntentDecision, IntentRouter};
pub use lcp::{LcpExecutionStep, LcpTurnSnapshot, LocalCodingProtocol, LCP_VERSION};
pub use lcp_response::{
    LcpApprovalSummary, LcpDiffSummary, LcpTurnResponse, LcpTurnResponseInput, LcpUsageSnapshot,
    LcpVerificationSummary,
};
pub use orchestrator::{
    agent_roles_for_intent, select_agents_for_intent, CodingOrchestrator, OrchestrationPlan,
    TranscriptPolicy,
};
pub use planning::PlanningEngine;
pub use transcript::{TranscriptEvent, TranscriptEventKind, TranscriptMapper};
pub use turn::{
    ExecutionStepState, PlanRisk, PlanStep, PlanStepKind, TurnPlan, TurnRequest, TurnResult,
    WorkspaceRef,
};
