use std::{fs, path::Path};

use anyhow::{Context, Result};
use gorsee_code_config::default_config_toml;

use crate::{auth, paths};

pub fn init(root: &Path) -> Result<String> {
    paths::ensure_layout(root)?;
    let config_path = paths::config_path(root);
    if !config_path.exists() {
        let text =
            default_config_toml(paths::project_name(root)).context("render default config")?;
        fs::write(&config_path, text).context("write gorsee-code.toml")?;
    }
    Ok(format!(
        "initialized: {}\nstate: {}\n",
        config_path.display(),
        paths::local_dir(root).display()
    ))
}

pub fn setup(root: &Path, env_key: Option<&str>) -> Result<String> {
    let mut out = "setup: ready\n".to_string();
    out.push_str(&init(root)?);
    if let Some(key) = env_key.map(str::trim).filter(|key| !key.is_empty()) {
        auth::set(root, key)?;
    }
    out.push_str(&auth::render_status(&auth::status(root, None)?));
    Ok(out)
}

pub fn reset(root: &Path, yes: bool) -> Result<String> {
    if !yes {
        anyhow::bail!("reset requires --yes");
    }
    remove_if_exists(paths::local_dir(root))?;
    remove_file_if_exists(paths::config_path(root))?;
    Ok("reset: complete\n".into())
}

fn remove_if_exists(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if path.exists() {
        fs::remove_dir_all(path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

fn remove_file_if_exists(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}
