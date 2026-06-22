use std::path::Path;

use anyhow::Result;
use gorsee_code_agent::TaskTurnOutput;
use gorsee_code_session::ApprovalDecision;
use gorsee_code_tui::{run_app, TuiHandlers};
use gorsee_code_ui_state::{workspace_state, workspace_state_for_session};

use crate::{
    approval_commands,
    args::SessionIdArgs,
    auth,
    commands_extra::{format_task_output, run_interactive_task_response},
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
        _ => auth::api_key_at(&options.root, None, options.global_auth_path.as_deref()),
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
) -> impl Fn(&Path, Option<&str>, String) -> Result<String> + Send + Sync + 'static {
    move |root, session_id, objective| {
        let output =
            run_interactive_task_response(root, session_id, objective, env_key.as_deref(), None)?;
        Ok(format_summary(&output))
    }
}

fn approve_handler(
    env_key: Option<String>,
) -> impl Fn(&Path, String) -> Result<String> + Send + Sync + 'static {
    move |root, id| {
        approval_commands::decide(
            root,
            &id,
            ApprovalDecision::Approved,
            env_key.as_deref(),
            None,
        )
    }
}

fn deny_handler(
    env_key: Option<String>,
) -> impl Fn(&Path, String) -> Result<String> + Send + Sync + 'static {
    move |root, id| {
        approval_commands::decide(
            root,
            &id,
            ApprovalDecision::Denied,
            env_key.as_deref(),
            None,
        )
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

fn format_summary(output: &TaskTurnOutput) -> String {
    format_task_output(output).trim_end().to_string()
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
