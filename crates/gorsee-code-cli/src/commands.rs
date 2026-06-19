use std::{fs, net::SocketAddr, path::Path};

use anyhow::{anyhow, Context, Result};
use gorsee_code_config::default_config_toml;
use gorsee_code_core::{Event, EventKind};
use gorsee_code_gateway::{serve, GatewayState};
use gorsee_code_safety::Redactor;
use gorsee_code_session::{export_markdown, SessionStore};
use gorsee_code_skills::{builtin_skills, find_skill};
use gorsee_code_tui::render_mission_control;
use gorsee_code_ui_state::workspace_state;
use serde_json::json;

use crate::commands_extra::{
    agents, capabilities, config_check, hooks, load_or_default, mission, read_manifest,
    resolve_session_id, session_ids, skill_mission, tools, usage,
};
use crate::{
    args::{AuthCommand, Cli, Command, SessionIdArgs, SessionsCommand, SkillsCommand},
    auth, live, paths, CliOptions,
};

pub fn run(cli: Cli, options: CliOptions) -> Result<String> {
    match cli.command {
        None => render_workspace_tui(&options.root),
        Some(Command::Init) => init(&options.root),
        Some(Command::Setup) => setup(&options.root),
        Some(Command::Auth(args)) => {
            run_auth(&options.root, args.command, options.env_key.as_deref())
        }
        Some(Command::Doctor) => doctor(&options.root, options.env_key.as_deref()),
        Some(Command::Models) => models(&options.root, options.env_key.as_deref()),
        Some(Command::Limits) => limits(&options.root, options.env_key.as_deref()),
        Some(Command::Sessions(args)) => match args.command.unwrap_or(SessionsCommand::List) {
            SessionsCommand::List => sessions(&options.root),
        },
        Some(Command::Pause(args)) => pause(&options.root, args),
        Some(Command::Resume(args)) => session_summary(&options.root, args, "resume"),
        Some(Command::Replay(args)) => replay(&options.root, args),
        Some(Command::Export(args)) => export(&options.root, args),
        Some(Command::Gateway(args)) => gateway(&options.root, &args.bind),
        Some(Command::Tui) => render_workspace_tui(&options.root),
        Some(Command::Skills(args)) => {
            skills(&options.root, args.command, options.env_key.as_deref())
        }
        Some(Command::Agents) => agents(),
        Some(Command::Usage) => usage(&options.root),
        Some(Command::Tools) => tools(&options.root),
        Some(Command::Hooks) => hooks(),
        Some(Command::Capabilities) => capabilities(&options.root, options.env_key.as_deref()),
        Some(Command::Exec(args)) | Some(Command::Mission(args)) => {
            mission(&options.root, args, options.env_key.as_deref())
        }
    }
}

fn init(root: &Path) -> Result<String> {
    paths::ensure_layout(root)?;
    let config_path = paths::config_path(root);
    if !config_path.exists() {
        let text =
            default_config_toml(paths::project_name(root)).context("render default config")?;
        fs::write(&config_path, text).context("write gorsee-code.toml")?;
    }
    Ok(format!(
        "initialized: {}\nstate: {}\n",
        config_path.display(),
        paths::local_dir(root).display()
    ))
}

fn setup(root: &Path) -> Result<String> {
    let mut out = init(root)?;
    out.push_str("next: export NEUROGATE_API_KEY=... && gcode auth set\nnext: gcode doctor\n");
    Ok(out)
}

fn run_auth(root: &Path, command: AuthCommand, env_key: Option<&str>) -> Result<String> {
    match command {
        AuthCommand::Set { api_key } => {
            let api_key = resolve_auth_set_key(api_key, env_key)?;
            let status = auth::set(root, &api_key)?;
            Ok(auth::render_status(&status))
        }
        AuthCommand::Status => Ok(auth::render_status(&auth::status(root, env_key)?)),
    }
}

fn resolve_auth_set_key(api_key: Option<String>, env_key: Option<&str>) -> Result<String> {
    api_key
        .filter(|key| !key.trim().is_empty())
        .or_else(|| {
            env_key
                .map(str::trim)
                .filter(|key| !key.is_empty())
                .map(str::to_string)
        })
        .ok_or_else(|| anyhow!("auth set needs an API key argument or NEUROGATE_API_KEY"))
}

fn doctor(root: &Path, env_key: Option<&str>) -> Result<String> {
    let mut out = String::new();
    out.push_str(&config_check(root));
    out.push_str(&auth::render_status(&auth::status(root, env_key)?));
    match live::client(root, env_key)? {
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

fn models(root: &Path, env_key: Option<&str>) -> Result<String> {
    if let Some(client) = live::client(root, env_key)? {
        return live::block_on(async move {
            let models = client.list_models().await?;
            let mut out = "models: live\n".to_string();
            for model in models {
                out.push_str(&format!(
                    "- {} cost={} streaming={} tools={}\n",
                    model.id,
                    model.relative_cost_label(),
                    model.supports_streaming,
                    model.supports_tools
                ));
            }
            Ok(out)
        });
    }
    let config = load_or_default(root);
    let mut out = "models: configured matrix (auth missing, live check skipped)\n".to_string();
    for (agent, profile) in config.agents {
        out.push_str(&format!("- {agent}: {}\n", profile.model));
    }
    Ok(out)
}

fn limits(root: &Path, env_key: Option<&str>) -> Result<String> {
    let Some(client) = live::client(root, env_key)? else {
        return Ok("limits: skipped reason=missing_auth\n".into());
    };
    live::block_on(async move {
        let windows = client.account_limits().await?;
        let mut out = "limits: live\n".to_string();
        for window in windows {
            out.push_str(&format!(
                "- {} credits={:.1}/{:.1} requests={}/{}\n",
                window.label,
                window.credits_used,
                window.credit_limit,
                window.requests_used,
                window.request_limit
            ));
        }
        Ok(out)
    })
}

fn sessions(root: &Path) -> Result<String> {
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

fn session_summary(root: &Path, args: SessionIdArgs, label: &str) -> Result<String> {
    let id = resolve_session_id(root, args.session_id)?;
    let store = SessionStore::new(paths::local_dir(root), Redactor::default());
    let events = store.read_events(&id)?;
    Ok(format!("{label}: {id}\nevents: {}\n", events.len()))
}

fn pause(root: &Path, args: SessionIdArgs) -> Result<String> {
    let id = resolve_session_id(root, args.session_id)?;
    let store = SessionStore::new(paths::local_dir(root), Redactor::default());
    let events = store.read_events(&id)?;
    let mut manifest = read_manifest(root, &id)?;
    let event = Event::new(
        events.len() as u64 + 1,
        &id,
        None,
        EventKind::MissionPaused,
        json!({ "message": "mission paused" }),
    );
    store.append_event(&event)?;
    manifest.status = "paused".into();
    store.write_manifest(&manifest)?;
    Ok(format!("pause: {id}\nevent: mission_paused\n"))
}

fn replay(root: &Path, args: SessionIdArgs) -> Result<String> {
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

fn export(root: &Path, args: SessionIdArgs) -> Result<String> {
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
    Ok(render_mission_control(&workspace_state(root)))
}

fn skills(root: &Path, command: SkillsCommand, env_key: Option<&str>) -> Result<String> {
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
        SkillsCommand::Run { id, objective } => skill_mission(root, &id, objective, env_key),
    }
}
