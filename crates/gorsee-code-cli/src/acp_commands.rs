use std::{
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
};

use anyhow::Result;
use gorsee_code_acp::{
    stdio::{AcpPromptRunner, AcpProtocolState},
    AcpAdapter, AcpTurnInput, AcpTurnOutput,
};
use gorsee_code_agent::EventObserver;
use gorsee_code_core::default_agent_matrix;
use gorsee_code_neurogate::NeuroGateClient;
use serde_json::json;

use crate::{
    args::AcpTurnArgs, commands_extra::require_live_client, interactive_agents::live_turn_agents,
    live,
};

pub(crate) fn status() -> Result<String> {
    Ok("acp:\nprotocol=v1\nsurface=cli\ncommands=plan,run,stdio\n".into())
}

pub(crate) fn plan(root: &Path, args: AcpTurnArgs) -> Result<String> {
    let input = input(root, args);
    let plan = AcpAdapter::default().plan_prompt(input, default_agent_matrix());
    Ok(format!("{}\n", serde_json::to_string_pretty(&plan)?))
}

pub(crate) fn run(
    root: &Path,
    args: AcpTurnArgs,
    env_key: Option<&str>,
    global_auth_path: Option<&Path>,
) -> Result<String> {
    let input = input(root, args);
    let client = require_live_client(root, env_key, global_auth_path)?;
    let agents = live_turn_agents(&client, root, &input.prompt)?;
    let output = AcpAdapter::default().run_prompt(input, agents, &client)?;
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(&json!({
            "protocol_version": output.protocol_version,
            "session_id": output.summary.session_id,
            "events": output.summary.events,
            "agents": output.summary.agents,
            "artifacts": output.summary.artifacts,
            "intent": output.orchestration.intent,
            "plan": output.orchestration.plan,
            "response": output.response,
        }))?
    ))
}

pub(crate) fn stdio(
    root: &Path,
    env_key: Option<&str>,
    global_auth_path: Option<&Path>,
) -> Result<String> {
    let client = live::client(root, env_key, global_auth_path)?;
    let mut state = AcpProtocolState::new(root);
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let responses = state.handle_line_batch(
            &line,
            &LiveAcpRunner {
                root: root.to_path_buf(),
                client: client.as_ref(),
            },
        )?;
        for response in responses {
            writeln!(stdout, "{response}")?;
        }
        stdout.flush()?;
    }

    Ok(String::new())
}

struct LiveAcpRunner<'a> {
    root: PathBuf,
    client: Option<&'a NeuroGateClient>,
}

impl AcpPromptRunner for LiveAcpRunner<'_> {
    fn run_acp_prompt(&self, input: AcpTurnInput) -> Result<AcpTurnOutput, String> {
        self.run_acp_prompt_observed(input, Box::new(|_| Ok(())))
    }

    fn run_acp_prompt_observed(
        &self,
        input: AcpTurnInput,
        observer: Box<EventObserver>,
    ) -> Result<AcpTurnOutput, String> {
        let Some(client) = self.client else {
            return Err(
                "missing_auth: run `gcode` and enter a NeuroGate API key or set NEUROGATE_API_KEY"
                    .into(),
            );
        };
        let agents = live_turn_agents(client, &self.root, &input.prompt)
            .map_err(|error| error.to_string())?;
        AcpAdapter::default()
            .run_prompt_with_event_observer(input, agents, client, observer)
            .map_err(|error| error.to_string())
    }
}

fn input(root: &Path, args: AcpTurnArgs) -> AcpTurnInput {
    AcpTurnInput {
        session_id: args.session,
        workspace_root: root.display().to_string(),
        prompt: args.objective.join(" "),
        user_id: Some("cli".into()),
    }
}
