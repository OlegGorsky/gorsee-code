use std::{collections::BTreeMap, fs, path::PathBuf};

use serde_json::Value;

use crate::{call::json_object, *};

#[test]
fn discovers_cursor_and_codex_mcp_configs() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join(".cursor")).unwrap();
    fs::write(
        temp.path().join(".cursor/mcp.json"),
        r#"{
          "mcpServers": {
            "filesystem": {
              "command": "npx",
              "args": ["-y", "@modelcontextprotocol/server-filesystem", "."],
              "env": {"DEBUG": "1"}
            }
          }
        }"#,
    )
    .unwrap();

    let runtime = McpRuntime::discover(temp.path()).unwrap();

    assert_eq!(runtime.servers.len(), 1);
    assert_eq!(runtime.servers[0].name, "filesystem");
    assert_eq!(runtime.bridge_specs()[0].capability, "mcp:call");
    assert!(McpRuntime::rmcp_tool_type_name().contains("Tool"));
}

#[test]
fn discovers_user_configs_and_lets_workspace_override_by_server_name() {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::create_dir_all(home.path().join(".codex")).unwrap();
    fs::write(
        home.path().join(".codex/mcp.json"),
        r#"{
          "mcpServers": {
            "shared": { "command": "user-command" },
            "user-only": { "command": "user-only-command" }
          }
        }"#,
    )
    .unwrap();
    fs::write(
        project.path().join(".mcp.json"),
        r#"{
          "mcpServers": {
            "shared": { "command": "project-command" }
          }
        }"#,
    )
    .unwrap();

    let runtime =
        McpRuntime::discover_with_user_dirs(project.path(), Some(home.path()), None).unwrap();

    assert_eq!(runtime.servers.len(), 2);
    let shared = runtime
        .servers
        .iter()
        .find(|server| server.name == "shared")
        .unwrap();
    let user_only = runtime
        .servers
        .iter()
        .find(|server| server.name == "user-only")
        .unwrap();
    assert_eq!(shared.command.as_deref(), Some("project-command"));
    assert_eq!(shared.source, ".mcp.json");
    assert_eq!(user_only.command.as_deref(), Some("user-only-command"));
    assert_eq!(user_only.source, "user:.codex/mcp.json");
}

#[test]
fn call_tool_rejects_unknown_or_non_object_arguments() {
    let runtime = McpRuntime {
        root: PathBuf::from("."),
        servers: Vec::new(),
    };
    let error = runtime
        .call_tool(McpCallRequest {
            server: "missing".into(),
            tool: "x".into(),
            arguments: Value::Object(Default::default()),
        })
        .unwrap_err();
    assert!(matches!(error, McpError::ServerNotFound(_)));

    assert!(matches!(
        json_object(Value::String("bad".into())),
        Err(McpError::InvalidArguments)
    ));
}

#[test]
fn tool_inventory_reports_per_server_configuration_errors() {
    let runtime = McpRuntime {
        root: PathBuf::from("."),
        servers: vec![
            McpServerConfig {
                name: "disabled".into(),
                source: ".mcp.json".into(),
                command: Some("never-runs".into()),
                args: Vec::new(),
                env: BTreeMap::new(),
                disabled: true,
            },
            McpServerConfig {
                name: "missing-command".into(),
                source: ".codex/mcp.json".into(),
                command: None,
                args: Vec::new(),
                env: BTreeMap::new(),
                disabled: false,
            },
        ],
    };

    let inventory = runtime.tool_inventory().unwrap();
    assert_eq!(inventory.len(), 2);
    assert_eq!(inventory[0].error.as_deref(), Some("server disabled"));
    assert_eq!(
        inventory[1].error.as_deref(),
        Some("server command is missing")
    );

    let rendered = runtime.render_status_with_tools().unwrap();
    assert!(rendered.contains("disabled [.mcp.json] error=server disabled"));
    assert!(rendered.contains("missing-command [.codex/mcp.json] error=server command is missing"));
}
