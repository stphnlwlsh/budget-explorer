//! Application configuration

#![allow(dead_code, unused_imports, unused_variables)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_plan_id: Option<String>,
    pub currency_symbol: String,
    pub last_updated: String,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(serde_json::Error),
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Config::default());
        }
        let content = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
        serde_json::from_str(&content).map_err(ConfigError::Parse)
    }

    fn path() -> Result<PathBuf, ConfigError> {
        let dir = dirs::config_dir().ok_or_else(|| {
            ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not find config directory",
            ))
        })?;
        Ok(dir.join("budget-explorer").join("config.json"))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_plan_id: None,
            currency_symbol: "$".to_string(),
            last_updated: chrono::Utc::now().to_rfc3339(),
        }
    }
}
