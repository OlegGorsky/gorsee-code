pub mod auth;

mod approval_commands;
mod args;
mod budget_commands;
mod checkpoint_commands;
mod commands;
mod commands_extra;
mod config_file;
mod interactive;
mod limit_commands;
mod live;
mod model_commands;
mod mouse_debug;
mod paths;
mod project_commands;
mod protection_commands;
mod route_commands;
pub mod secret_prompt;
mod session_commands;
mod tui_commands;
mod uninstall_commands;

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{error::ErrorKind, Parser};

pub use args::Cli;

#[derive(Debug, Clone)]
pub struct CliOptions {
    pub root: PathBuf,
    pub env_key: Option<String>,
}

impl CliOptions {
    pub fn for_root(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            env_key: None,
        }
    }

    pub fn current() -> Result<Self> {
        Ok(Self {
            root: std::env::current_dir()?,
            env_key: std::env::var("NEUROGATE_API_KEY")
                .ok()
                .or_else(|| std::env::var("GORSEE_NEUROGATE_API_KEY").ok()),
        })
    }
}

pub fn run_with_options<I, S>(args: I, options: CliOptions) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            return Ok(error.to_string());
        }
        Err(error) => return Err(error.into()),
    };
    commands::run(cli, options)
}

pub fn run_interactive_tui(options: &CliOptions) -> Result<()> {
    interactive::run(options)
}
