use gorsee_code_ui_state::WorkspaceState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAction {
    Display(String),
    External(String),
    Approve(String),
    Deny(String),
    Pause(String),
    Resume(String),
    Quit,
}

pub fn parse_command(input: &str, state: &WorkspaceState) -> CommandAction {
    let trimmed = input.trim();
    if trimmed == "/" || trimmed == "/help" {
        return CommandAction::Display(help());
    }

    let line = trimmed.trim_start_matches('/').trim();
    let mut parts = line.split_whitespace();
    let command = parts.next().unwrap_or_default();
    let args = parts.collect::<Vec<_>>();

    match command {
        "agents" => CommandAction::Display(agents(state)),
        "approvals" => CommandAction::Display(approvals(state)),
        "budget" if args.is_empty() => CommandAction::Display(budget(state)),
        "budget" => CommandAction::External(external_line(command, &args)),
        "usage" => CommandAction::Display(budget(state)),
        "context" => CommandAction::Display(context(state)),
        "route" if args.is_empty() => CommandAction::Display(route(state)),
        "route" => CommandAction::External(external_line(command, &args)),
        "timeline" => CommandAction::Display(timeline(state)),
        "approve" => approval_command(state, args.first(), CommandAction::Approve),
        "deny" => approval_command(state, args.first(), CommandAction::Deny),
        "pause" => session_command(state, args.first(), CommandAction::Pause),
        "resume" => session_command(state, args.first(), CommandAction::Resume),
        "quit" | "exit" => CommandAction::Quit,
        "capabilities" | "checkpoint" | "diff" | "doctor" | "export" | "files" | "hooks"
        | "limits" | "models" | "replay" | "sessions" | "skills" | "tools" => {
            CommandAction::External(external_line(command, &args))
        }
        _ => CommandAction::Display(format!("unknown command: /{command}\n{}", help())),
    }
}

fn help() -> String {
    [
        "commands:",
        "- /agents show active coding agents",
        "- /budget show token usage",
        "- /doctor check local setup and NeuroGate access",
        "- /models show model routing",
        "- /limits show live account limits",
        "- /capabilities show available model capabilities",
        "- /skills list coding skills",
        "- /hooks show enabled safety hooks",
        "- /route show workspace routing",
        "- /timeline show recent events",
        "- /context show repo, branch, session and gateway",
        "- /approvals show pending approvals",
        "- /checkpoint save current session state",
        "- /approve [id] approve a pending tool call",
        "- /deny [id] deny a pending tool call",
        "- /pause [id] pause a running session",
        "- /resume [id] resume a paused session",
        "- /files show workspace files",
        "- /diff show git diff",
        "- /export [id] export a session",
        "- /replay [id] replay a session",
    ]
    .join("\n")
        + "\n"
}

fn agents(state: &WorkspaceState) -> String {
    let mut out = "agents:\n".to_string();
    for agent in &state.agents {
        out.push_str(&format!(
            "- {} role={} status={} model={} tokens={}/{}\n",
            agent.id, agent.role, agent.status, agent.model, agent.tokens_used, agent.tokens_limit
        ));
    }
    out
}

fn approvals(state: &WorkspaceState) -> String {
    if state.approvals.is_empty() {
        return "approvals: none\n".into();
    }
    let mut out = "approvals:\n".to_string();
    for approval in &state.approvals {
        out.push_str(&format!(
            "- {} tool={} status={} risk={}\n",
            approval.id, approval.name, approval.status, approval.risk
        ));
    }
    out
}

fn budget(state: &WorkspaceState) -> String {
    format!(
        "budget: {}/{} tokens ({:.1}%) status={}\n",
        state.budget.used_tokens,
        state.budget.limit_tokens,
        state.budget.percent_used,
        budget_status(state)
    )
}

fn context(state: &WorkspaceState) -> String {
    format!(
        "context:\n- session={} status={}\n- repo={}\n- branch={}\n- gateway={}\n",
        state.session.id,
        state.session.status,
        state.session.repo,
        state.session.branch,
        state.gateway_status
    )
}

fn route(state: &WorkspaceState) -> String {
    let mut out = "route:\n".to_string();
    for agent in &state.agents {
        out.push_str(&format!("- {} -> {}\n", agent.role, agent.model));
    }
    out
}

fn timeline(state: &WorkspaceState) -> String {
    let mut out = "timeline:\n".to_string();
    for event in &state.timeline {
        let agent = event.agent_id.as_deref().unwrap_or("workspace");
        out.push_str(&format!(
            "- #{:04} {} {}: {}\n",
            event.sequence, event.kind, agent, event.summary
        ));
    }
    out
}

fn approval_command(
    state: &WorkspaceState,
    requested: Option<&&str>,
    build: fn(String) -> CommandAction,
) -> CommandAction {
    match requested
        .map(|id| id.to_string())
        .or_else(|| latest_approval(state))
    {
        Some(id) => build(id),
        None => CommandAction::Display("approvals: none\n".into()),
    }
}

fn session_command(
    state: &WorkspaceState,
    requested: Option<&&str>,
    build: fn(String) -> CommandAction,
) -> CommandAction {
    match requested
        .map(|id| id.to_string())
        .or_else(|| active_session(state))
    {
        Some(id) => build(id),
        None => CommandAction::Display("session: none\n".into()),
    }
}

fn latest_approval(state: &WorkspaceState) -> Option<String> {
    state.approvals.first().map(|approval| approval.id.clone())
}

fn active_session(state: &WorkspaceState) -> Option<String> {
    let id = state.session.id.trim();
    (!id.is_empty() && id != "workspace").then(|| id.to_string())
}

fn external_line(command: &str, args: &[&str]) -> String {
    if args.is_empty() {
        command.to_string()
    } else {
        format!("{command} {}", args.join(" "))
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
