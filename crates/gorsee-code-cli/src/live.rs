use std::path::Path;

use anyhow::{Context, Result};
use gorsee_code_config::{default_config, GorseeConfig};
use gorsee_code_neurogate::NeuroGateClient;

use crate::{auth, paths};

pub fn client(
    root: &Path,
    env_key: Option<&str>,
    global_auth_path: Option<&Path>,
) -> Result<Option<NeuroGateClient>> {
    let Some(api_key) = auth::api_key_at(root, env_key, global_auth_path)? else {
        return Ok(None);
    };
    let config = GorseeConfig::load(paths::config_path(root))
        .unwrap_or_else(|_| default_config(paths::project_name(root)));
    Ok(Some(NeuroGateClient::new(
        config.neurogate.endpoint,
        api_key,
    )?))
}

pub fn block_on<T>(future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::runtime::Runtime::new()
        .context("start tokio runtime")?
        .block_on(future)
}
