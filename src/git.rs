use anyhow::{Context, Result};
use git2::{
    Branch, CherrypickOptions, ErrorCode, IndexAddOption, Repository, Signature,
    WorktreePruneOptions,
};
use std::fs;
use std::path::Path;
use std::process::Command;

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

        if let Ok(current_dir) = std::env::current_dir()
            && let Ok(maram_dir) = crate::worktree_set::WorktreeSet::get_maram_dir()
            && current_dir.starts_with(&maram_dir)
        {
            let relative = current_dir
                .strip_prefix(&maram_dir)
                .ok()
                .and_then(|p| p.components().next())
                .and_then(|c| c.as_os_str().to_str());

            if let Some(repo_name) = relative {
                return Ok(repo_name.to_string());
            }
        }

        let git_path = self.repo.path();

        let main_repo_path = if git_path.to_string_lossy().contains("worktrees") {
            git_path
                .parent() // .git/worktrees/<name>/
                .and_then(|p| p.parent()) // .git/worktrees/
                .and_then(|p| p.parent()) // .git/
                .and_then(|p| p.parent()) // repository root
                .unwrap_or_else(|| git_path.parent().unwrap_or(Path::new(".")))
        } else {
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

    pub fn add_worktree(&self, path: &Path, branch: &str) -> Result<()> {
        self.repo
            .find_branch(branch, git2::BranchType::Local)
            .with_context(|| format!("Branch {} does not exist", branch))?;

        let workdir = self
            .repo
            .workdir()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| anyhow::anyhow!("Repository has no working directory"))?;

        // shell out to binary instead of using git2 to allow for sparse checkouts
        let output = Command::new("git")
            .current_dir(&workdir)
            .arg("worktree")
            .arg("add")
            .arg(path)
            .arg(branch)
            .output()
            .with_context(|| format!("Failed to execute git worktree add for {:?}", path))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "Failed to add worktree for branch {} at {:?}: {}",
                branch,
                path,
                stderr.trim()
            );
        }

        Ok(())
    }

    pub fn remove_worktree(&self, path: &Path) -> Result<()> {
        let worktree_path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let worktree_name = Self::find_worktree_name_by_path(&self.repo, &worktree_path)
            .with_context(|| format!("Failed to find worktree at path: {:?}", path))?;
        let worktree_name = worktree_name
            .ok_or_else(|| anyhow::anyhow!("Worktree not found at path: {:?}", path))?;
        let worktree = self.repo.find_worktree(&worktree_name).with_context(|| {
            format!(
                "Failed to load git worktree metadata for `{}`",
                worktree_name
            )
        })?;

        let mut prune_opts = WorktreePruneOptions::new();
        prune_opts.valid(true);
        prune_opts.working_tree(true);

        worktree
            .prune(Some(&mut prune_opts))
            .with_context(|| format!("Failed to remove worktree `{}`", worktree_name))?;

        drop(worktree);

        if worktree_path.exists() {
            fs::remove_dir_all(&worktree_path).with_context(|| {
                format!(
                    "Failed to clean worktree directory `{}`",
                    worktree_path.display()
                )
            })?;
        }
        Ok(())
    }

    fn find_worktree_name_by_path(
        repo: &Repository,
        worktree_path: &Path,
    ) -> Result<Option<String>> {
        let target = worktree_path
            .canonicalize()
            .unwrap_or_else(|_| worktree_path.to_path_buf());

        let names = repo
            .worktrees()
            .context("Failed to list repository worktrees")?;

        for name in names.iter().flatten() {
            let worktree = match repo.find_worktree(name) {
                Ok(worktree) => worktree,
                Err(err) if err.code() == ErrorCode::NotFound => continue,
                Err(err) => {
                    return Err(anyhow::anyhow!(
                        "Failed to open git worktree `{}`: {}",
                        name,
                        err
                    ));
                }
            };

            let path = worktree
                .path()
                .canonicalize()
                .unwrap_or_else(|_| worktree.path().to_path_buf());
            if path == target {
                return Ok(Some(name.to_owned()));
            }
        }

        Ok(None)
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

    pub fn has_commits_between(&self, base: &str, branch: &str) -> Result<bool> {
        let base_oid = self
            .repo
            .revparse_single(base)
            .with_context(|| format!("Failed to resolve base reference: {}", base))?
            .id();

        let branch_oid = self
            .repo
            .revparse_single(branch)
            .with_context(|| format!("Failed to resolve branch reference: {}", branch))?
            .id();

        if base_oid == branch_oid {
            return Ok(false);
        }

        let mut revwalk = self.repo.revwalk().context("Failed to create revwalk")?;
        revwalk
            .push(branch_oid)
            .with_context(|| format!("Failed to push branch {} to revwalk", branch))?;
        revwalk
            .hide(base_oid)
            .with_context(|| format!("Failed to hide base {} from revwalk", base))?;

        let has_commits = revwalk.next().is_some();
        Ok(has_commits)
    }

    pub fn cherry_pick_commits(&self, from: &str, to: &str) -> Result<bool> {
        let from_oid = self
            .repo
            .revparse_single(from)
            .with_context(|| format!("Failed to resolve from reference: {}", from))?
            .id();

        let to_oid = self
            .repo
            .revparse_single(to)
            .with_context(|| format!("Failed to resolve to reference: {}", to))?
            .id();

        let mut revwalk = self.repo.revwalk().context("Failed to create revwalk")?;
        revwalk
            .push(to_oid)
            .with_context(|| format!("Failed to push to {} to revwalk", to))?;
        revwalk
            .hide(from_oid)
            .with_context(|| format!("Failed to hide from {} from revwalk", from))?;

        let mut commits: Vec<git2::Oid> = revwalk
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to walk commits")?;
        commits.reverse();

        let mut opts = CherrypickOptions::new();

        for commit_oid in commits {
            let commit = self
                .repo
                .find_commit(commit_oid)
                .with_context(|| format!("Failed to find commit {}", commit_oid))?;

            match self.repo.cherrypick(&commit, Some(&mut opts)) {
                Ok(()) => {
                    let index = self
                        .repo
                        .index()
                        .context("Failed to get repository index")?;
                    if index.has_conflicts() {
                        return Ok(false);
                    }
                }
                Err(e) if e.code() == ErrorCode::Conflict => {
                    // Conflicts are expected and should be handled by the caller
                    return Ok(false);
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Failed to cherry-pick commit {}: {}",
                        commit_oid,
                        e
                    ));
                }
            }
        }

        Ok(true)
    }

    pub fn commit_changes(&self, message: &str) -> Result<()> {
        let mut index = self
            .repo
            .index()
            .context("Failed to get repository index")?;

        index
            .add_all(["*"], IndexAddOption::DEFAULT, None)
            .context("Failed to add changes to index")?;

        index.write().context("Failed to write index")?;

        let tree_id = index
            .write_tree()
            .context("Failed to write tree from index")?;
        let tree = self
            .repo
            .find_tree(tree_id)
            .context("Failed to find tree")?;

        let signature = self
            .repo
            .signature()
            .or_else(|_| Signature::now("maram", "maram@maram.local"))
            .context("Failed to create signature")?;

        match self.repo.head() {
            Ok(head) => {
                let parent_commit = head.peel_to_commit().context("Failed to get HEAD commit")?;
                self.repo
                    .commit(
                        Some("HEAD"),
                        &signature,
                        &signature,
                        message,
                        &tree,
                        &[&parent_commit],
                    )
                    .context("Failed to create commit")?;
            }
            Err(e) if e.code() == ErrorCode::UnbornBranch => {
                self.repo
                    .commit(Some("HEAD"), &signature, &signature, message, &tree, &[])
                    .context("Failed to create initial commit")?;
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to get HEAD: {}", e));
            }
        }

        Ok(())
    }

    pub fn diff_branches(&self, branch1: &str, branch2: &str) -> Result<()> {
        // shell out to binary instead of using git2 to allow for custom pagers
        let status = Command::new("git")
            .args(["diff", branch1, branch2])
            .status()
            .context("Failed to execute git diff")?;

        if !status.success() {
            anyhow::bail!("git diff failed");
        }

        Ok(())
    }

    pub fn get_upstream_branch(&self, branch: &str) -> Result<Option<String>> {
        let branch_ref = self
            .repo
            .find_branch(branch, git2::BranchType::Local)
            .with_context(|| format!("Branch {} does not exist", branch))?;

        let upstream = match branch_ref.upstream() {
            Ok(upstream_branch) => {
                let name = upstream_branch
                    .name()
                    .with_context(|| "Failed to get upstream branch name")?
                    .ok_or_else(|| anyhow::anyhow!("Upstream branch name is not valid UTF-8"))?
                    .to_string();
                Some(name)
            }
            Err(e) if e.code() == ErrorCode::NotFound => None,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to get upstream for branch {}: {}",
                    branch,
                    e
                ))
            }
        };

        Ok(upstream)
    }

    pub fn rebase_branch(&self, worktree_path: &Path, branch: &str, onto: &str) -> Result<()> {
        let output = Command::new("git")
            .current_dir(worktree_path)
            .args(["rebase", onto])
            .output()
            .with_context(|| {
                format!(
                    "Failed to execute git rebase {} onto {}",
                    branch, onto
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Abort the rebase
            Command::new("git")
                .current_dir(worktree_path)
                .args(["rebase", "--abort"])
                .output()
                .ok();

            anyhow::bail!(
                "Failed to rebase {} onto {}\nstdout: {}\nstderr: {}",
                branch,
                onto,
                stdout.trim(),
                stderr.trim()
            );
        }

        Ok(())
    }
}
