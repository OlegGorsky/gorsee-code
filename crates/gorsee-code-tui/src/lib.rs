use std::io::{self, Read, Write};

use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use gorsee_code_ui_state::MissionControlState;

pub fn run_mission_control(state: &MissionControlState) -> Result<()> {
    let mut stdout = io::stdout();
    let _terminal = TerminalSession::enter(&mut stdout)?;
    draw(&mut stdout, state)?;
    read_until_quit()
}

pub fn render_mission_control(state: &MissionControlState) -> String {
    let mut out = String::new();
    push_header(&mut out, state);
    push_agents(&mut out, state);
    push_timeline(&mut out, state);
    push_inspector(&mut out, state);
    out
}

struct TerminalSession {
    raw_enabled: bool,
}

impl TerminalSession {
    fn enter(stdout: &mut impl Write) -> Result<Self> {
        let raw_enabled = terminal::enable_raw_mode().is_ok();
        execute!(stdout, EnterAlternateScreen, Hide)?;
        Ok(Self { raw_enabled })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, Show, LeaveAlternateScreen);
        if self.raw_enabled {
            let _ = terminal::disable_raw_mode();
        }
    }
}

fn draw(stdout: &mut impl Write, state: &MissionControlState) -> Result<()> {
    execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    write!(
        stdout,
        "{}\nq quit | Esc close | Ctrl-C cancel\n",
        render_mission_control(state)
    )?;
    stdout.flush()?;
    Ok(())
}

fn read_until_quit() -> Result<()> {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut byte = [0_u8; 1];
    loop {
        if stdin.read(&mut byte)? == 0 {
            return Ok(());
        }
        if matches!(byte[0], b'q' | b'Q' | 0x1b | 0x03) {
            return Ok(());
        }
    }
}

fn push_header(out: &mut String, state: &MissionControlState) {
    out.push_str(&format!("Gorsee Code Mission: {}\n", state.session.title));
    out.push_str(&format!(
        "session={} status={} gateway={}\n\n",
        state.session.id, state.session.status, state.gateway_status
    ));
}

fn push_agents(out: &mut String, state: &MissionControlState) {
    out.push_str("Agents\n");
    for agent in &state.agents {
        out.push_str(&format!(
            "- {} {} {} {}/{}\n",
            agent.id, agent.status, agent.model, agent.tokens_used, agent.tokens_limit
        ));
    }
    out.push('\n');
}

fn push_timeline(out: &mut String, state: &MissionControlState) {
    out.push_str("Timeline\n");
    for event in &state.timeline {
        let agent = event.agent_id.as_deref().unwrap_or("mission");
        out.push_str(&format!(
            "- #{:04} {} {}: {}\n",
            event.sequence, event.kind, agent, event.summary
        ));
    }
    out.push('\n');
}

fn push_inspector(out: &mut String, state: &MissionControlState) {
    out.push_str("Inspector\n");
    out.push_str(&format!(
        "- budget: {}/{} ({:.1}%)\n",
        state.budget.used_tokens, state.budget.limit_tokens, state.budget.percent_used
    ));
    for approval in &state.approvals {
        out.push_str(&format!(
            "- approval: {} {} {}\n",
            approval.id, approval.name, approval.risk
        ));
    }
}
