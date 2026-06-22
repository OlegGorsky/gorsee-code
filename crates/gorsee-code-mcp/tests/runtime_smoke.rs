use std::fs;

use gorsee_code_mcp::{McpCallRequest, McpRuntime};
use serde_json::json;

#[test]
fn runtime_lists_and_calls_real_child_process_tool() {
    let temp = tempfile::tempdir().unwrap();
    let fixture = env!("CARGO_BIN_EXE_gorsee-mcp-fixture");
    fs::write(
        temp.path().join(".mcp.json"),
        json!({
            "mcpServers": {
                "fixture": {
                    "command": fixture
                }
            }
        })
        .to_string(),
    )
    .unwrap();

    let runtime = McpRuntime::discover(temp.path()).unwrap();
    let inventory = runtime.tool_inventory().unwrap();

    assert_eq!(inventory.len(), 1);
    assert_eq!(inventory[0].server, "fixture");
    assert!(
        inventory[0].tools.iter().any(|tool| tool.name == "echo"),
        "inventory: {inventory:#?}"
    );

    let result = runtime
        .call_tool(McpCallRequest {
            server: "fixture".into(),
            tool: "echo".into(),
            arguments: json!({ "text": "ping" }),
        })
        .unwrap();

    assert_eq!(result.server, "fixture");
    assert_eq!(result.tool, "echo");
    assert_eq!(result.text, "fixture: ping");
    assert_eq!(result.raw["content"][0]["text"], "fixture: ping");
}
