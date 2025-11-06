use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_variants: Vec<String>,
    #[serde(default)]
    pub no_session: bool,
    /// Default template for Zellij layouts (KDL format)
    #[serde(default)]
    pub prefix_zellij_layout: Option<String>,
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
            Self::create_default_config(&config_path)
        }
    }

    fn create_default_config(config_path: &PathBuf) -> Result<Config> {
        // Create directory structure if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
        }

        // Create default config
        let default_config = Config {
            default_variants: Vec::new(),
            no_session: false,
            prefix_zellij_layout: Some(
                r#"default_tab_template {
  pane size=1 borderless=true {
      plugin location="zellij:tab-bar"
  }
  children
  pane size=2 borderless=true {
      plugin location="zellij:status-bar"
  }
}"#
                .to_string(),
            ),
        };

        // Write default config to file with exact format
        let toml_content = r#"default_variants = []

no_session = false

prefix_zellij_layout = """
default_tab_template {
  pane size=1 borderless=true {
      plugin location="zellij:tab-bar"
  }
  children
  pane size=2 borderless=true {
      plugin location="zellij:status-bar"
  }
}
"""
"#;
        fs::write(config_path, toml_content)
            .with_context(|| format!("Failed to write config file: {:?}", config_path))?;

        Ok(default_config)
    }

    pub fn config_path() -> Result<PathBuf> {
        // Use ~/.config/maram/config.toml for consistency across platforms
        let home = dirs::home_dir().context("Failed to find home directory")?;
        let dot_config_path = home.join(".config").join("maram").join("config.toml");

        Ok(dot_config_path)
    }
}
