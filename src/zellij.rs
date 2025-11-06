use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub struct ZellijSession {
    session_name: String,
}

impl ZellijSession {
    pub fn new(session_name: String) -> Self {
        ZellijSession { session_name }
    }

    pub fn session_exists(&self) -> bool {
        let output = Command::new("zellij").args(["list-sessions"]).output().ok();

        if let Some(output) = output
            && let Ok(output_str) = String::from_utf8(output.stdout)
        {
            return output_str
                .lines()
                .any(|line| line.trim() == self.session_name);
        }
        false
    }

    pub fn create_session(&self, tabs: &[(&str, &Path)]) -> Result<()> {
        let layout = self.create_layout(tabs)?;

        // Save layout to temp file
        let temp_layout = std::env::temp_dir().join(format!("maram-{}.kdl", self.session_name));
        std::fs::write(&temp_layout, layout).context("Failed to write layout file")?;

        // Create new session with layout
        let status = Command::new("zellij")
            .args([
                "--session",
                &self.session_name,
                "--layout",
                temp_layout.to_str().unwrap(),
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

        // Add tabs
        for (tab_name, path) in tabs {
            let path_str = path.to_str().unwrap();
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
}
