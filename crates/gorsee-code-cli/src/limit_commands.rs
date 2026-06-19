use std::path::Path;

use anyhow::Result;
use gorsee_code_limits::{LimitDecision, LimitPolicy, UsageWindow};
use serde::Serialize;

use crate::{
    args::{LimitsArgs, LimitsCommand, LimitsWatchArgs},
    live,
};

pub fn run(root: &Path, env_key: Option<&str>, args: LimitsArgs) -> Result<String> {
    match args.command {
        None => current(root, env_key, args.json),
        Some(LimitsCommand::Watch(watch_args)) => watch(root, env_key, args.json, watch_args),
    }
}

fn current(root: &Path, env_key: Option<&str>, json: bool) -> Result<String> {
    let Some(client) = live::client(root, env_key)? else {
        return missing_auth("limits:", json);
    };
    live::block_on(async move {
        let windows = client.account_limits().await?;
        render_current(&windows, json)
    })
}

fn watch(root: &Path, env_key: Option<&str>, json: bool, _args: LimitsWatchArgs) -> Result<String> {
    let Some(client) = live::client(root, env_key)? else {
        return missing_auth("limits watch:", json);
    };
    live::block_on(async move {
        let windows = client.account_limits().await?;
        render_watch(&windows, json)
    })
}

fn missing_auth(label: &str, json: bool) -> Result<String> {
    if json {
        return Ok(serde_json::to_string(&LimitReport::skipped())?);
    }
    Ok(format!("{label}\nstatus=skipped\nreason=missing_auth\n"))
}

fn render_current(windows: &[UsageWindow], json: bool) -> Result<String> {
    if json {
        return Ok(serde_json::to_string(&LimitReport::live(windows))?);
    }
    Ok(render_text("limits: live\n", windows))
}

fn render_watch(windows: &[UsageWindow], json: bool) -> Result<String> {
    if json {
        return Ok(serde_json::to_string(&LimitReport::live(windows))?);
    }
    Ok(render_text("limits watch:\nstatus=live\n", windows))
}

fn render_text(header: &str, windows: &[UsageWindow]) -> String {
    let mut out = header.to_string();
    out.push_str(&format!(
        "decision={}\n",
        decision_label(&LimitPolicy::default().evaluate(windows))
    ));
    for window in windows {
        out.push_str(&format!(
            "- {} credits={:.1}/{:.1} credit_percent={:.1} requests={}/{} request_percent={:.1}\n",
            window.label,
            window.credits_used,
            window.credit_limit,
            window.credit_percent(),
            window.requests_used,
            window.request_limit,
            window.request_percent()
        ));
    }
    out
}

fn decision_label(decision: &LimitDecision) -> String {
    match decision {
        LimitDecision::Continue => "continue".into(),
        LimitDecision::Warn(window) => format!("warn window={window}"),
        LimitDecision::Stop(window) => format!("stop window={window}"),
    }
}

#[derive(Debug, Serialize)]
struct LimitReport {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
    decision: String,
    windows: Vec<WindowReport>,
}

impl LimitReport {
    fn skipped() -> Self {
        Self {
            status: "skipped",
            reason: Some("missing_auth"),
            decision: "unavailable".into(),
            windows: Vec::new(),
        }
    }

    fn live(windows: &[UsageWindow]) -> Self {
        Self {
            status: "live",
            reason: None,
            decision: decision_label(&LimitPolicy::default().evaluate(windows)),
            windows: windows.iter().map(WindowReport::from).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct WindowReport {
    label: String,
    credits_used: f64,
    credit_limit: f64,
    credit_percent: f64,
    requests_used: u64,
    request_limit: u64,
    request_percent: f64,
    started_at: Option<String>,
    ends_at: Option<String>,
}

impl From<&UsageWindow> for WindowReport {
    fn from(window: &UsageWindow) -> Self {
        Self {
            label: window.label.clone(),
            credits_used: window.credits_used,
            credit_limit: window.credit_limit,
            credit_percent: window.credit_percent(),
            requests_used: window.requests_used,
            request_limit: window.request_limit,
            request_percent: window.request_percent(),
            started_at: window.started_at.clone(),
            ends_at: window.ends_at.clone(),
        }
    }
}
