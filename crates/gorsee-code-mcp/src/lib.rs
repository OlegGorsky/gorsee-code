mod call;
mod config;
mod inventory;
mod runtime;

#[cfg(test)]
mod tests;

use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use config::sanitize_tool_name;

pub type RmcpToolModel = rmcp::model::Tool;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("mcp config io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("mcp config json failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("mcp server not found: {0}")]
    ServerNotFound(String),
    #[error("mcp server is disabled: {0}")]
    ServerDisabled(String),
    #[error("mcp server has no command: {0}")]
    MissingCommand(String),
    #[error("mcp tool arguments must be a JSON object")]
    InvalidArguments,
    #[error("mcp runtime failed: {0}")]
    Runtime(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpRuntime {
    pub root: PathBuf,
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub source: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpToolBridge {
    pub server: String,
    pub tool_name: String,
    pub capability: String,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpToolInventory {
    pub server: String,
    pub source: String,
    pub disabled: bool,
    pub tools: Vec<McpToolInfo>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpCallRequest {
    pub server: String,
    pub tool: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpCallResult {
    pub server: String,
    pub tool: String,
    pub text: String,
    pub raw: Value,
}

impl McpRuntime {
    pub fn bridge_specs(&self) -> Vec<McpToolBridge> {
        self.servers
            .iter()
            .filter(|server| !server.disabled)
            .map(|server| McpToolBridge {
                server: server.name.clone(),
                tool_name: format!("mcp__{}", sanitize_tool_name(&server.name)),
                capability: "mcp:call".into(),
                requires_approval: true,
            })
            .collect()
    }

    pub fn render_status(&self) -> String {
        let mut out = format!("mcp:\nsource=rmcp\nservers={}\n", self.servers.len());
        if self.servers.is_empty() {
            out.push_str("configs=none\n");
            return out;
        }
        for server in &self.servers {
            let command = server.command.as_deref().unwrap_or("not configured");
            let disabled = if server.disabled { " disabled" } else { "" };
            out.push_str(&format!(
                "- {} [{}]{} command={} args={}\n",
                server.name,
                server.source,
                disabled,
                command,
                server.args.join(" ")
            ));
        }
        out
    }

    pub fn rmcp_tool_type_name() -> &'static str {
        std::any::type_name::<RmcpToolModel>()
    }
}
