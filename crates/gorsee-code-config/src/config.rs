use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("config parse failed: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("config render failed: {0}")]
    Render(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GorseeConfig {
    pub project: ProjectConfig,
    pub neurogate: NeuroGateConfig,
    pub budget: BudgetConfig,
    pub agents: std::collections::BTreeMap<String, AgentConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub guidance_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeuroGateConfig {
    pub endpoint: String,
    pub auth_source: AuthSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthSource {
    Env,
    LocalFile,
    Keyring,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetConfig {
    pub mission_tokens: u64,
    pub mission_usd: f64,
    pub warn_at_percent: u8,
    pub stop_at_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentConfig {
    pub model: String,
    pub reasoning: String,
    pub tools: Vec<String>,
    pub budget_tokens: u64,
    pub temperature: f32,
}

impl GorseeConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path)?;
        Ok(toml::from_str(&text)?)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        let text = toml::to_string_pretty(self)?;
        fs::write(path, text)?;
        Ok(())
    }
}
