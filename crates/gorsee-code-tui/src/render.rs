use gorsee_code_ui_state::WorkspaceState;

use crate::WorkspaceApp;

pub fn render_app(state: &WorkspaceState, app: &WorkspaceApp) -> String {
    let mut out = render_workspace(state);
    out.push('\n');
    out.push_str("Введите задачу для Gorsee Code\n");
    out.push_str("> ");
    out.push_str(app.input());
    out.push('\n');
    out.push_str(
        "Enter запуск | /help команды | a подтвердить | d отклонить | p пауза | r продолжить | q выход\n",
    );
    if let Some(status) = app.status() {
        out.push_str(&format!("Статус: {status}\n"));
    }
    if let Some(output) = app.output() {
        out.push('\n');
        out.push_str("Результат команды\n");
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
        "Сессия {} | Статус {} | Repo {} | Branch {}\n",
        state.session.id, state.session.status, state.session.repo, state.session.branch
    ));
    out.push_str(&format!(
        "Безопасность workspace approvals | Gateway {}\n\n",
        state.gateway_status
    ));
}

fn push_agents(out: &mut String, state: &WorkspaceState) {
    out.push_str("Агенты\n");
    for agent in &state.agents {
        out.push_str(&format!(
            "- {} role={} status={} model={} tokens={}/{} cached={}\n",
            agent.id,
            agent.role,
            agent.status,
            agent.model,
            agent.tokens_used,
            agent.tokens_limit,
            agent.cached_tokens
        ));
    }
    out.push('\n');
}

fn push_timeline(out: &mut String, state: &WorkspaceState) {
    out.push_str("Лента\n");
    for event in &state.timeline {
        let agent = event.agent_id.as_deref().unwrap_or(&event.kind);
        out.push_str(&format!("- {}: {}\n", agent, event.summary));
    }
    out.push('\n');
}

fn push_approvals(out: &mut String, state: &WorkspaceState) {
    out.push_str("Подтверждения\n");
    if state.approvals.is_empty() {
        out.push_str("- нет\n\n");
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
    out.push_str("Инспектор\n");
    let budget_status = budget_status(state);
    out.push_str(&format!(
        "- Лимиты: {}/{} токенов ({:.1}%) cache={} {}\n",
        state.budget.used_tokens,
        state.budget.limit_tokens,
        state.budget.percent_used,
        state.budget.cached_tokens,
        budget_status
    ));
    if state.budget.warning {
        out.push_str("- Лимиты: проверь текущее использование перед продолжением\n");
    } else {
        out.push_str("- Лимиты: в рамках настроенного бюджета\n");
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
