use crate::metadata::WorktreeMetadata;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub struct WorktreeSet {
    pub metadata: WorktreeMetadata,
    pub base_dir: PathBuf,
}

impl WorktreeSet {
    pub fn get_maram_dir() -> Result<PathBuf> {
        if let Ok(maram_dir) = std::env::var("MARAM_DIR") {
            Ok(PathBuf::from(maram_dir))
        } else {
            let home = dirs::home_dir().context("Failed to find home directory")?;
            Ok(home.join("maram"))
        }
    }

    pub fn get_repo_dir(repo_name: &str) -> Result<PathBuf> {
        Ok(Self::get_maram_dir()?.join(repo_name))
    }

    pub fn get_worktree_dir(repo_name: &str, branch_name: &str) -> Result<PathBuf> {
        Ok(Self::get_repo_dir(repo_name)?.join(branch_name))
    }

    pub fn list_worktree_sets(repo_name: &str) -> Result<Vec<String>> {
        let repo_dir = Self::get_repo_dir(repo_name)?;

        if !repo_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sets = Vec::new();

        for entry in fs::read_dir(&repo_dir)
            .with_context(|| format!("Failed to read directory: {:?}", repo_dir))?
        {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir()
                && WorktreeMetadata::exists(&path)
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
            {
                sets.push(name.to_string());
            }
        }

        Ok(sets)
    }

    pub fn load(repo_name: &str, branch_name: &str) -> Result<Self> {
        let worktree_dir = Self::get_worktree_dir(repo_name, branch_name)?;
        let metadata = WorktreeMetadata::load(&worktree_dir)?;

        Ok(WorktreeSet {
            metadata,
            base_dir: worktree_dir,
        })
    }

    pub fn load_from_path(worktree_dir: &Path) -> Result<Self> {
        let metadata = WorktreeMetadata::load(worktree_dir)?;

        Ok(WorktreeSet {
            metadata,
            base_dir: worktree_dir.to_path_buf(),
        })
    }

    pub fn is_in_worktree_set() -> bool {
        if let Ok(current_dir) = std::env::current_dir()
            && let Ok(maram_dir) = Self::get_maram_dir()
        {
            return current_dir.starts_with(&maram_dir);
        }
        false
    }

    pub fn find_current() -> Result<Self> {
        if !Self::is_in_worktree_set() {
            anyhow::bail!("This command must be run from within a worktree set directory");
        }

        let current_dir = std::env::current_dir()?;
        let maram_dir = Self::get_maram_dir()?;

        let mut path = current_dir.clone();
        while path.starts_with(&maram_dir) && path != maram_dir {
            if WorktreeMetadata::exists(&path) {
                return Self::load_from_path(&path);
            }
            path = path
                .parent()
                .ok_or_else(|| {
                    anyhow::anyhow!("Reached filesystem root without finding worktree set")
                })?
                .to_path_buf();
        }

        anyhow::bail!("Could not find worktree set metadata");
    }

    pub fn format_variant_branch(variant: &str, base_branch: &str) -> String {
        format!("{}/{}", variant, base_branch)
    }

    pub fn is_in_base_worktree(&self) -> Result<bool> {
        let current_dir = std::env::current_dir()?;
        let base_path = fs::canonicalize(&self.metadata.base_path)
            .unwrap_or_else(|_| self.metadata.base_path.clone());
        let current_path = fs::canonicalize(&current_dir).unwrap_or(current_dir);

        Ok(current_path.starts_with(&base_path)
            && !self.metadata.variant_paths.values().any(|variant_path| {
                if let Ok(canonical_variant) = fs::canonicalize(variant_path) {
                    current_path.starts_with(&canonical_variant)
                } else {
                    false
                }
            }))
    }
}
