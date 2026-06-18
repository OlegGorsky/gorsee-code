use gorsee_code_ui_state::MissionControlState;

pub fn render_mission_control(state: &MissionControlState) -> String {
    let mut out = String::new();
    push_header(&mut out, state);
    push_agents(&mut out, state);
    push_timeline(&mut out, state);
    push_inspector(&mut out, state);
    out
}

fn push_header(out: &mut String, state: &MissionControlState) {
    out.push_str(&format!("Gorsee Code Mission: {}\n", state.session.title));
    out.push_str(&format!(
        "session={} status={} gateway={}\n\n",
        state.session.id, state.session.status, state.gateway_status
    ));
}

fn push_agents(out: &mut String, state: &MissionControlState) {
    out.push_str("Agents\n");
    for agent in &state.agents {
        out.push_str(&format!(
            "- {} {} {} {}/{}\n",
            agent.id, agent.status, agent.model, agent.tokens_used, agent.tokens_limit
        ));
    }
    out.push('\n');
}

fn push_timeline(out: &mut String, state: &MissionControlState) {
    out.push_str("Timeline\n");
    for event in &state.timeline {
        let agent = event.agent_id.as_deref().unwrap_or("mission");
        out.push_str(&format!(
            "- #{:04} {} {}: {}\n",
            event.sequence, event.kind, agent, event.summary
        ));
    }
    out.push('\n');
}

fn push_inspector(out: &mut String, state: &MissionControlState) {
    out.push_str("Inspector\n");
    out.push_str(&format!(
        "- budget: {}/{} ({:.1}%)\n",
        state.budget.used_tokens, state.budget.limit_tokens, state.budget.percent_used
    ));
    for approval in &state.approvals {
        out.push_str(&format!(
            "- approval: {} {} {}\n",
            approval.id, approval.name, approval.risk
        ));
    }
}
