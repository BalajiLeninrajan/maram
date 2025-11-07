use crate::config::Config;
use crate::git::GitRepo;
use crate::metadata::{LAYOUT_FILE, WorktreeMetadata};
use crate::worktree_set::WorktreeSet;
use crate::zellij::ZellijSession;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dialoguer::{Input, Select, theme::ColorfulTheme};
use std::process::Command;

#[derive(Parser)]
#[command(name = "maram")]
#[command(about = "Manage git worktree workflow")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new worktree set
    #[command(alias = "c")]
    Create {
        /// Branch name (optional, will prompt if not provided)
        branch_name: Option<String>,
        /// Don't attach to zellij session, just drop into the base worktree directory
        #[arg(long = "no-session", short = 'n')]
        no_session: Option<bool>,
        /// Variants to create (skips interactive TUI). If not provided, uses interactive TUI. If provided with no values, only creates the base branch.
        #[arg(long = "variants", short='v', num_args = 0..)]
        variants: Option<Vec<String>>,
    },
    /// Checkout/switch to a worktree set
    #[command(alias = "co")]
    Checkout {
        /// Branch name (optional, will prompt if not provided)
        branch_name: Option<String>,
        /// Don't attach to zellij session, just print the directory path
        #[arg(long = "no-session", short = 'n')]
        no_session: Option<bool>,
    },
    /// Delete a worktree set
    #[command(alias = "d")]
    Delete {
        /// Branch name (optional, will prompt if not provided)
        branch_name: Option<String>,
    },
    /// Show status of current worktree set
    #[command(alias = "s")]
    Status,
    /// Pick a variant to apply to base
    #[command(alias = "p")]
    Pick {
        /// Variant name (optional, will prompt if not provided)
        variant_name: Option<String>,
    },
    /// Reset base branch to original state
    #[command(alias = "r")]
    Reset,
    /// Diff between variants
    Diff {
        /// First variant name
        variant1: String,
        /// Second variant name (defaults to base)
        variant2: Option<String>,
    },
}

fn select_from_list(items: &[String], prompt: &str) -> Result<String> {
    if items.is_empty() {
        anyhow::bail!("No items available to select from");
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .items(items)
        .default(0)
        .interact()?;

    Ok(items[selection].clone())
}

fn select_worktree_set(repo_name: &str, prompt: &str) -> Result<String> {
    let worktree_sets = WorktreeSet::list_worktree_sets(repo_name)?;

    if worktree_sets.is_empty() {
        anyhow::bail!("No worktree sets found for repository '{}'", repo_name);
    }

    select_from_list(&worktree_sets, prompt)
}

fn manage_variants_interactive(default_variants: Vec<String>) -> Result<Vec<String>> {
    let mut variants = default_variants;

    loop {
        let mut items = vec!["[Done]".to_string()];
        items.extend(variants.iter().map(|v| format!("[Remove] {}", v)));
        items.push("[Add new]".to_string());

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Manage variants")
            .items(&items)
            .default(0)
            .interact()?;

        match selection {
            0 => break,
            i if i <= variants.len() => {
                // Remove variant
                variants.remove(i - 1);
            }
            _ => {
                // Add new variant
                let new_variant: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("New variant name")
                    .interact_text()?;
                if !variants.contains(&new_variant) {
                    variants.push(new_variant);
                }
            }
        }
    }

    Ok(variants)
}

fn sanitize_branch_name(name: &str) -> String {
    name.replace(' ', "-")
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '~' | '^' | ':' | '\\' | '?' | '*' | '[' | ']' | '@' | '{' | '}'
            )
        })
        .collect()
}

fn drop_into_shell(target_dir: &std::path::Path) -> Result<()> {
    std::env::set_current_dir(target_dir)
        .with_context(|| format!("Failed to change directory to {}", target_dir.display()))?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let status = Command::new(&shell)
        .current_dir(target_dir)
        .status()
        .with_context(|| {
            format!(
                "Failed to launch interactive shell '{}' in {}",
                shell,
                target_dir.display()
            )
        })?;

    if !status.success() {
        anyhow::bail!("Shell exited with a non-zero status");
    }

    Ok(())
}

