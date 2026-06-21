use clap::{Args, Parser, Subcommand, ValueEnum};

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
    Models(ModelsArgs),
    Limits(LimitsArgs),
    Sessions(SessionsArgs),
    Pause(SessionIdArgs),
    Resume(SessionIdArgs),
    Approvals,
    Approve {
        approval_id: String,
    },
    Deny {
        approval_id: String,
    },
    Replay(SessionIdArgs),
    Export(SessionIdArgs),
    Gateway(GatewayArgs),
    MouseDebug,
    Tui,
    Skills(SkillsArgs),
    Agents,
    Usage,
    Tools,
    Files,
    Diff,
    Route(ObjectiveArgs),
    Budget(BudgetArgs),
    Protect(ProtectArgs),
    Checkpoint,
    Uninstall(UninstallArgs),
    Hooks,
    Capabilities,
    Reset {
        #[arg(long)]
        yes: bool,
    },
    Exec(ObjectiveArgs),
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
pub struct ModelsArgs {
    #[command(subcommand)]
    pub command: Option<ModelsCommand>,
}

#[derive(Debug, Subcommand)]
pub enum ModelsCommand {
    Benchmark,
    Recommend(ModelsRecommendArgs),
    Set(ModelsSetArgs),
}

#[derive(Debug, Args)]
pub struct ModelsRecommendArgs {
    #[arg(long, num_args = 1..)]
    pub task: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ModelsSetArgs {
    #[arg(long)]
    pub agent: String,
    #[arg(long)]
    pub model: String,
}

#[derive(Debug, Args)]
pub struct LimitsArgs {
    #[arg(long)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Option<LimitsCommand>,
}

#[derive(Debug, Subcommand)]
pub enum LimitsCommand {
    Watch(LimitsWatchArgs),
}

#[derive(Debug, Args)]
pub struct LimitsWatchArgs {
    #[arg(long)]
    pub once: bool,
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

#[derive(Debug, Args)]
pub struct BudgetArgs {
    #[command(subcommand)]
    pub command: BudgetCommand,
}

#[derive(Debug, Subcommand)]
pub enum BudgetCommand {
    Set(BudgetSetArgs),
}

#[derive(Debug, Args)]
pub struct BudgetSetArgs {
    #[arg(long)]
    pub session: Option<String>,
    #[arg(long, num_args = 2, value_names = ["AGENT", "TOKENS"])]
    pub agent: Option<Vec<String>>,
}

#[derive(Debug, Args)]
pub struct ProtectArgs {
    #[arg(required = true)]
    pub paths: Vec<String>,
}

#[derive(Debug, Args)]
pub struct UninstallArgs {
    #[arg(long, value_enum)]
    pub user_data: UserDataMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum UserDataMode {
    Keep,
    Remove,
}
