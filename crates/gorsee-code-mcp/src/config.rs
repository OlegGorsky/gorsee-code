use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;

use crate::{McpError, McpRuntime, McpServerConfig};

impl McpRuntime {
    pub fn discover(root: impl AsRef<Path>) -> Result<Self, McpError> {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let xdg_config = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
        Self::discover_with_user_dirs(root, home.as_deref(), xdg_config.as_deref())
    }

    pub fn discover_with_user_dirs(
        root: impl AsRef<Path>,
        home: Option<&Path>,
        xdg_config: Option<&Path>,
    ) -> Result<Self, McpError> {
        let root = root.as_ref();
        let mut by_name = BTreeMap::new();
        for (source, path) in config_paths(root, home, xdg_config) {
            if !path.exists() {
                continue;
            }
            let value = serde_json::from_str::<Value>(&fs::read_to_string(&path)?)?;
            for server in parse_servers(&source, &value) {
                by_name.insert(server.name.clone(), server);
            }
        }
        let mut servers = by_name.into_values().collect::<Vec<_>>();
        servers.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(Self {
            root: root.to_path_buf(),
            servers,
        })
    }
}

fn config_paths(
    root: &Path,
    home: Option<&Path>,
    xdg_config: Option<&Path>,
) -> Vec<(String, PathBuf)> {
    let mut paths = Vec::new();
    if let Some(home) = home {
        for source in [".mcp.json", ".cursor/mcp.json", ".codex/mcp.json"] {
            paths.push((format!("user:{source}"), home.join(source)));
        }
    }
    if let Some(xdg_config) = xdg_config {
        paths.push((
            "user:gorsee-code/mcp.json".into(),
            xdg_config.join("gorsee-code/mcp.json"),
        ));
    }
    for source in [".mcp.json", ".cursor/mcp.json", ".codex/mcp.json"] {
        paths.push((source.into(), root.join(source)));
    }
    paths
}

fn parse_servers(source: &str, value: &Value) -> Vec<McpServerConfig> {
    let Some(map) = value
        .get("mcpServers")
        .or_else(|| value.get("servers"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    map.iter()
        .filter_map(|(name, config)| parse_server(source, name, config))
        .collect()
}

fn parse_server(source: &str, name: &str, config: &Value) -> Option<McpServerConfig> {
    let command = config
        .get("command")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let args = config
        .get("args")
        .and_then(Value::as_array)
        .map(|args| args.iter().filter_map(value_string).collect())
        .unwrap_or_default();
    let env = config
        .get("env")
        .and_then(Value::as_object)
        .map(|env| {
            env.iter()
                .filter_map(|(key, value)| value_string(value).map(|value| (key.clone(), value)))
                .collect()
        })
        .unwrap_or_default();
    let disabled = config
        .get("disabled")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            config
                .get("enabled")
                .and_then(Value::as_bool)
                .map(|enabled| !enabled)
                .unwrap_or(false)
        });
    Some(McpServerConfig {
        name: name.into(),
        source: source.into(),
        command,
        args,
        env,
        disabled,
    })
}

fn value_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(crate) fn sanitize_tool_name(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
