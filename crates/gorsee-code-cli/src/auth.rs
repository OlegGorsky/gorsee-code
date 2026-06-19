use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthStatus {
    pub configured: bool,
    pub source: Option<String>,
    pub redacted_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct AuthFile {
    api_key: String,
}

pub fn set(root: &Path, api_key: &str) -> Result<AuthStatus> {
    paths::ensure_layout(root)?;
    let auth = AuthFile {
        api_key: api_key.to_string(),
    };
    let text = serde_json::to_string_pretty(&auth).context("render auth file")?;
    write_private(paths::auth_path(root), &text).context("write auth file")?;
    status(root, None)
}

pub fn status(root: &Path, env_key: Option<&str>) -> Result<AuthStatus> {
    if let Some(key) = non_empty(env_key) {
        return Ok(found("env", key));
    }
    match read_local_key(root)? {
        Some(key) => Ok(found("local_file", &key)),
        None => Ok(AuthStatus {
            configured: false,
            source: None,
            redacted_key: None,
        }),
    }
}

pub fn api_key(root: &Path, env_key: Option<&str>) -> Result<Option<String>> {
    if let Some(key) = non_empty(env_key) {
        return Ok(Some(key.to_string()));
    }
    read_local_key(root)
}

pub fn render_status(status: &AuthStatus) -> String {
    match (&status.source, &status.redacted_key) {
        (Some(source), Some(key)) => format!("auth: configured source={source} key={key}\n"),
        _ => "auth: missing\n".to_string(),
    }
}

fn read_local_key(root: &Path) -> Result<Option<String>> {
    let path = paths::auth_path(root);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("read auth file"),
    };
    let auth: AuthFile = serde_json::from_str(&text).context("parse auth file")?;
    Ok(non_empty(Some(&auth.api_key)).map(ToOwned::to_owned))
}

fn found(source: &str, key: &str) -> AuthStatus {
    AuthStatus {
        configured: true,
        source: Some(source.to_string()),
        redacted_key: Some(redact_key(key)),
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn redact_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 8 {
        return "[REDACTED]".into();
    }
    let head: String = chars.iter().take(4).collect();
    let tail: String = chars
        .iter()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{head}...{tail}")
}

#[cfg(unix)]
fn write_private(path: impl AsRef<Path>, text: &str) -> Result<()> {
    use std::{io::Write, os::unix::fs::PermissionsExt};

    let path = path.as_ref();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(text.as_bytes())?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(path: impl AsRef<Path>, text: &str) -> Result<()> {
    fs::write(path, text)?;
    Ok(())
}
