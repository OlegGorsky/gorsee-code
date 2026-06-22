use std::process::Stdio;

use rmcp::{
    model::{CallToolRequestParams, JsonObject},
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde_json::Value;

use crate::{runtime::tokio_runtime, McpCallRequest, McpCallResult, McpError, McpRuntime};

impl McpRuntime {
    pub fn call_tool(&self, request: McpCallRequest) -> Result<McpCallResult, McpError> {
        let runtime = tokio_runtime()?;
        runtime.block_on(self.call_tool_async(request))
    }

    pub async fn call_tool_async(
        &self,
        request: McpCallRequest,
    ) -> Result<McpCallResult, McpError> {
        let server = self
            .servers
            .iter()
            .find(|server| server.name == request.server)
            .ok_or_else(|| McpError::ServerNotFound(request.server.clone()))?;
        if server.disabled {
            return Err(McpError::ServerDisabled(server.name.clone()));
        }
        let command = server
            .command
            .clone()
            .ok_or_else(|| McpError::MissingCommand(server.name.clone()))?;
        let arguments = json_object(request.arguments)?;
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
            .call_tool(CallToolRequestParams::new(request.tool.clone()).with_arguments(arguments))
            .await
            .map_err(|error| McpError::Runtime(error.to_string()))?;
        let raw = serde_json::to_value(&result)?;
        let text = render_call_result(&raw);
        let _ = client.cancel().await;
        Ok(McpCallResult {
            server: request.server,
            tool: request.tool,
            text,
            raw,
        })
    }
}

pub(crate) fn json_object(value: Value) -> Result<JsonObject, McpError> {
    match value {
        Value::Null => Ok(JsonObject::new()),
        Value::Object(map) => Ok(map),
        _ => Err(McpError::InvalidArguments),
    }
}

fn render_call_result(raw: &Value) -> String {
    raw.get("content")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| raw.to_string())
}
