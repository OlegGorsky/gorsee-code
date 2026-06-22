use std::{net::SocketAddr, path::Path};

use anyhow::{anyhow, Context, Result};
use gorsee_code_gateway::{serve, GatewayState};
use gorsee_code_safety::Redactor;
use gorsee_code_session::{export_markdown, ApprovalDecision, SessionStore};
use gorsee_code_skills::{builtin_skills, find_skill};
use gorsee_code_tui::render_workspace;
use gorsee_code_ui_state::workspace_state;

use crate::commands_extra::{
    agents, capabilities, config_check, diff, execute, files, hooks, read_manifest,
    resolve_session_id, run_skill, session_ids, tools, usage,
};
use crate::{
    acp_commands, approval_commands,
    args::{
        AcpCommand, AuthCommand, BudgetCommand, Cli, Command, SessionIdArgs, SessionsCommand,
        SkillsCommand,
    },
    auth, budget_commands, checkpoint_commands, limit_commands, live, model_commands, mouse_debug,
    paths, project_commands, protection_commands, route_commands, session_commands,
    uninstall_commands, CliOptions,
};

pub fn run(cli: Cli, options: CliOptions) -> Result<String> {
    match cli.command {
        None => render_workspace_tui(&options.root),
        Some(Command::Init) => project_commands::init(&options.root),
        Some(Command::Setup) => project_commands::setup(
            &options.root,
            options.env_key.as_deref(),
            options.global_auth_path.as_deref(),
        ),
        Some(Command::Auth(args)) => run_auth(
            &options.root,
            args.command,
            options.env_key.as_deref(),
            options.global_auth_path.as_deref(),
        ),
        Some(Command::Doctor) => doctor(
            &options.root,
            options.env_key.as_deref(),
            options.global_auth_path.as_deref(),
        ),
        Some(Command::Models(args)) => model_commands::run(
            &options.root,
            options.env_key.as_deref(),
            options.global_auth_path.as_deref(),
            args,
        ),
        Some(Command::Limits(args)) => limit_commands::run(
            &options.root,
            options.env_key.as_deref(),
            options.global_auth_path.as_deref(),
            args,
        ),
        Some(Command::Sessions(args)) => match args.command.unwrap_or(SessionsCommand::List) {
            SessionsCommand::List => sessions(&options.root),
        },
        Some(Command::Pause(args)) => session_commands::pause(&options.root, args),
        Some(Command::Resume(args)) => session_commands::resume(&options.root, args),
        Some(Command::Approvals) => approval_commands::list(&options.root),
        Some(Command::Approve { approval_id }) => approval_commands::decide(
            &options.root,
            &approval_id,
            ApprovalDecision::Approved,
            options.env_key.as_deref(),
            options.global_auth_path.as_deref(),
        ),
        Some(Command::Deny { approval_id }) => approval_commands::decide(
            &options.root,
            &approval_id,
            ApprovalDecision::Denied,
            options.env_key.as_deref(),
            options.global_auth_path.as_deref(),
        ),
        Some(Command::Replay(args)) => replay(&options.root, args),
        Some(Command::Export(args)) => export(&options.root, args),
        Some(Command::Gateway(args)) => gateway(&options.root, &args.bind),
        Some(Command::Acp(args)) => match args.command {
            Some(AcpCommand::Plan(args)) => acp_commands::plan(&options.root, args),
            Some(AcpCommand::Run(args)) => acp_commands::run(
                &options.root,
                args,
                options.env_key.as_deref(),
                options.global_auth_path.as_deref(),
            ),
            Some(AcpCommand::Stdio) => acp_commands::stdio(
                &options.root,
                options.env_key.as_deref(),
                options.global_auth_path.as_deref(),
            ),
            None => acp_commands::status(),
        },
        Some(Command::MouseDebug) => {
            mouse_debug::run()?;
            Ok(String::new())
        }
        Some(Command::Tui) => render_workspace_tui(&options.root),
        Some(Command::Skills(args)) => skills(
            &options.root,
            args.command,
            options.env_key.as_deref(),
            options.global_auth_path.as_deref(),
        ),
        Some(Command::Agents) => agents(),
        Some(Command::Usage) => usage(&options.root),
        Some(Command::Tools) => tools(&options.root),
        Some(Command::Mcp) => crate::tui_commands::mcp(&options.root),
        Some(Command::Files) => files(&options.root),
        Some(Command::Diff) => diff(&options.root),
        Some(Command::Route(args)) => route_commands::explain(&options.root, args),
        Some(Command::Budget(args)) => match args.command {
            BudgetCommand::Set(args) => budget_commands::set(&options.root, args),
        },
        Some(Command::Protect(args)) => protection_commands::protect(&options.root, args),
        Some(Command::Checkpoint) => checkpoint_commands::save(&options.root),
        Some(Command::Uninstall(args)) => uninstall_commands::run(&options.root, args),
        Some(Command::Hooks) => hooks(),
        Some(Command::Capabilities) => capabilities(
            &options.root,
            options.env_key.as_deref(),
            options.global_auth_path.as_deref(),
        ),
        Some(Command::Reset { yes }) => project_commands::reset(&options.root, yes),
        Some(Command::Exec(args)) => execute(
            &options.root,
            args,
            options.env_key.as_deref(),
            options.global_auth_path.as_deref(),
        ),
    }
}

