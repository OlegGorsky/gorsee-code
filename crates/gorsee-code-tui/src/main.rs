use clap::Parser;
use gorsee_code_tui::render_workspace;
use gorsee_code_ui_state::workspace_state;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    json: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let state = workspace_state(std::env::current_dir()?);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&state)?);
    } else {
        println!("{}", render_workspace(&state));
    }
    Ok(())
}
