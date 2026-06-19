use std::io::{self, Write};

use anyhow::Result;
use gorsee_code_tui::run_mission_control;
use gorsee_code_ui_state::workspace_state;

use crate::{commands_extra::run_mission, CliOptions};

pub fn run(options: &CliOptions) -> Result<()> {
    let objective = prompt_objective()?;
    if let Some(objective) = objective {
        let summary = run_mission(&options.root, objective, options.env_key.as_deref())?;
        print_summary(&summary);
    }
    run_mission_control(&workspace_state(&options.root))
}

fn prompt_objective() -> Result<Option<String>> {
    print!("Mission objective: ");
    io::stdout().flush()?;

    let mut objective = String::new();
    io::stdin().read_line(&mut objective)?;
    Ok(normalize_objective(&objective))
}

fn normalize_objective(input: &str) -> Option<String> {
    let objective = input.trim();
    if objective.is_empty() || matches!(objective, "q" | "quit" | "exit") {
        return None;
    }
    Some(objective.to_string())
}

fn print_summary(summary: &gorsee_code_agent::MissionRunSummary) {
    println!(
        "mission: completed session={}\nevents={}\nagents={}\nartifacts={}",
        summary.session_id,
        summary.events,
        summary.agents.join(","),
        summary.artifacts.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objective_input_trims_text() {
        assert_eq!(
            normalize_objective("  fix auth tests  \n"),
            Some("fix auth tests".into())
        );
    }

    #[test]
    fn objective_input_quit_is_not_a_mission() {
        assert_eq!(normalize_objective("q\n"), None);
        assert_eq!(normalize_objective("quit\n"), None);
        assert_eq!(normalize_objective("exit\n"), None);
    }
}
