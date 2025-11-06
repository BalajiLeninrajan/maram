use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const METADATA_DIR: &str = ".maram";
pub const METADATA_FILE: &str = "metadata.json";
pub const LAYOUT_FILE: &str = "layout.kdl";
pub const BASE_VARIANT: &str = "base";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeMetadata {
    pub branch_name: String,
    pub variants: Vec<String>,
    pub base_path: PathBuf,
    pub variant_paths: std::collections::HashMap<String, PathBuf>,
    pub current_picked_variant: Option<String>,
    pub base_commit: String, // Store the original commit before any picks
}

impl WorktreeMetadata {
    pub fn new(branch_name: String, variants: Vec<String>) -> Self {
        WorktreeMetadata {
            branch_name,
            variants,
            base_path: PathBuf::new(),
            variant_paths: std::collections::HashMap::new(),
            current_picked_variant: None,
            base_commit: String::new(),
        }
    }

    pub fn metadata_dir(worktree_dir: &Path) -> PathBuf {
        worktree_dir.join(METADATA_DIR)
    }

    pub fn load(worktree_dir: &Path) -> Result<Self> {
        let metadata_path = Self::metadata_dir(worktree_dir).join(METADATA_FILE);

        if !metadata_path.exists() {
            anyhow::bail!("Metadata not found at {:?}", metadata_path);
        }

        let content = fs::read_to_string(&metadata_path)
            .with_context(|| format!("Failed to read metadata from {:?}", metadata_path))?;

        let metadata: WorktreeMetadata =
            serde_json::from_str(&content).context("Failed to parse metadata")?;

        Ok(metadata)
    }

    pub fn save(&self, worktree_dir: &Path) -> Result<()> {
        let metadata_dir = Self::metadata_dir(worktree_dir);

        if !metadata_dir.exists() {
            fs::create_dir_all(&metadata_dir).with_context(|| {
                format!("Failed to create metadata directory: {:?}", metadata_dir)
            })?;
        }

        let metadata_path = metadata_dir.join(METADATA_FILE);
        let content = serde_json::to_string_pretty(self).context("Failed to serialize metadata")?;

        fs::write(&metadata_path, content)
            .with_context(|| format!("Failed to write metadata to {:?}", metadata_path))?;

        Ok(())
    }

    pub fn exists(worktree_dir: &Path) -> bool {
        Self::metadata_dir(worktree_dir)
            .join(METADATA_FILE)
            .exists()
    }
}
