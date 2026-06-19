use std::{io::ErrorKind, path::Path};

use anyhow::{Context, Result};
use gorsee_code_config::{config::ConfigError, default_config, GorseeConfig};

use crate::paths;

pub fn load_editable(root: &Path) -> Result<GorseeConfig> {
    match GorseeConfig::load(paths::config_path(root)) {
        Ok(config) => Ok(config),
        Err(ConfigError::Io(error)) if error.kind() == ErrorKind::NotFound => {
            Ok(default_config(paths::project_name(root)))
        }
        Err(error) => Err(error).context("load gorsee-code.toml"),
    }
}

pub fn save(root: &Path, config: &GorseeConfig) -> Result<()> {
    config
        .save(paths::config_path(root))
        .context("write gorsee-code.toml")
}
