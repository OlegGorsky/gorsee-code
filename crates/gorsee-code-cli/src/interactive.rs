use std::path::PathBuf;

use anyhow::Result;
use gorsee_code_agent::TaskRunSummary;
use gorsee_code_session::ApprovalDecision;
use gorsee_code_tui::{run_app, TuiHandlers};
use gorsee_code_ui_state::workspace_state;

use crate::{
    approval_commands, args::SessionIdArgs, commands_extra::run_task, session_commands, CliOptions,
};

pub fn run(options: &CliOptions) -> Result<()> {
    let root = options.root.clone();
    let load_root = root.clone();
    run_app(
        move || workspace_state(&load_root),
        handlers(root, options.env_key.clone()),
    )
}

fn handlers(root: PathBuf, env_key: Option<String>) -> TuiHandlers {
    TuiHandlers::new(
        submit_handler(root.clone(), env_key.clone()),
        approve_handler(root.clone(), env_key.clone()),
        deny_handler(root.clone(), env_key.clone()),
        pause_handler(root.clone()),
        resume_handler(root.clone()),
        crate::tui_commands::handler(root, env_key),
    )
}

fn submit_handler(
    root: PathBuf,
    env_key: Option<String>,
) -> impl Fn(String) -> Result<String> + Send + Sync + 'static {
    move |objective| {
        let summary = run_task(&root, objective, env_key.as_deref())?;
        Ok(format_summary(&summary))
    }
}

fn approve_handler(
    root: PathBuf,
    env_key: Option<String>,
) -> impl Fn(String) -> Result<String> + Send + Sync + 'static {
    move |id| approval_commands::decide(&root, &id, ApprovalDecision::Approved, env_key.as_deref())
}

fn deny_handler(
    root: PathBuf,
    env_key: Option<String>,
) -> impl Fn(String) -> Result<String> + Send + Sync + 'static {
    move |id| approval_commands::decide(&root, &id, ApprovalDecision::Denied, env_key.as_deref())
}

fn pause_handler(root: PathBuf) -> impl Fn(String) -> Result<String> + Send + Sync + 'static {
    move |id| {
        session_commands::pause(
            &root,
            SessionIdArgs {
                session_id: Some(id),
            },
        )
    }
}

fn resume_handler(root: PathBuf) -> impl Fn(String) -> Result<String> + Send + Sync + 'static {
    move |id| {
        session_commands::resume(
            &root,
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
