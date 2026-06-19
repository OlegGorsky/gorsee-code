use gorsee_code_ui_state::WorkspaceState;

use crate::WorkspaceApp;

pub fn render_app(state: &WorkspaceState, app: &WorkspaceApp) -> String {
    let mut out = render_workspace(state);
    out.push('\n');
    out.push_str("What should Gorsee Code do?\n");
    out.push_str("> ");
    out.push_str(app.input());
    out.push('\n');
    out.push_str("Enter run | /help commands | a approve | d deny | p pause | r resume | q quit\n");
    if let Some(status) = app.status() {
        out.push_str(&format!("Status: {status}\n"));
    }
    if let Some(output) = app.output() {
        out.push('\n');
        out.push_str("Output\n");
        out.push_str(output);
        if !output.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

pub fn render_workspace(state: &WorkspaceState) -> String {
    let mut out = String::new();
    push_header(&mut out, state);
    push_agents(&mut out, state);
    push_timeline(&mut out, state);
    push_approvals(&mut out, state);
    push_inspector(&mut out, state);
    out
}

fn push_header(out: &mut String, state: &WorkspaceState) {
    out.push_str("Gorsee Code Workspace\n");
    out.push_str(&format!(
        "Session {} | Status {} | Repo {} | Branch {}\n",
        state.session.id, state.session.status, state.session.repo, state.session.branch
    ));
    out.push_str(&format!(
        "Security workspace approvals | Gateway {}\n\n",
        state.gateway_status
    ));
}

fn push_agents(out: &mut String, state: &WorkspaceState) {
    out.push_str("Agents\n");
    for agent in &state.agents {
        out.push_str(&format!(
            "- {} role={} status={} model={} tokens={}/{}\n",
            agent.id, agent.role, agent.status, agent.model, agent.tokens_used, agent.tokens_limit
        ));
    }
    out.push('\n');
}

fn push_timeline(out: &mut String, state: &WorkspaceState) {
    out.push_str("Timeline\n");
    for event in &state.timeline {
        let agent = event.agent_id.as_deref().unwrap_or("session");
        out.push_str(&format!(
            "- #{:04} {} {}: {}\n",
            event.sequence, event.kind, agent, event.summary
        ));
    }
    out.push('\n');
}

fn push_approvals(out: &mut String, state: &WorkspaceState) {
    out.push_str("Approvals\n");
    if state.approvals.is_empty() {
        out.push_str("- none\n\n");
        return;
    }
    for approval in &state.approvals {
        out.push_str(&format!(
            "- {} tool={} status={} risk={}\n",
            approval.id, approval.name, approval.status, approval.risk
        ));
        out.push_str(&format!("  approve: gcode approve {}\n", approval.id));
        out.push_str(&format!("  deny: gcode deny {}\n", approval.id));
    }
    out.push('\n');
}

fn push_inspector(out: &mut String, state: &WorkspaceState) {
    out.push_str("Inspector\n");
    let budget_status = budget_status(state);
    out.push_str(&format!(
        "- Budget: {}/{} ({:.1}%) {}\n",
        state.budget.used_tokens,
        state.budget.limit_tokens,
        state.budget.percent_used,
        budget_status
    ));
    if state.budget.warning {
        out.push_str("- Limits: review current usage before continuing\n");
    } else {
        out.push_str("- Limits: within configured budget\n");
    }
}

fn budget_status(state: &WorkspaceState) -> &'static str {
    if state.budget.stopped {
        "stopped"
    } else if state.budget.warning {
        "warning"
    } else {
        "ok"
    }
}
