use crate::config::Config;
use crate::metadata::{BASE_VARIANT, WorktreeMetadata};
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub struct ZellijSession {
    session_name: String,
}

impl ZellijSession {
    pub fn session_name(repo_name: &str, branch_name: &str) -> String {
        // Sanitize session name: zellij doesn't allow '/' in session names
        let repo_name = repo_name.replace("/", "-");
        let branch_name = branch_name.replace("/", "-");
        format!("{}-{}", repo_name, branch_name)
            .chars()
            .take(32)
            .collect::<String>()
    }

    pub fn new(session_name: String) -> Self {
        ZellijSession { session_name }
    }

    pub fn from_repo_and_branch(repo_name: &str, branch_name: &str) -> Self {
        Self::new(Self::session_name(repo_name, branch_name))
    }

    pub fn name(&self) -> &str {
        &self.session_name
    }

    pub fn session_exists(&self) -> bool {
        let output = Command::new("zellij")
            .args(["list-sessions", "-s"])
            .output()
            .ok();

        if let Some(output) = output
            && let Ok(output_str) = String::from_utf8(output.stdout)
        {
            return output_str
                .lines()
                .any(|line| line.trim() == self.session_name);
        }
        false
    }

    pub fn create_session(&self, tabs: &[(&str, &Path)], layout_path: Option<&Path>) -> Result<()> {
        let layout_file = if let Some(layout_path) = layout_path {
            layout_path.to_path_buf()
        } else {
            // Generate layout from tabs and save to temp file
            let layout = self.create_layout(tabs)?;
            let temp_layout = std::env::temp_dir().join(format!("maram-{}.kdl", self.session_name));
            std::fs::write(&temp_layout, layout).context("Failed to write layout file")?;
            temp_layout
        };

        let status = Command::new("zellij")
            .args([
                "--session",
                &self.session_name,
                "--new-session-with-layout",
                layout_file
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("Layout path contains invalid UTF-8"))?,
            ])
            .status()
            .context("Failed to execute zellij")?;

        if !status.success() {
            anyhow::bail!("Failed to create zellij session");
        }

        Ok(())
    }

    pub fn attach_session(&self) -> Result<()> {
        if !self.session_exists() {
            anyhow::bail!("Session {} does not exist", self.session_name);
        }

        let status = Command::new("zellij")
            .args(["attach", &self.session_name])
            .status()
            .context("Failed to attach to zellij session")?;

        if !status.success() {
            anyhow::bail!("Failed to attach to zellij session");
        }

        Ok(())
    }

    pub fn kill_session(&self) -> Result<()> {
        let status = Command::new("zellij")
            .args(["kill-session", &self.session_name])
            .status()
            .context("Failed to kill zellij session")?;

        if !status.success() {
            anyhow::bail!("Failed to kill zellij session");
        }

        Ok(())
    }

    fn create_layout(&self, tabs: &[(&str, &Path)]) -> Result<String> {
        let mut layout = String::from("layout {\n");

        // Add layout prefix from config if provided
        if let Ok(config) = Config::load()
            && let Some(template) = &config.prefix_zellij_layout
        {
            let indented_template: String = template
                .lines()
                .map(|line| {
                    if line.trim().is_empty() {
                        "\n".to_string()
                    } else {
                        format!("  {}\n", line)
                    }
                })
                .collect();
            layout.push_str(&indented_template);
        }

        // Add tabs
        for (tab_name, path) in tabs {
            let path_str = path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Path contains invalid UTF-8: {:?}", path))?;
            layout.push_str(&format!("  tab \"{}\" {{\n", tab_name));
            layout.push_str(&format!("    pane cwd=\"{}\" command=\"zsh\"\n", path_str));
            layout.push_str("  }\n");
        }

        layout.push_str("}\n");

        Ok(layout)
    }

    pub fn save_layout(&self, layout_path: &Path, tabs: &[(&str, &Path)]) -> Result<()> {
        let layout = self.create_layout(tabs)?;
        std::fs::write(layout_path, layout)
            .with_context(|| format!("Failed to write layout to {:?}", layout_path))?;
        Ok(())
    }

    pub fn tabs_from_metadata(metadata: &WorktreeMetadata) -> Vec<(&str, &Path)> {
        let mut tabs = vec![(BASE_VARIANT, metadata.base_path.as_path())];
        for (variant, path) in &metadata.variant_paths {
            tabs.push((variant.as_str(), path.as_path()));
        }
        tabs
    }

    pub fn create_or_attach_with_layout(
        &self,
        tabs: &[(&str, &Path)],
        layout_path: Option<&Path>,
    ) -> Result<()> {
        if self.session_exists() {
            self.attach_session()?;
        } else {
            let layout_to_use = if let Some(layout_path) = layout_path
                && layout_path.exists()
            {
                Some(layout_path)
            } else {
                None
            };
            self.create_session(tabs, layout_to_use)?;
        }
        Ok(())
    }
}
