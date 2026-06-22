use std::{process::Stdio, time::Duration};

use rmcp::{
    model::Tool,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};

use crate::{
    runtime::tokio_runtime, McpError, McpRuntime, McpServerConfig, McpToolInfo, McpToolInventory,
};

const TOOL_LIST_TIMEOUT: Duration = Duration::from_secs(5);

impl McpRuntime {
    pub fn tool_inventory(&self) -> Result<Vec<McpToolInventory>, McpError> {
        let runtime = tokio_runtime()?;
        Ok(runtime.block_on(self.tool_inventory_async()))
    }

    pub async fn tool_inventory_async(&self) -> Vec<McpToolInventory> {
        let mut inventory = Vec::new();
        for server in &self.servers {
            inventory.push(self.server_tool_inventory(server).await);
        }
        inventory
    }

    pub fn render_status_with_tools(&self) -> Result<String, McpError> {
        let inventory = self.tool_inventory()?;
        let mut out = self.render_status();
        if inventory.is_empty() {
            return Ok(out);
        }
        out.push_str("tools:\n");
        for server in inventory {
            render_server_tools(&mut out, &server);
        }
        Ok(out)
    }

    async fn server_tool_inventory(&self, server: &McpServerConfig) -> McpToolInventory {
        if server.disabled {
            return tool_inventory_error(server, "server disabled");
        }
        if server.command.is_none() {
            return tool_inventory_error(server, "server command is missing");
        }
        match tokio::time::timeout(TOOL_LIST_TIMEOUT, self.list_server_tools(server)).await {
            Ok(Ok(tools)) => McpToolInventory {
                server: server.name.clone(),
                source: server.source.clone(),
                disabled: false,
                tools,
                error: None,
            },
            Ok(Err(error)) => tool_inventory_error(server, error),
            Err(_) => tool_inventory_error(
                server,
                format!(
                    "tools/list timed out after {}s",
                    TOOL_LIST_TIMEOUT.as_secs()
                ),
            ),
        }
    }

    async fn list_server_tools(
        &self,
        server: &McpServerConfig,
    ) -> Result<Vec<McpToolInfo>, McpError> {
        let command = server
            .command
            .clone()
            .ok_or_else(|| McpError::MissingCommand(server.name.clone()))?;
        let transport =
            TokioChildProcess::new(tokio::process::Command::new(command).configure(|cmd| {
                cmd.args(&server.args).current_dir(&self.root);
                cmd.stderr(Stdio::null());
                for (key, value) in &server.env {
                    cmd.env(key, value);
                }
            }))
            .map_err(|error| McpError::Runtime(error.to_string()))?;
        let client =
            ().serve(transport)
                .await
                .map_err(|error| McpError::Runtime(error.to_string()))?;
        let result = client
            .list_tools(Default::default())
            .await
            .map_err(|error| McpError::Runtime(error.to_string()))?;
        let _ = client.cancel().await;
        Ok(result.tools.into_iter().map(tool_info).collect())
    }
}

fn tool_inventory_error(
    server: &McpServerConfig,
    error: impl std::fmt::Display,
) -> McpToolInventory {
    McpToolInventory {
        server: server.name.clone(),
        source: server.source.clone(),
        disabled: server.disabled,
        tools: Vec::new(),
        error: Some(error.to_string()),
    }
}

fn tool_info(tool: Tool) -> McpToolInfo {
    McpToolInfo {
        name: tool.name.into_owned(),
        title: tool.title,
        description: tool.description.map(|description| description.into_owned()),
    }
}

fn render_server_tools(out: &mut String, inventory: &McpToolInventory) {
    if let Some(error) = &inventory.error {
        out.push_str(&format!(
            "- {} [{}] error={}\n",
            inventory.server, inventory.source, error
        ));
        return;
    }
    if inventory.tools.is_empty() {
        out.push_str(&format!(
            "- {} [{}] tools=none\n",
            inventory.server, inventory.source
        ));
        return;
    }
    for tool in &inventory.tools {
        let description = tool.description.as_deref().unwrap_or("");
        out.push_str(&format!(
            "- {}::{} {}\n",
            inventory.server, tool.name, description
        ));
    }
}
