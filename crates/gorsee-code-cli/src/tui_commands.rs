use std::{path::Path, path::PathBuf};

use anyhow::{anyhow, Result};

use crate::{
    args::{SessionIdArgs, SkillsCommand},
    commands,
    commands_extra::{capabilities, diff, files, hooks, tools},
    run_with_options, CliOptions,
};

pub(crate) fn handler(
    root: PathBuf,
    env_key: Option<String>,
) -> impl Fn(String) -> Result<String> + Send + Sync + 'static {
    move |line| run(&root, env_key.as_deref(), &line)
}

fn run(root: &Path, env_key: Option<&str>, line: &str) -> Result<String> {
    let (command, args) = parse(line)?;
    match command {
        "capabilities" => capabilities(root, env_key),
        "budget" | "checkpoint" | "limits" | "models" | "route" => run_cli(root, env_key, line),
        "diff" => diff(root),
        "doctor" => commands::doctor(root, env_key),
        "export" => commands::export(root, session_args(first_arg(&args))),
        "files" => files(root),
        "hooks" => hooks(),
        "replay" => commands::replay(root, session_args(first_arg(&args))),
        "sessions" => commands::sessions(root),
        "skills" => commands::skills(root, skills_args(&args)?, env_key),
        "tools" => tools(root),
        _ => Err(anyhow!("unknown command: /{command}")),
    }
}

fn run_cli(root: &Path, env_key: Option<&str>, line: &str) -> Result<String> {
    let mut args = vec!["gcode".to_string()];
    args.extend(line.split_whitespace().map(str::to_string));
    run_with_options(
        args,
        CliOptions {
            root: root.to_path_buf(),
            env_key: env_key.map(str::to_string),
        },
    )
}

fn parse(line: &str) -> Result<(&str, Vec<String>)> {
    let mut parts = line.split_whitespace();
    let command = parts.next().ok_or_else(|| anyhow!("missing command"))?;
    let args = parts.map(str::to_string).collect();
    Ok((command, args))
}

fn first_arg(args: &[String]) -> Option<String> {
    args.first().cloned()
}

fn session_args(session_id: Option<String>) -> SessionIdArgs {
    SessionIdArgs { session_id }
}

fn skills_args(args: &[String]) -> Result<SkillsCommand> {
    let Some(command) = args.first().map(String::as_str) else {
        return Ok(SkillsCommand::List);
    };
    match command {
        "list" => Ok(SkillsCommand::List),
        "show" => Ok(SkillsCommand::Show {
            id: required_arg(args, "skill id")?,
        }),
        "run" => {
            let id = required_arg(args, "skill id")?;
            Ok(SkillsCommand::Run {
                id,
                objective: args.iter().skip(2).cloned().collect(),
            })
        }
        other => Err(anyhow!("unknown skills command: {other}")),
    }
}

fn required_arg(args: &[String], name: &str) -> Result<String> {
    args.get(1)
        .cloned()
        .ok_or_else(|| anyhow!("missing {name}"))
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn run_supports_actionable_workspace_commands() {
        let temp = tempfile::tempdir().unwrap();

        let budget = run(temp.path(), None, "budget set --session 100k").unwrap();
        let route = run(temp.path(), None, "route refactor auth").unwrap();
        let checkpoint = run(temp.path(), None, "checkpoint").unwrap();

        assert!(budget.contains("budget: updated"));
        assert!(budget.contains("session_tokens=100000"));
        assert!(route.contains("route:"));
        assert!(route.contains("objective: refactor auth"));
        assert!(checkpoint.contains("checkpoint:"));
    }

    #[test]
    fn run_preserves_external_command_arguments() {
        let temp = tempfile::tempdir().unwrap();

        let limits = run(temp.path(), None, "limits watch --once").unwrap();
        let models = run(temp.path(), None, "models recommend --task frontend bugfix").unwrap();

        assert!(limits.contains("limits watch:"));
        assert!(models.contains("models recommend:"));
        assert!(models.contains("task=frontend bugfix"));
    }
}
