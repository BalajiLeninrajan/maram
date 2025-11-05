use anyhow::{Context, Result};
use git2::{Branch, Repository, Worktree, WorktreeAddOptions};
use std::path::Path;

pub struct GitRepo {
    repo: Repository,
}

impl GitRepo {
    pub fn open(path: &Path) -> Result<Self> {
        let repo = Repository::open(path)
            .with_context(|| format!("Failed to open git repository at: {:?}", path))?;
        Ok(GitRepo { repo })
    }

    pub fn open_from_current_dir() -> Result<Self> {
        let repo = Repository::discover(".")
            .context("Failed to discover git repository. Are you in a git repo?")?;
        Ok(GitRepo { repo })
    }

    pub fn repo(&self) -> &Repository {
        &self.repo
    }

    pub fn get_repo_name(&self) -> Result<String> {
        if let Ok(remote) = self.repo.find_remote("origin")
            && let Some(url) = remote.url()
        {
            // Extract repo name from URL (e.g., https://github.com/user/repo.git -> repo)
            let mut parts = url.split('/');
            let name = parts
                .next_back()
                .and_then(|s| s.strip_suffix(".git"))
                .unwrap_or_else(|| parts.next_back().unwrap_or("unknown"))
                .to_string();
            return Ok(name);
        }

        // Fallback to directory name
        let path = self.repo.path().parent().unwrap_or(Path::new("."));
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        Ok(name)
    }

    // pub fn get_current_head(&self) -> Result<String> {
    //     let head = self.repo.head().context("Failed to get HEAD")?;
    //     let name = head.shorthand().context("HEAD is not a branch")?;
    //     Ok(name.to_string())
    // }

    pub fn create_branch(&self, name: &str) -> Result<Branch<'_>> {
        let head = self.repo.head().context("Failed to get HEAD")?;
        let head_commit = head.peel_to_commit().context("Failed to get HEAD commit")?;

        let branch = self
            .repo
            .branch(name, &head_commit, false)
            .with_context(|| format!("Failed to create branch: {}", name))?;
        Ok(branch)
    }

    pub fn branch_exists(&self, name: &str) -> bool {
        self.repo.find_branch(name, git2::BranchType::Local).is_ok()
    }

    pub fn add_worktree(&self, path: &Path, branch: &str) -> Result<Worktree> {
        let mut opts = WorktreeAddOptions::new();
        let branch_ref = self
            .repo
            .find_branch(branch, git2::BranchType::Local)
            .with_context(|| format!("Branch {} does not exist", branch))?;
        let reference = branch_ref.into_reference();
        opts.reference(Some(&reference));

        // Use a sanitized worktree name (replace slashes with dashes) to avoid directory creation issues
        // The worktree name is used as an identifier in .git/worktrees/, so it can't contain slashes
        let worktree_name = branch.replace('/', "-");

        let worktree = self
            .repo
            .worktree(&worktree_name, path, Some(&opts))
            .with_context(|| {
                format!("Failed to add worktree for branch {} at {:?}", branch, path)
            })?;
        Ok(worktree)
    }

    pub fn remove_worktree(&self, path: &Path) -> Result<()> {
        // Git2 doesn't have a direct remove_worktree method, so we'll use git command
        use std::process::Command;

        let status = Command::new("git")
            .args(["worktree", "remove", path.to_str().unwrap()])
            .status()
            .context("Failed to execute git worktree remove")?;

        if !status.success() {
            anyhow::bail!("git worktree remove failed");
        }

        Ok(())
    }

    // pub fn list_worktrees(&self) -> Result<Vec<PathBuf>> {
    //     let mut worktrees = Vec::new();
    //
    //     let worktree_names = self.repo.worktrees().context("Failed to list worktrees")?;
    //
    //     for name in worktree_names.iter().flatten() {
    //         if let Ok(worktree) = self.repo.find_worktree(name) {
    //             let path = worktree.path();
    //             worktrees.push(PathBuf::from(path));
    //         }
    //     }
    //
    //     Ok(worktrees)
    // }

    pub fn delete_branch(&self, name: &str) -> Result<()> {
        let mut branch = self
            .repo
            .find_branch(name, git2::BranchType::Local)
            .with_context(|| format!("Branch {} does not exist", name))?;
        branch
            .delete()
            .with_context(|| format!("Failed to delete branch: {}", name))?;
        Ok(())
    }

    // pub fn get_base_commit_oid(&self) -> Result<git2::Oid> {
    //     let head = self.repo.head().context("Failed to get HEAD")?;
    //     let head_commit = head.peel_to_commit().context("Failed to get HEAD commit")?;
    //     Ok(head_commit.id())
    // }

    // pub fn cherry_pick(&self, commit_id: git2::Oid) -> Result<()> {
    //     // Use git command instead for better conflict handling
    //     use std::process::Command;
    //
    //     let status = Command::new("git")
    //         .args(["cherry-pick", &commit_id.to_string()])
    //         .status()
    //         .context("Failed to execute git cherry-pick")?;
    //
    //     if !status.success() {
    //         anyhow::bail!("Cherry-pick failed. Please resolve conflicts manually.");
    //     }
    //
    //     Ok(())
    // }

    pub fn reset_to_commit(&self, commit_id: git2::Oid) -> Result<()> {
        let commit = self
            .repo
            .find_commit(commit_id)
            .with_context(|| format!("Failed to find commit {}", commit_id))?;

        self.repo
            .reset(commit.as_object(), git2::ResetType::Hard, None)
            .context("Failed to reset to commit")?;

        Ok(())
    }

    pub fn get_diff(&self, branch1: &str, branch2: &str) -> Result<String> {
        use std::process::Command;

        let output = Command::new("git")
            .args(["diff", branch1, branch2])
            .output()
            .context("Failed to execute git diff")?;

        if !output.status.success() {
            anyhow::bail!("git diff failed");
        }

        String::from_utf8(output.stdout).context("Failed to parse git diff output")
    }
}
