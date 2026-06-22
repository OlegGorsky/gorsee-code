use std::{
    fs,
    path::{Path, PathBuf},
};

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

pub fn set_global(api_key: &str) -> Result<()> {
    set_global_at(paths::global_auth_path().as_deref(), api_key)
}

pub fn set_global_at(path: Option<&Path>, api_key: &str) -> Result<()> {
    let Some(path) = path else { return Ok(()) };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create global auth dir")?;
    }
    let auth = AuthFile {
        api_key: api_key.to_string(),
    };
    let text = serde_json::to_string_pretty(&auth).context("render global auth file")?;
    write_private(path, &text).context("write global auth file")
}

pub fn status(root: &Path, env_key: Option<&str>) -> Result<AuthStatus> {
    status_at(root, env_key, paths::global_auth_path().as_deref())
}

pub fn status_at(
    root: &Path,
    env_key: Option<&str>,
    global_auth_path: Option<&Path>,
) -> Result<AuthStatus> {
    if let Some(key) = non_empty(env_key) {
        return Ok(found("env", key));
    }
    match read_local_key(root)? {
        Some(key) => Ok(found("local_file", &key)),
        None => match read_global_key(global_auth_path)? {
            Some(key) => Ok(found("global_file", &key)),
            None => Ok(AuthStatus {
                configured: false,
                source: None,
                redacted_key: None,
            }),
        },
    }
}

pub fn api_key(root: &Path, env_key: Option<&str>) -> Result<Option<String>> {
    api_key_at(root, env_key, paths::global_auth_path().as_deref())
}

pub fn api_key_at(
    root: &Path,
    env_key: Option<&str>,
    global_auth_path: Option<&Path>,
) -> Result<Option<String>> {
    if let Some(key) = non_empty(env_key) {
        return Ok(Some(key.to_string()));
    }
    read_local_key(root)?.map_or_else(|| read_global_key(global_auth_path), |key| Ok(Some(key)))
}

pub fn render_status(status: &AuthStatus) -> String {
    match (&status.source, &status.redacted_key) {
        (Some(source), Some(key)) => format!("auth: configured source={source} key={key}\n"),
        _ => "auth: missing\n".to_string(),
    }
}

fn read_local_key(root: &Path) -> Result<Option<String>> {
    read_key_file(paths::auth_path(root))
}

fn read_global_key(path: Option<&Path>) -> Result<Option<String>> {
    match path {
        Some(path) => read_key_file(path),
        None => Ok(None),
    }
}

fn read_key_file(path: impl AsRef<Path>) -> Result<Option<String>> {
    let text = match fs::read_to_string(path.as_ref()) {
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
    let tmp_path = temp_path(path);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&tmp_path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600))?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(path: impl AsRef<Path>, text: &str) -> Result<()> {
    let path = path.as_ref();
    let tmp_path = temp_path(path);
    fs::write(&tmp_path, text)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

fn temp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("auth.json");
    path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}