pub fn handle_create(
    branch_name: Option<String>,
    no_session: Option<bool>,
    cli_variants: Option<Vec<String>>,
) -> Result<()> {
    let repo = GitRepo::open_from_current_dir()?;
    let repo_name = repo.get_repo_name()?;
    let config = Config::load()?;
    let no_session = no_session.unwrap_or(config.no_session);

    let branch_name = if let Some(name) = branch_name {
        name
    } else {
        Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Branch name")
            .interact_text()?
    };

    let branch_name = sanitize_branch_name(&branch_name);

    let existing_sets = WorktreeSet::list_worktree_sets(&repo_name)?;
    if existing_sets.contains(&branch_name) {
        anyhow::bail!(
            "Worktree set '{}' already exists. Use 'maram checkout {}' to switch to it, or 'maram delete {}' to remove it first.",
            branch_name,
            branch_name,
            branch_name
        );
    }

    let worktree_dir = WorktreeSet::get_worktree_dir(&repo_name, &branch_name)?;
    if worktree_dir.exists() {
        // Check if it's empty or has content
        let is_empty = worktree_dir
            .read_dir()
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);

        if !is_empty {
            anyhow::bail!(
                "Directory '{}' already exists and is not empty. Please remove it manually or use a different branch name.",
                worktree_dir.display()
            );
        }
    }

    let variants = if let Some(cli_variants) = cli_variants {
        cli_variants
    } else {
        manage_variants_interactive(config.default_variants.clone())?
    };

    let base_branch = branch_name.clone();
    if repo.branch_exists(&base_branch) {
        anyhow::bail!(
            "Branch '{}' already exists. Use a different branch name or delete the existing branch first.",
            base_branch
        );
    }

    let variant_branches: Vec<String> = variants
        .iter()
        .map(|v| WorktreeSet::format_variant_branch(v, &base_branch))
        .collect();

    for variant_branch in &variant_branches {
        if repo.branch_exists(variant_branch) {
            anyhow::bail!(
                "Branch '{}' already exists. Use a different variant name or delete the existing branch first.",
                variant_branch
            );
        }
    }

    // Create all branches
    repo.create_branch(&base_branch)?;
    for variant_branch in &variant_branches {
        repo.create_branch(variant_branch)?;
    }

    // Create worktree directories
    std::fs::create_dir_all(&worktree_dir)?;

    let base_path = worktree_dir.join("base");
    repo.add_worktree(&base_path, &base_branch)?;

    let mut variant_paths = std::collections::HashMap::new();
    for (variant, variant_branch) in variants.iter().zip(variant_branches.iter()) {
        let variant_path = worktree_dir.join(variant);
        repo.add_worktree(&variant_path, variant_branch)?;
        variant_paths.insert(variant.clone(), variant_path);
    }

    // Get base commit
    let head = repo.repo().head()?;
    let base_commit = head.target().unwrap().to_string();

    // Create metadata
    let mut metadata = WorktreeMetadata::new(branch_name.clone(), variants.clone());
    metadata.base_path = base_path.clone();
    metadata.variant_paths = variant_paths;
    metadata.base_commit = base_commit;
    metadata.save(&worktree_dir)?;

    println!(
        "Created worktree set '{}' with {} variants",
        branch_name,
        variants.len()
    );

    if no_session {
        // Drop into the base worktree instead of attaching to zellij
        drop_into_shell(&base_path)?;
        return Ok(());
    }

    // Create zellij session
    let session = ZellijSession::from_repo_and_branch(&repo_name, &branch_name);
    let tabs = ZellijSession::tabs_from_metadata(&metadata);

    // Save layout to metadata directory
    let layout_path = WorktreeMetadata::metadata_dir(&worktree_dir).join(LAYOUT_FILE);
    session.save_layout(&layout_path, &tabs)?;
    session.create_session(&tabs, Some(&layout_path))?;

    println!("Zellij session '{}' created", session.name());

    Ok(())
}

