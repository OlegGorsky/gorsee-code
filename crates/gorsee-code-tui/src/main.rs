use clap::Parser;
use gorsee_code_tui::render_mission_control;
use gorsee_code_ui_state::fixture_state;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "mission-running")]
    fixture: String,
    #[arg(long)]
    json: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let state = fixture_state(&args.fixture);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&state)?);
    } else {
        println!("{}", render_mission_control(&state));
    }
    Ok(())
}
