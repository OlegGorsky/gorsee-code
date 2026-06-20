use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};

use crate::{
    args::{SessionIdArgs, SkillsCommand},
    commands,
    commands_extra::{capabilities, diff, files, hooks, tools},
    run_with_options, CliOptions,
};

pub(crate) fn handler(
    env_key: Option<String>,
) -> impl Fn(&Path, String) -> Result<String> + Send + Sync + 'static {
    move |root, line| run(root, env_key.as_deref(), &line)
}

fn run(root: &Path, env_key: Option<&str>, line: &str) -> Result<String> {
    let (command, args) = parse(line)?;
    match command.as_str() {
        "capabilities" => capabilities(root, env_key),
        "budget" | "checkpoint" | "limits" | "models" | "route" => run_cli(root, env_key, line),
        "diff" => diff(root),
        "doctor" => commands::doctor(root, env_key),
        "export" => commands::export(root, session_args(first_arg(&args))),
        "files" => files(root),
        "hooks" => hooks(),
        "instructions" => instructions(root),
        "mcp" => mcp(root),
        "replay" => commands::replay(root, session_args(first_arg(&args))),
        "sessions" => commands::sessions(root),
        "skills" => commands::skills(root, skills_args(&args)?, env_key),
        "terminal" => terminal(root, &args),
        "tools" => tools(root),
        _ => Err(anyhow!("unknown command: /{command}")),
    }
}

fn instructions(root: &Path) -> Result<String> {
    let files = ["AGENTS.md", "GORSEE.md", "README.md"];
    let mut out = "instructions:\n".to_string();
    let mut found = false;
    for file in files {
        let path = root.join(file);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        found = true;
        out.push_str(&format!("\n## {file}\n"));
        for line in text.lines().take(40) {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !found {
        out.push_str("none\n");
    }
    Ok(out)
}

fn mcp(root: &Path) -> Result<String> {
    let tools = tools(root)?;
    Ok(format!("mcp:\nsource=tool-runtime\n{tools}"))
}

fn terminal(root: &Path, args: &[String]) -> Result<String> {
    let line = args.join(" ");
    if line.trim().is_empty() {
        return Ok("terminal: missing command\n".into());
    }
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".into());
    let mut child = Command::new(shell)
        .arg("-lc")
        .arg(&line)
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while child.try_wait()?.is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output()?;
            return Ok(render_terminal_output(&line, output, true));
        }
        thread::sleep(Duration::from_millis(50));
    }
    let output = child.wait_with_output()?;
    Ok(render_terminal_output(&line, output, false))
}

fn render_terminal_output(line: &str, output: std::process::Output, timed_out: bool) -> String {
    let mut rendered = format!("$ {line}\n");
    rendered.push_str(&String::from_utf8_lossy(&output.stdout));
    rendered.push_str(&String::from_utf8_lossy(&output.stderr));
    if timed_out {
        rendered.push_str("timeout: command stopped after 30s\n");
    }
    if !output.status.success() {
        rendered.push_str(&format!("exit: {}\n", output.status));
    }
    rendered
}

fn run_cli(root: &Path, env_key: Option<&str>, line: &str) -> Result<String> {
    let mut args = vec!["gcode".to_string()];
    args.extend(split_args(line)?);
    run_with_options(
        args,
        CliOptions {
            root: root.to_path_buf(),
            env_key: env_key.map(str::to_string),
        },
    )
}

fn parse(line: &str) -> Result<(String, Vec<String>)> {
    let parts = split_args(line)?;
    let mut parts = parts.iter();
    let command = parts.next().ok_or_else(|| anyhow!("missing command"))?;
    let args = parts.map(ToString::to_string).collect();
    Ok((command.to_string(), args))
}

fn split_args(line: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    for ch in line.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '\'' | '"' if quote == Some(ch) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(ch),
            ch if ch.is_whitespace() && quote.is_none() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            ch => current.push(ch),
        }
    }
    if escaped {
        current.push('\\');
    }
    if let Some(ch) = quote {
        return Err(anyhow!("unterminated quote: {ch}"));
    }
    if !current.is_empty() {
        args.push(current);
    }
    Ok(args)
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

    #[test]
    fn run_preserves_quoted_external_command_arguments() {
        let temp = tempfile::tempdir().unwrap();

        let models = run(
            temp.path(),
            None,
            "models recommend --task \"frontend bugfix\"",
        )
        .unwrap();
        let terminal = run(temp.path(), None, "terminal printf \"hello world\"").unwrap();

        assert!(models.contains("task=frontend bugfix"));
        assert!(terminal.contains("hello world"));
    }

    #[test]
    fn run_supports_terminal_command_output() {
        let temp = tempfile::tempdir().unwrap();

        let output = run(temp.path(), None, "terminal printf ok").unwrap();

        assert!(output.contains("$ printf ok"));
        assert!(output.contains("ok"));
    }

    #[test]
    fn run_supports_instruction_and_mcp_sections() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("AGENTS.md"), "# Rules\nUse tests.\n").unwrap();

        let instructions = run(temp.path(), None, "instructions").unwrap();
        let mcp = run(temp.path(), None, "mcp").unwrap();

        assert!(instructions.contains("instructions:"));
        assert!(instructions.contains("AGENTS.md"));
        assert!(instructions.contains("Use tests."));
        assert!(mcp.contains("mcp:"));
        assert!(mcp.contains("tool-runtime"));
    }
}
