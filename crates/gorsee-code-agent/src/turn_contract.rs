use gorsee_code_coding_core::{
    ExecutionContract, ExecutionEngine, LocalCodingProtocol, TurnPlan, TurnRequest, WorkspaceRef,
};
use gorsee_code_core::{AgentProfile, TaskSpec};

pub(crate) struct TurnExecutionContext {
    pub(crate) plan: Option<TurnPlan>,
    pub(crate) contract: ExecutionContract,
}

pub(crate) fn turn_execution_context(
    spec: &TaskSpec,
    agents: &[AgentProfile],
) -> TurnExecutionContext {
    let snapshot = LocalCodingProtocol::default().plan_turn(turn_request(spec), agents.to_vec());
    let plan = snapshot.orchestration.plan;
    let contract = ExecutionEngine.contract(plan.as_ref());
    TurnExecutionContext { plan, contract }
}

fn turn_request(spec: &TaskSpec) -> TurnRequest {
    TurnRequest {
        workspace: WorkspaceRef {
            root: spec.repo_path.clone(),
            branch: None,
            session_id: None,
        },
        message: spec.objective.clone(),
        user_id: None,
    }
}