pub fn handle_checkout(branch_name: Option<String>, no_session: Option<bool>) -> Result<()> {
    let repo = GitRepo::open_from_current_dir()?;
    let repo_name = repo.get_repo_name()?;
    let config = Config::load()?;
    let no_session = no_session.unwrap_or(config.no_session);

    let selected_branch = if let Some(name) = branch_name {
        name
    } else {
        select_worktree_set(&repo_name, "Select worktree set")?
    };

    if no_session {
        // Drop into the base worktree instead of attaching to zellij
        let worktree_set = WorktreeSet::load(&repo_name, &selected_branch)?;
        drop_into_shell(&worktree_set.metadata.base_path)?;
        return Ok(());
    }

    // Load metadata and create/attach session
    let worktree_set = WorktreeSet::load(&repo_name, &selected_branch)?;
    let session = ZellijSession::from_repo_and_branch(&repo_name, &selected_branch);
    let tabs = ZellijSession::tabs_from_metadata(&worktree_set.metadata);

    // Try to load saved layout, otherwise create new one
    let layout_path = WorktreeMetadata::metadata_dir(&worktree_set.base_dir).join(LAYOUT_FILE);
    session.create_or_attach_with_layout(&tabs, Some(&layout_path))?;

    Ok(())
}

pub fn handle_delete(branch_name: Option<String>) -> Result<()> {
    let repo = GitRepo::open_from_current_dir()?;
    let repo_name = repo.get_repo_name()?;

    let selected_branch = if let Some(name) = branch_name {
        name
    } else {
        select_worktree_set(&repo_name, "Select worktree set to delete")?
    };

    // Load metadata
    let worktree_set = WorktreeSet::load(&repo_name, &selected_branch)?;

    // Kill zellij session
    let session = ZellijSession::from_repo_and_branch(&repo_name, &selected_branch);
    if session.session_exists() {
        session.kill_session().ok(); // Ignore errors
    }

    // Remove worktrees
    repo.remove_worktree(&worktree_set.metadata.base_path)?;
    for path in worktree_set.metadata.variant_paths.values() {
        repo.remove_worktree(path)?;
    }

    // Delete variant branches (keep base branch)
    for variant in worktree_set.metadata.variants.iter() {
        let branch_name = WorktreeSet::format_variant_branch(variant, &selected_branch);
        repo.delete_branch(&branch_name).ok(); // Ignore errors if branch doesn't exist
    }

    // Delete worktree directory
    std::fs::remove_dir_all(&worktree_set.base_dir)?;

    println!(
        "Deleted worktree set '{}', base branch still available",
        selected_branch
    );

    Ok(())
}

pub fn handle_status() -> Result<()> {
    let worktree_set = WorktreeSet::find_current()?;

    println!("Worktree set: {}", worktree_set.metadata.branch_name);
    println!(
        "Number of trees: {}",
        worktree_set.metadata.variants.len() + 1
    );
    if let Some(picked) = &worktree_set.metadata.current_picked_variant {
        println!("Current picked variant: {}", picked);
    } else {
        println!("Current picked variant: (none)");
    }

    Ok(())
}

