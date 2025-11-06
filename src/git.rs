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

        // Try to detect if we're in a maram worktree directory structure
        // Path structure: ~/maram/<repo_name>/<branch_name>/...
        if let Ok(current_dir) = std::env::current_dir()
            && let Ok(maram_dir) = crate::worktree_set::WorktreeSet::get_maram_dir()
            && current_dir.starts_with(&maram_dir)
        {
            // We're in a maram directory, extract repo name from path
            // ~/maram/<repo_name>/<branch_name>/...
            let relative = current_dir
                .strip_prefix(&maram_dir)
                .ok()
                .and_then(|p| p.components().next())
                .and_then(|c| c.as_os_str().to_str());

            if let Some(repo_name) = relative {
                return Ok(repo_name.to_string());
            }
        }

        // For worktrees, repo.path() points to .git/worktrees/<name>/
        // We need to get the main repository path
        let git_path = self.repo.path();

        // Check if we're in a worktree (worktrees have .git as a file, not a directory)
        // But git2's repo.path() for worktrees points to .git/worktrees/<name>/
        // We can detect this by checking if the path contains "worktrees"
        let main_repo_path = if git_path.to_string_lossy().contains("worktrees") {
            // For worktrees, navigate up to find the main .git directory
            // .git/worktrees/<name>/ -> .git/ -> repository root
            git_path
                .parent() // .git/worktrees/<name>/
                .and_then(|p| p.parent()) // .git/worktrees/
                .and_then(|p| p.parent()) // .git/
                .and_then(|p| p.parent()) // repository root
                .unwrap_or_else(|| git_path.parent().unwrap_or(Path::new(".")))
        } else {
            // Regular repository, .git is in the repo root
            git_path.parent().unwrap_or(Path::new("."))
        };

        let name = main_repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        Ok(name)
    }

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

    pub fn has_commits_between(&self, base: &str, branch: &str) -> Result<bool> {
        use std::process::Command;

        let output = Command::new("git")
            .args(["rev-list", "--count", &format!("{}..{}", base, branch)])
            .output()
            .context("Failed to execute git rev-list")?;

        if !output.status.success() {
            // If the command fails, the branch might not exist or there's an issue
            return Ok(false);
        }

        let count_str = String::from_utf8(output.stdout)
            .context("Failed to parse git rev-list output")?
            .trim()
            .to_string();

        let count: u32 = count_str.parse().context("Failed to parse commit count")?;

        Ok(count > 0)
    }

    /// Checkout a branch in the repository
    /// If workdir is provided, run the command in that directory; otherwise use the repo's workdir
    pub fn checkout_branch(&self, branch: &str, workdir: Option<&Path>) -> Result<()> {
        use std::process::Command;

        let dir = workdir
            .or_else(|| self.repo.workdir())
            .unwrap_or_else(|| Path::new("."));

        let status = Command::new("git")
            .args(["checkout", branch])
            .current_dir(dir)
            .status()
            .context("Failed to execute git checkout")?;

        if !status.success() {
            anyhow::bail!("git checkout failed");
        }

        Ok(())
    }

    /// Cherry-pick commits from one branch to another (without committing)
    /// Returns true if successful, false if there are conflicts
    /// If workdir is provided, run the command in that directory; otherwise use the repo's workdir
    pub fn cherry_pick_commits(
        &self,
        from: &str,
        to: &str,
        workdir: Option<&Path>,
    ) -> Result<bool> {
        use std::process::Command;

        let dir = workdir
            .or_else(|| self.repo.workdir())
            .unwrap_or_else(|| Path::new("."));

        let status = Command::new("git")
            .args(["cherry-pick", "--no-commit", &format!("{}..{}", from, to)])
            .current_dir(dir)
            .status()
            .context("Failed to execute git cherry-pick")?;

        Ok(status.success())
    }

    /// Commit changes with a message
    /// If workdir is provided, run the command in that directory; otherwise use the repo's workdir
    pub fn commit_changes(&self, message: &str, workdir: Option<&Path>) -> Result<()> {
        use std::process::Command;

        let dir = workdir
            .or_else(|| self.repo.workdir())
            .unwrap_or_else(|| Path::new("."));

        let status = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(dir)
            .status()
            .context("Failed to execute git commit")?;

        if !status.success() {
            anyhow::bail!("git commit failed");
        }

        Ok(())
    }
}