fn run_auth(
    root: &Path,
    command: AuthCommand,
    env_key: Option<&str>,
    global_auth_path: Option<&Path>,
) -> Result<String> {
    match command {
        AuthCommand::Set { api_key } => {
            let api_key = resolve_auth_set_key(api_key, env_key)?;
            let status = auth::set(root, &api_key)?;
            auth::set_global_at(global_auth_path, &api_key)?;
            Ok(auth::render_status(&status))
        }
        AuthCommand::Status => Ok(auth::render_status(&auth::status_at(
            root,
            env_key,
            global_auth_path,
        )?)),
    }
}

fn resolve_auth_set_key(api_key: Option<String>, env_key: Option<&str>) -> Result<String> {
    api_key
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
        .or_else(|| {
            env_key
                .map(str::trim)
                .filter(|key| !key.is_empty())
                .map(str::to_string)
        })
        .ok_or_else(|| anyhow!("auth set needs an API key argument or NEUROGATE_API_KEY"))
}

pub(crate) fn doctor(
    root: &Path,
    env_key: Option<&str>,
    global_auth_path: Option<&Path>,
) -> Result<String> {
    let mut out = String::new();
    out.push_str(&config_check(root));
    out.push_str(&mouse_debug::doctor_report());
    out.push_str(&auth::render_status(&auth::status_at(
        root,
        env_key,
        global_auth_path,
    )?));
    match live::client(root, env_key, global_auth_path)? {
        Some(client) => live::block_on(async move {
            client.list_models().await.context("check /v1/models")?;
            client.account_limits().await.context("check /v1/me")?;
            Ok("neurogate: ok endpoints=/v1/models,/v1/me\n".to_string())
        })
        .map(|line| {
            out.push_str(&line);
            out
        }),
        None => {
            out.push_str("neurogate: skipped reason=missing_auth\n");
            Ok(out)
        }
    }
}

pub(crate) fn sessions(root: &Path) -> Result<String> {
    paths::ensure_layout(root)?;
    let mut ids = session_ids(root)?;
    ids.sort();
    if ids.is_empty() {
        return Ok("sessions: none\n".into());
    }
    let mut out = "sessions:\n".to_string();
    for id in ids {
        out.push_str(&format!("- {id}\n"));
    }
    Ok(out)
}

pub(crate) fn replay(root: &Path, args: SessionIdArgs) -> Result<String> {
    let id = resolve_session_id(root, args.session_id)?;
    let store = SessionStore::new(paths::local_dir(root), Redactor::default());
    let mut out = format!("replay: {id}\n");
    for event in store.read_events(&id)? {
        out.push_str(&format!(
            "- #{:04} {:?} {}\n",
            event.sequence, event.kind, event.payload
        ));
    }
    Ok(out)
}

pub(crate) fn export(root: &Path, args: SessionIdArgs) -> Result<String> {
    let id = resolve_session_id(root, args.session_id)?;
    let manifest = read_manifest(root, &id)?;
    let store = SessionStore::new(paths::local_dir(root), Redactor::default());
    let events = store.read_events(&id)?;
    Ok(export_markdown(&manifest, &events, &Redactor::default()))
}

fn gateway(root: &Path, bind: &str) -> Result<String> {
    let addr: SocketAddr = bind.parse().context("parse gateway bind address")?;
    eprintln!("gateway: listening on http://{addr}");
    let state = GatewayState::workspace(root);
    live::block_on(async move {
        serve(addr, state).await.context("serve gateway")?;
        Ok(format!("gateway: stopped {addr}\n"))
    })
}

fn render_workspace_tui(root: &Path) -> Result<String> {
    Ok(render_workspace(&workspace_state(root)))
}

pub(crate) fn skills(
    root: &Path,
    command: SkillsCommand,
    env_key: Option<&str>,
    global_auth_path: Option<&Path>,
) -> Result<String> {
    match command {
        SkillsCommand::List => {
            let mut out = "skills:\n".to_string();
            for skill in builtin_skills() {
                out.push_str(&format!("- {}: {}\n", skill.id, skill.description));
            }
            Ok(out)
        }
        SkillsCommand::Show { id } => {
            let skill = find_skill(&id).ok_or_else(|| anyhow!("unknown skill: {id}"))?;
            Ok(serde_json::to_string_pretty(&skill)?)
        }
        SkillsCommand::Run { id, objective } => {
            run_skill(root, &id, objective, env_key, global_auth_path)
        }
    }
}
