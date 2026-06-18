use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

pub fn config_path(root: &Path) -> PathBuf {
    root.join("gorsee-code.toml")
}

pub fn local_dir(root: &Path) -> PathBuf {
    root.join(".gorsee-code")
}

pub fn sessions_dir(root: &Path) -> PathBuf {
    local_dir(root).join("sessions")
}

pub fn artifacts_dir(root: &Path) -> PathBuf {
    local_dir(root).join("artifacts")
}

pub fn auth_path(root: &Path) -> PathBuf {
    local_dir(root).join("auth.json")
}

pub fn ensure_layout(root: &Path) -> Result<()> {
    fs::create_dir_all(sessions_dir(root)).context("create session store")?;
    fs::create_dir_all(artifacts_dir(root)).context("create artifact store")?;
    Ok(())
}

pub fn project_name(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("gorsee-code")
        .to_string()
}
