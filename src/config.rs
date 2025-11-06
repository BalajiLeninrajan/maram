use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_variants: Vec<String>,
}

impl Config {
    pub fn load() -> Result<Config> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config file: {:?}", config_path))?;
            let config: Config = toml::from_str(&content)
                .with_context(|| {
                    format!(
                        "Failed to parse config file at {:?}. Please check the TOML syntax. Content: {}",
                        config_path,
                        content
                    )
                })?;
            Ok(config)
        } else {
            Ok(Config {
                default_variants: Vec::new(),
            })
        }
    }

    pub fn config_path() -> Result<PathBuf> {
        // Prefer ~/.config/maram/config.toml for consistency across platforms
        // Fall back to platform-specific config directory if ~/.config doesn't exist
        let home = dirs::home_dir().context("Failed to find home directory")?;
        let dot_config_path = home.join(".config").join("maram").join("config.toml");
        
        if dot_config_path.exists() {
            return Ok(dot_config_path);
        }
        
        // Fall back to platform-specific config directory
        let config_dir = dirs::config_dir()
            .context("Failed to find config directory")?
            .join("maram");

        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)
                .with_context(|| format!("Failed to create config directory: {:?}", config_dir))?;
        }

        Ok(config_dir.join("config.toml"))
    }
}
