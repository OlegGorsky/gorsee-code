use std::path::Path;

use anyhow::Result;
use gorsee_code_agent::TaskRunSummary;
use gorsee_code_session::ApprovalDecision;
use gorsee_code_tui::{run_app, TuiHandlers};
use gorsee_code_ui_state::{workspace_state, workspace_state_for_session};

use crate::{
    approval_commands, args::SessionIdArgs, commands_extra::run_interactive_task, session_commands,
    CliOptions,
};

pub fn run(options: &CliOptions) -> Result<()> {
    let root = options.root.clone();
    run_app(
        root,
        move |root, session_id| match session_id {
            Some(id) => workspace_state_for_session(root, Some(id)),
            None => workspace_state(root),
        },
        handlers(options.env_key.clone()),
    )
}

fn handlers(env_key: Option<String>) -> TuiHandlers {
    TuiHandlers::new(
        submit_handler(env_key.clone()),
        approve_handler(env_key.clone()),
        deny_handler(env_key.clone()),
        pause_handler(),
        resume_handler(),
        crate::tui_commands::handler(env_key),
    )
}

fn submit_handler(
    env_key: Option<String>,
) -> impl Fn(&Path, String) -> Result<String> + Send + Sync + 'static {
    move |root, objective| {
        let summary = run_interactive_task(root, objective, env_key.as_deref())?;
        Ok(format_summary(&summary))
    }
}

fn approve_handler(
    env_key: Option<String>,
) -> impl Fn(&Path, String) -> Result<String> + Send + Sync + 'static {
    move |root, id| {
        approval_commands::decide(root, &id, ApprovalDecision::Approved, env_key.as_deref())
    }
}

fn deny_handler(
    env_key: Option<String>,
) -> impl Fn(&Path, String) -> Result<String> + Send + Sync + 'static {
    move |root, id| {
        approval_commands::decide(root, &id, ApprovalDecision::Denied, env_key.as_deref())
    }
}

fn pause_handler() -> impl Fn(&Path, String) -> Result<String> + Send + Sync + 'static {
    move |root, id| {
        session_commands::pause(
            root,
            SessionIdArgs {
                session_id: Some(id),
            },
        )
    }
}

fn resume_handler() -> impl Fn(&Path, String) -> Result<String> + Send + Sync + 'static {
    move |root, id| {
        session_commands::resume(
            root,
            SessionIdArgs {
                session_id: Some(id),
            },
        )
    }
}

fn format_summary(summary: &TaskRunSummary) -> String {
    format!(
        "run: completed session={}\nevents={}\nagents={}\nartifacts={}",
        summary.session_id,
        summary.events,
        summary.agents.join(","),
        summary.artifacts.len()
    )
}
