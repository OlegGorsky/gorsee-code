use std::path::PathBuf;

use gorsee_code_mcp::{McpCallRequest, McpRuntime};
use gorsee_code_safety::RiskClass;
use gorsee_code_tool_runtime::{Tool, ToolManifest, ToolOutput, ToolRegistry, ToolRuntimeError};
use serde_json::{json, Value};

pub struct McpInventoryTool {
    root: PathBuf,
}

pub struct McpCallTool {
    root: PathBuf,
}

pub struct McpServerTool {
    root: PathBuf,
    server: String,
    name: String,
}

impl McpInventoryTool {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl McpCallTool {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl McpServerTool {
    pub fn new(root: PathBuf, server: String, name: String) -> Self {
        Self { root, server, name }
    }
}

pub fn register_server_tools(registry: &mut ToolRegistry, root: PathBuf) {
    let Ok(runtime) = McpRuntime::discover(&root) else {
        return;
    };
    for bridge in runtime.bridge_specs() {
        registry.register(McpServerTool::new(
            root.clone(),
            bridge.server,
            bridge.tool_name,
        ));
    }
}

impl Tool for McpInventoryTool {
    fn manifest(&self) -> ToolManifest {
        ToolManifest {
            name: "mcp_inventory".into(),
            description: "List configured MCP servers and their tools".into(),
            risk: RiskClass::Read,
            capabilities: vec!["mcp:list".into()],
        }
    }

    fn run(&self, _args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let runtime = McpRuntime::discover(&self.root)
            .map_err(|error| ToolRuntimeError::Handler(error.to_string()))?;
        let inventory = runtime
            .tool_inventory()
            .map_err(|error| ToolRuntimeError::Handler(error.to_string()))?;
        let text = render_inventory(&runtime, &inventory);
        Ok(ToolOutput {
            text,
            json: Some(json!({
                "servers": runtime.servers,
                "inventory": inventory,
                "bridge_tools": runtime.bridge_specs(),
            })),
            truncated: false,
        })
    }
}

impl Tool for McpCallTool {
    fn manifest(&self) -> ToolManifest {
        ToolManifest {
            name: "mcp_call".into(),
            description: "Call a configured MCP server tool".into(),
            risk: RiskClass::Network,
            capabilities: vec!["mcp:call".into()],
        }
    }

    fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let request = parse_request(args)?;
        let runtime = McpRuntime::discover(&self.root)
            .map_err(|error| ToolRuntimeError::Handler(error.to_string()))?;
        let result = runtime
            .call_tool(request)
            .map_err(|error| ToolRuntimeError::Handler(error.to_string()))?;
        Ok(call_output(result))
    }
}

impl Tool for McpServerTool {
    fn manifest(&self) -> ToolManifest {
        ToolManifest {
            name: self.name.clone(),
            description: format!("Call MCP tools on configured server {}", self.server),
            risk: RiskClass::Network,
            capabilities: vec!["mcp:call".into()],
        }
    }

    fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let tool = required_string(&args, "tool")?;
        let arguments = args.get("arguments").cloned().unwrap_or(Value::Null);
        let runtime = McpRuntime::discover(&self.root)
            .map_err(|error| ToolRuntimeError::Handler(error.to_string()))?;
        let result = runtime
            .call_tool(McpCallRequest {
                server: self.server.clone(),
                tool,
                arguments,
            })
            .map_err(|error| ToolRuntimeError::Handler(error.to_string()))?;
        Ok(call_output(result))
    }
}

fn call_output(result: gorsee_code_mcp::McpCallResult) -> ToolOutput {
    ToolOutput {
        text: result.text,
        json: Some(json!({
            "server": result.server,
            "tool": result.tool,
            "raw": result.raw,
        })),
        truncated: false,
    }
}

fn render_inventory(
    runtime: &McpRuntime,
    inventory: &[gorsee_code_mcp::McpToolInventory],
) -> String {
    let mut out = runtime.render_status();
    if inventory.is_empty() {
        return out;
    }
    out.push_str("tools:\n");
    for server in inventory {
        if let Some(error) = &server.error {
            out.push_str(&format!("- {} error={}\n", server.server, error));
            continue;
        }
        if server.tools.is_empty() {
            out.push_str(&format!("- {} tools=none\n", server.server));
            continue;
        }
        for tool in &server.tools {
            out.push_str(&format!("- {}::{}\n", server.server, tool.name));
        }
    }
    out
}

fn parse_request(args: Value) -> Result<McpCallRequest, ToolRuntimeError> {
    let server = required_string(&args, "server")?;
    let tool = required_string(&args, "tool")?;
    let arguments = args.get("arguments").cloned().unwrap_or(Value::Null);
    Ok(McpCallRequest {
        server,
        tool,
        arguments,
    })
}

fn required_string(args: &Value, key: &str) -> Result<String, ToolRuntimeError> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| ToolRuntimeError::Handler(format!("missing mcp_call.{key}")))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_mcp_call_request() {
        let request = parse_request(json!({
            "server": "fs",
            "tool": "read",
            "arguments": {"path": "README.md"}
        }))
        .unwrap();

        assert_eq!(request.server, "fs");
        assert_eq!(request.tool, "read");
        assert_eq!(request.arguments["path"], "README.md");
    }

    #[test]
    fn mcp_call_is_network_risk_for_shared_approval_policy() {
        let manifest = McpCallTool::new(PathBuf::from(".")).manifest();

        assert_eq!(manifest.risk, RiskClass::Network);
        assert_eq!(manifest.capabilities, ["mcp:call"]);
    }

    #[test]
    fn mcp_inventory_is_readable_and_structured_without_configs() {
        let temp = tempfile::tempdir().unwrap();

        let output = McpInventoryTool::new(temp.path().to_path_buf())
            .run(json!({}))
            .unwrap();

        assert!(output.text.contains("servers=0"));
        assert_eq!(output.json.as_ref().unwrap()["inventory"], json!([]));
    }

    #[test]
    fn register_server_tools_exposes_configured_mcp_bridge() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join(".mcp.json"),
            r#"{"mcpServers":{"fs-server":{"command":"echo","args":["ok"]}}}"#,
        )
        .unwrap();

        let registry = crate::builtin_registry(temp.path()).unwrap();
        let manifest = registry.manifest("mcp__fs_server").unwrap();

        assert_eq!(manifest.risk, RiskClass::Network);
        assert_eq!(manifest.capabilities, ["mcp:call"]);
    }

    #[test]
    fn mcp_call_output_keeps_structured_raw_result() {
        let output = call_output(gorsee_code_mcp::McpCallResult {
            server: "fs".into(),
            tool: "read".into(),
            text: "hello".into(),
            raw: json!({"content":[{"type":"text","text":"hello"}]}),
        });

        assert_eq!(output.text, "hello");
        assert_eq!(output.json.as_ref().unwrap()["server"], "fs");
        assert_eq!(output.json.as_ref().unwrap()["tool"], "read");
        assert_eq!(
            output.json.as_ref().unwrap()["raw"]["content"][0]["text"],
            "hello"
        );
    }
}