pub fn handle_pick(variant_name: Option<String>) -> Result<()> {
    let mut worktree_set = WorktreeSet::find_current()?;

    let variant_name = if let Some(name) = variant_name {
        name
    } else {
        select_from_list(&worktree_set.metadata.variants, "Select variant to pick")?
    };

    if !worktree_set.metadata.variants.contains(&variant_name) {
        anyhow::bail!("Variant '{}' does not exist", variant_name);
    }

    let base_repo = GitRepo::open(&worktree_set.metadata.base_path)?;
    let base_branch = worktree_set.metadata.branch_name.clone();
    let base_commit = worktree_set.metadata.base_commit.clone();

    base_repo.checkout_branch(&base_branch)?;

    // Reset to base commit if there was a previous pick
    if let Some(prev_picked) = &worktree_set.metadata.current_picked_variant
        && prev_picked != &variant_name
    {
        println!("Resetting base branch to discard previous pick...");
        let base_commit_oid = base_commit
            .parse::<git2::Oid>()
            .context("Failed to parse base commit")?;
        base_repo.reset_to_commit(base_commit_oid)?;
    }

    // Get variant branch
    let variant_branch = WorktreeSet::format_variant_branch(&variant_name, &base_branch);

    let has_commits = base_repo.has_commits_between(&base_commit, &variant_branch)?;

    if !has_commits {
        println!("Variant '{}' has no commits to pick.", variant_name);
        println!("The variant branch is at the same commit as the base branch.");
        worktree_set.metadata.current_picked_variant = Some(variant_name.clone());
        worktree_set.metadata.save(&worktree_set.base_dir)?;

        println!(
            "Updated metadata to reflect variant '{}' as the current pick.",
            variant_name
        );
        return Ok(());
    }

    println!(
        "Picking variant '{}' (this may have conflicts)...",
        variant_name
    );
    println!("Warning: This operation is destructive. Conflicts must be resolved manually.");

    let success = base_repo.cherry_pick_commits(
        &base_commit,
        &variant_branch,
    )?;

    if !success {
        println!("Cherry-pick has conflicts. Please resolve them manually.");
        println!("After resolving, run: git commit");
        anyhow::bail!("Cherry-pick failed with conflicts");
    }

    base_repo.commit_changes(
        &format!("Pick variant {}", variant_name),
    )?;

    worktree_set.metadata.current_picked_variant = Some(variant_name.clone());
    worktree_set.metadata.save(&worktree_set.base_dir)?;

    println!("Picked variant '{}'", variant_name);

    Ok(())
}

pub fn handle_reset() -> Result<()> {
    let mut worktree_set = WorktreeSet::find_current()?;

    // Reset to base commit
    let base_commit = worktree_set
        .metadata
        .base_commit
        .parse::<git2::Oid>()
        .context("Failed to parse base commit")?;
    let base_repo = GitRepo::open(&worktree_set.metadata.base_path)?;
    base_repo.reset_to_commit(base_commit)?;

    // Update metadata
    worktree_set.metadata.current_picked_variant = None;
    worktree_set.metadata.save(&worktree_set.base_dir)?;

    println!("Reset base branch to original state");

    Ok(())
}

pub fn handle_diff(variant1: String, variant2: Option<String>) -> Result<()> {
    if !WorktreeSet::is_in_worktree_set() {
        anyhow::bail!("This command must be run from within a worktree set directory");
    }

    let current_dir = std::env::current_dir()?;

    let mut path = current_dir.clone();
    while path.starts_with(&WorktreeSet::get_maram_dir()?) {
        if WorktreeMetadata::exists(&path) {
            // Use the found directory directly instead of reconstructing it
            let worktree_set = WorktreeSet::load_from_path(&path)?;

            // Get worktree paths from metadata
            let worktree_path1 = if variant1 == "base" {
                worktree_set.metadata.base_path.clone()
            } else {
                worktree_set
                    .metadata
                    .variant_paths
                    .get(&variant1)
                    .ok_or_else(|| anyhow::anyhow!("Variant '{}' not found", variant1))?
                    .clone()
            };

            let worktree_path2 = if let Some(v2) = variant2 {
                if v2 == "base" {
                    worktree_set.metadata.base_path
                } else {
                    worktree_set
                        .metadata
                        .variant_paths
                        .get(&v2)
                        .ok_or_else(|| anyhow::anyhow!("Variant '{}' not found", v2))?
                        .clone()
                }
            } else {
                worktree_set.metadata.base_path
            };

            use std::process::Command;
            let status = Command::new("diff")
                .args([
                    "-r",
                    worktree_path1.to_str().unwrap(),
                    worktree_path2.to_str().unwrap(),
                ])
                .status()
                .context("Failed to execute diff -r")?;

            // diff returns exit code 1 when differences are found, which is normal
            // Exit code 0 means no differences, exit code 2 means error
            if status.code() == Some(2) {
                anyhow::bail!("diff -r failed");
            }

            return Ok(());
        }
        path = path.parent().unwrap().to_path_buf();
    }

    anyhow::bail!("Could not find worktree set metadata");
}
