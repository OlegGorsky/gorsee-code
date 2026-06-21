use std::path::Path;

use anyhow::Result;
use gorsee_code_agent::TaskRunSummary;
use gorsee_code_session::ApprovalDecision;
use gorsee_code_tui::{run_app, TuiHandlers};
use gorsee_code_ui_state::{workspace_state, workspace_state_for_session};

use crate::{
    approval_commands, args::SessionIdArgs, auth, commands_extra::run_interactive_task,
    session_commands, CliOptions,
};

pub fn run(options: &CliOptions) -> Result<()> {
    let root = options.root.clone();
    let env_key = tui_env_key(options)?;
    run_app(
        root,
        move |root, session_id| match session_id {
            Some(id) => workspace_state_for_session(root, Some(id)),
            None => workspace_state(root),
        },
        handlers(env_key),
    )
}

fn tui_env_key(options: &CliOptions) -> Result<Option<String>> {
    match options.env_key.clone() {
        Some(key) if !key.trim().is_empty() => Ok(Some(key)),
        _ => auth::api_key(&options.root, None),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tui_env_key_reuses_key_from_launch_root() {
        let temp = tempfile::tempdir().unwrap();
        auth::set(temp.path(), "ng_sk_launch_root_123456").unwrap();

        let options = CliOptions::for_root(temp.path());

        assert_eq!(
            tui_env_key(&options).unwrap().as_deref(),
            Some("ng_sk_launch_root_123456")
        );
    }

    #[test]
    fn tui_env_key_prefers_process_env_key() {
        let temp = tempfile::tempdir().unwrap();
        auth::set(temp.path(), "ng_sk_launch_root_123456").unwrap();
        let mut options = CliOptions::for_root(temp.path());
        options.env_key = Some("ng_sk_env_123456".into());

        assert_eq!(
            tui_env_key(&options).unwrap().as_deref(),
            Some("ng_sk_env_123456")
        );
    }
}
