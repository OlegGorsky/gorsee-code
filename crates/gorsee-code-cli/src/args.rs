use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "gcode",
    version,
    about = "NeuroGate-native coding agent command center"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Init,
    Setup,
    Auth(AuthArgs),
    Doctor,
    Models,
    Limits,
    Sessions(SessionsArgs),
    Pause(SessionIdArgs),
    Resume(SessionIdArgs),
    Replay(SessionIdArgs),
    Export(SessionIdArgs),
    Gateway(GatewayArgs),
    Tui(TuiArgs),
    Skills(SkillsArgs),
    Agents,
    Usage,
    Tools,
    Hooks,
    Capabilities,
    Exec(ObjectiveArgs),
    Mission(ObjectiveArgs),
}

#[derive(Debug, Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommand,
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    Set { api_key: Option<String> },
    Status,
}

#[derive(Debug, Args)]
pub struct SessionsArgs {
    #[command(subcommand)]
    pub command: Option<SessionsCommand>,
}

#[derive(Debug, Subcommand)]
pub enum SessionsCommand {
    List,
}

#[derive(Debug, Args)]
pub struct SessionIdArgs {
    pub session_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct GatewayArgs {
    #[arg(long, default_value = "127.0.0.1:8765")]
    pub bind: String,
}

#[derive(Debug, Args)]
pub struct TuiArgs {
    #[arg(long, default_value = "mission-running")]
    pub fixture: String,
}

#[derive(Debug, Args)]
pub struct SkillsArgs {
    #[command(subcommand)]
    pub command: SkillsCommand,
}

#[derive(Debug, Subcommand)]
pub enum SkillsCommand {
    List,
    Show {
        id: String,
    },
    Run {
        id: String,
        #[arg(trailing_var_arg = true)]
        objective: Vec<String>,
    },
}

#[derive(Debug, Args)]
pub struct ObjectiveArgs {
    #[arg(required = true, trailing_var_arg = true)]
    pub objective: Vec<String>,
}
