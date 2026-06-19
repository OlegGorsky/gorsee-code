use std::path::Path;

use anyhow::Result;

use crate::{args::ObjectiveArgs, config_file};

const ROUTE: [(&str, &str); 4] = [
    ("architect", "plan"),
    ("scout", "read"),
    ("coder", "change"),
    ("validator", "verify"),
];

pub fn explain(root: &Path, args: ObjectiveArgs) -> Result<String> {
    let config = config_file::load_editable(root)?;
    let objective = args.objective.join(" ");
    let mut out = format!("route: ready\nobjective: {objective}\n");

    for (agent, action) in ROUTE {
        if let Some(profile) = config.agents.get(agent) {
            out.push_str(&format!(
                "- {agent} {} action={action} budget_tokens={}\n",
                profile.model, profile.budget_tokens
            ));
        }
    }

    Ok(out)
}
