use crate::commands::Commands;
use crate::config::Config;
use crate::git::GitRepo;
use crate::metadata::{LAYOUT_FILE, WorktreeMetadata};
use crate::worktree_set::WorktreeSet;
use crate::zellij::ZellijSession;
use anyhow::{Context, Result};
use clap::Parser;
use console::style;
use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use std::{process::Command, time::Duration};

#[derive(Parser)]
#[command(name = "maram")]
#[command(about = "Manage git worktree workflow")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

pub fn green<S: AsRef<str>>(string: S) -> String {
    style(string.as_ref()).green().to_string()
}

pub fn red<S: AsRef<str>>(string: S) -> String {
    style(string.as_ref()).red().to_string()
}

fn select_from_list(items: &[String], prompt: &str) -> Result<String> {
    if items.is_empty() {
        anyhow::bail!("{}", red("No items available to select from"));
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
        anyhow::bail!(
            "{}",
            red(format!(
                "No worktree sets found for repository '{}'",
                repo_name
            ))
        );
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
    name.replace([' ', '/'], "-")
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '~' | '^' | ':' | '\\' | '?' | '*' | '[' | ']' | '@' | '{' | '}'
            )
        })
        .collect()
}

fn format_variant_branch(variant: &str, base_branch: &str) -> String {
    let variant = sanitize_branch_name(variant);
    let base_branch = sanitize_branch_name(base_branch);
    format!("{}/{}", variant, base_branch)
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
        anyhow::bail!("{}", red("Shell exited with a non-zero status"));
    }

    Ok(())
}

fn run_with_spinner<F, T>(
    start_message: impl Into<String>,
    success_message: impl Into<String>,
    action: F,
) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    let start_message = start_message.into();
    let success_message = success_message.into();

    let spinner_style = ProgressStyle::with_template("{spinner} {msg}")
        .expect("spinner template is valid")
        .tick_strings(&["◐", "◓", "◑", "◒", &green("✔")]);

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(spinner_style);
    spinner.set_message(start_message.clone());
    spinner.enable_steady_tick(Duration::from_millis(120));

    let result = action();

    match result {
        Ok(value) => {
            spinner.finish_with_message(success_message);
            Ok(value)
        }
        Err(err) => {
            spinner.abandon_with_message(format!("{} failed", start_message));
            Err(err)
        }
    }
}

pub fn handle_create(
    branch_name: Option<String>,
    no_session: bool,
    cli_variants: Option<Vec<String>>,
) -> Result<()> {
    let repo = GitRepo::open_from_current_dir()?;
    let repo_name = repo.get_repo_name()?;
    let config = Config::load()?;

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
            "{}",
            red(format!(
                "Worktree set '{}' already exists. Use 'maram checkout {}' to switch to it, or 'maram delete {}' to remove it first.",
                branch_name, branch_name, branch_name
            ))
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
                "{}",
                red(format!(
                    "Directory '{}' already exists and is not empty. Please remove it manually or use a different branch name.",
                    worktree_dir.display()
                ))
            );
        }
    }

    let variants = if let Some(cli_variants) = cli_variants {
        cli_variants
    } else {
        manage_variants_interactive(config.default_variants.clone())?
    };

    let base_branch = branch_name.clone();
    let should_create_base = if repo.branch_exists(&base_branch) {
        let use_existing = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Branch '{}' already exists. Do you want to use this as your base branch?",
                base_branch
            ))
            .default(false)
            .interact()?;

        if !use_existing {
            anyhow::bail!(
                "{}",
                red(format!(
                    "Branch '{}' already exists. Use a different branch name or delete the existing branch first.",
                    base_branch
                ))
            );
        }
        false
    } else {
        true
    };

    let variant_branches: Vec<String> = variants
        .iter()
        .map(|v| format_variant_branch(v, &base_branch))
        .collect();

    for variant_branch in &variant_branches {
        if repo.branch_exists(variant_branch) {
            anyhow::bail!(
                "{}",
                red(format!(
                    "Branch '{}' already exists. Use a different variant name or delete the existing branch first.",
                    variant_branch
                ))
            );
        }
    }

    if should_create_base {
        repo.create_branch(&base_branch)?;
    }
    for variant_branch in &variant_branches {
        repo.create_branch(variant_branch)?;
    }

    // Create worktree directories
    std::fs::create_dir_all(&worktree_dir)?;

    let base_path = worktree_dir.join("base");
    run_with_spinner(
        format!("Creating base worktree for '{}'...", base_branch),
        format!("Created base worktree at {}", base_path.display()),
        || {
            repo.add_worktree(&base_path, &base_branch)?;
            Ok(())
        },
    )?;

    let mut variant_paths = std::collections::HashMap::new();
    for (index, (variant, variant_branch)) in
        variants.iter().zip(variant_branches.iter()).enumerate()
    {
        let variant_path = worktree_dir.join(variant);
        run_with_spinner(
            format!(
                "Creating worktree for variant '{}' ({}/{})...",
                variant,
                index + 1,
                variants.len()
            ),
            format!(
                "Created worktree for variant '{}' at {}",
                variant,
                variant_path.display()
            ),
            || {
                repo.add_worktree(&variant_path, variant_branch)?;
                Ok(())
            },
        )?;
        variant_paths.insert(variant.clone(), variant_path);
    }

    // Get base commit
    let head = repo.repo().head()?;
    let base_commit = head
        .target()
        .expect("HEAD must point to a valid commit")
        .to_string();

    // Get current branch as parent_branch
    let parent_branch = repo.get_current_branch()?;

    // Create metadata
    let mut metadata = WorktreeMetadata::new(branch_name.clone(), variants.clone());
    metadata.base_path = base_path.clone();
    metadata.variant_paths = variant_paths;
    metadata.base_commit = base_commit;
    metadata.parent_branch = parent_branch;
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

pub fn handle_checkout(branch_name: Option<String>, no_session: bool) -> Result<()> {
    let repo = GitRepo::open_from_current_dir()?;
    let repo_name = repo.get_repo_name()?;
    let config = Config::load()?;
    let no_session = no_session ^ config.no_session;

    let selected_branch = if let Some(name) = branch_name {
        sanitize_branch_name(&name)
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
        sanitize_branch_name(&name)
    } else {
        select_worktree_set(&repo_name, "Select worktree set to delete")?
    };

    let worktree_set = WorktreeSet::load(&repo_name, &selected_branch)?;

    run_with_spinner("Remove base worktree", "Removed base worktree", || {
        repo.remove_worktree(&worktree_set.metadata.base_path)?;
        Ok(())
    })?;

    let variant_count = worktree_set.metadata.variants.len();
    for (index, (variant_name, path)) in worktree_set.metadata.variant_paths.iter().enumerate() {
        run_with_spinner(
            format!(
                "Removing worktree for variant '{}' ({}/{})...",
                variant_name,
                index + 1,
                variant_count
            ),
            format!("Removed worktree for variant '{}'", variant_name),
            || {
                repo.remove_worktree(path)?;
                Ok(())
            },
        )?;
    }

    for (index, variant) in worktree_set.metadata.variants.iter().enumerate() {
        let branch_name = format_variant_branch(variant, &selected_branch);
        run_with_spinner(
            format!(
                "Deleting branch for variant '{}' ({}/{})...",
                variant,
                index + 1,
                variant_count
            ),
            format!("Deleted branch for variant '{}'", variant),
            || {
                repo.delete_branch(&branch_name)?;
                Ok(())
            },
        )?;
    }

    run_with_spinner(
        "Deleting worktree directory",
        "Deleted worktree directory",
        || {
            std::fs::remove_dir_all(&worktree_set.base_dir)?;
            Ok(())
        },
    )?;

    println!("Deleted worktree set '{}'", selected_branch);

    if Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "Do you want to delete the base branch '{}'?",
            selected_branch
        ))
        .default(false)
        .interact()?
    {
        run_with_spinner(
            format!("Deleting branch '{}'...", selected_branch),
            format!("Deleted branch '{}'", selected_branch),
            || {
                repo.delete_branch(&selected_branch)?;
                Ok(())
            },
        )?;
    }

    let session = ZellijSession::from_repo_and_branch(&repo_name, &selected_branch);
    if session.session_exists() {
        session.delete_session().ok(); // Ignore errors
    }

    Ok(())
}

pub fn handle_status() -> Result<()> {
    let worktree_set = WorktreeSet::find_current()?;

    println!(
        "{}{}",
        green("Worktree set: "),
        worktree_set.metadata.branch_name
    );
    println!(
        "{}{}",
        green("Number of trees: "),
        worktree_set.metadata.variants.len() + 1
    );
    if let Some(picked) = &worktree_set.metadata.current_picked_variant {
        println!("{}{}", green("Current picked variant: "), picked);
    } else {
        println!("{}(none)", green("Current picked variant: "));
    }

    Ok(())
}

pub fn handle_list() -> Result<()> {
    let repo = GitRepo::open_from_current_dir()?;
    let repo_name = repo.get_repo_name()?;
    let worktree_sets = WorktreeSet::list_worktree_sets(&repo_name)?;

    if worktree_sets.is_empty() {
        println!("No worktree sets found for repository '{}'", repo_name);
        return Ok(());
    }

    let current_set = WorktreeSet::find_current()
        .ok()
        .map(|ws| ws.metadata.branch_name);

    for worktree_set in worktree_sets {
        if current_set.as_ref() == Some(&worktree_set) {
            println!("{} {}", green("❯"), worktree_set);
        } else {
            println!("  {}", worktree_set);
        }
    }

    Ok(())
}

pub fn handle_pick(variant_name: Option<String>) -> Result<()> {
    let mut worktree_set = WorktreeSet::find_current()?;

    if !worktree_set.is_in_base_worktree()? {
        anyhow::bail!(
            "{}",
            red(
                "The 'pick' command can only be called from the base branch worktree. \
            Current directory is not in the base worktree. Please navigate to the base worktree first."
            )
        );
    }

    let variant_name = if let Some(name) = variant_name {
        name
    } else {
        select_from_list(&worktree_set.metadata.variants, "Select variant to pick")?
    };

    if !worktree_set.metadata.variants.contains(&variant_name) {
        anyhow::bail!(
            "{}",
            red(format!("Variant '{}' does not exist", variant_name))
        );
    }

    let base_repo = GitRepo::open(&worktree_set.metadata.base_path)?;
    let base_branch = worktree_set.metadata.branch_name.clone();
    let base_commit = worktree_set.metadata.base_commit.clone();

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
    let variant_branch = format_variant_branch(&variant_name, &base_branch);

    let has_commits = base_repo.has_commits_between(&base_commit, &variant_branch)?;

    if !has_commits {
        println!("Variant '{}' has no commits to pick.", variant_name);
        println!("The variant branch is at the same commit as the base branch.");
        worktree_set.metadata.current_picked_variant = Some(variant_name.clone());
        worktree_set.metadata.save(&worktree_set.base_dir)?;

        println!(
            "Updated metadata to reflect variant '{}' as the current pick.",
            green(variant_name)
        );
        return Ok(());
    }

    println!(
        "Picking variant '{}' (this may have conflicts)...",
        variant_name
    );
    println!("Warning: This operation is destructive. Conflicts must be resolved manually.");

    let success = base_repo.cherry_pick_commits(&base_commit, &variant_branch)?;

    if !success {
        println!(
            "{}",
            red("Cherry-pick has conflicts. Please resolve them manually.")
        );
        println!("After resolving, run: {}", green("git commit"));
        anyhow::bail!("{}", red("Cherry-pick failed with conflicts"));
    }

    base_repo.commit_changes(&format!(
        "Apply variant '{}' to '{}'",
        variant_name, base_branch
    ))?;

    worktree_set.metadata.current_picked_variant = Some(variant_name.clone());
    worktree_set.metadata.save(&worktree_set.base_dir)?;

    println!("Picked variant '{}'", green(variant_name));

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
    let worktree_set = WorktreeSet::find_current()?;
    let repo = GitRepo::open_from_current_dir()?;

    let base_branch = worktree_set.metadata.branch_name;

    let branch1 = if variant1 == "base" {
        base_branch.clone()
    } else {
        if !worktree_set.metadata.variants.contains(&variant1) {
            anyhow::bail!("{}", red(format!("Variant '{}' not found", variant1)));
        }
        format_variant_branch(&variant1, &base_branch)
    };

    let Some(variant2) = variant2 else {
        repo.diff_branches(&base_branch, &branch1)?;
        return Ok(());
    };

    let branch2 = if variant2 == "base" {
        base_branch
    } else {
        if !worktree_set.metadata.variants.contains(&variant2) {
            anyhow::bail!("{}", red(format!("Variant '{}' not found", variant2)));
        }
        format_variant_branch(&variant2, &base_branch)
    };

    repo.diff_branches(&branch1, &branch2)?;

    Ok(())
}

pub fn handle_add(variant_name: Option<String>) -> Result<()> {
    let mut worktree_set = WorktreeSet::find_current()?;
    let repo = GitRepo::open_from_current_dir()?;
    let repo_name = repo.get_repo_name()?;

    let variant_name = if let Some(name) = variant_name {
        name
    } else {
        Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Variant name")
            .interact_text()?
    };

    if worktree_set.metadata.variants.contains(&variant_name) {
        anyhow::bail!(
            "{}",
            red(format!(
                "Variant '{}' already exists in this worktree set",
                variant_name
            ))
        );
    }

    let base_branch = worktree_set.metadata.branch_name.clone();
    let variant_branch = format_variant_branch(&variant_name, &base_branch);

    if repo.branch_exists(&variant_branch) {
        anyhow::bail!(
            "{}",
            red(format!(
                "Branch '{}' already exists. Use a different variant name or delete the existing branch first.",
                variant_branch
            ))
        );
    }

    repo.create_branch(&variant_branch)?;

    let variant_path = worktree_set.base_dir.join(&variant_name);
    run_with_spinner(
        format!("Creating worktree for variant '{}'...", variant_name),
        format!(
            "Created worktree for variant '{}' at {}",
            variant_name,
            variant_path.display()
        ),
        || {
            repo.add_worktree(&variant_path, &variant_branch)?;
            Ok(())
        },
    )?;

    worktree_set.metadata.variants.push(variant_name.clone());
    worktree_set
        .metadata
        .variant_paths
        .insert(variant_name.clone(), variant_path.clone());
    worktree_set.metadata.save(&worktree_set.base_dir)?;

    let session = ZellijSession::from_repo_and_branch(&repo_name, &base_branch);
    if session.session_exists() {
        let tabs = ZellijSession::tabs_from_metadata(&worktree_set.metadata);
        let layout_path = WorktreeMetadata::metadata_dir(&worktree_set.base_dir).join(LAYOUT_FILE);
        session.save_layout(&layout_path, &tabs)?;
    }

    println!(
        "Added variant '{}' to worktree set '{}'",
        green(variant_name),
        base_branch
    );

    Ok(())
}

pub fn handle_remove(variant_name: Option<String>) -> Result<()> {
    let mut worktree_set = WorktreeSet::find_current()?;
    let repo = GitRepo::open_from_current_dir()?;
    let repo_name = repo.get_repo_name()?;

    let variant_name = if let Some(name) = variant_name {
        name
    } else {
        select_from_list(&worktree_set.metadata.variants, "Select variant to remove")?
    };

    if !worktree_set.metadata.variants.contains(&variant_name) {
        anyhow::bail!(
            "{}",
            red(format!(
                "Variant '{}' does not exist in this worktree set",
                variant_name
            ))
        );
    }

    let is_picked = worktree_set.metadata.current_picked_variant.as_ref() == Some(&variant_name);

    if is_picked {
        handle_reset()?;
        worktree_set = WorktreeSet::find_current()?;
    }

    let variant_path = worktree_set
        .metadata
        .variant_paths
        .get(&variant_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{}",
                red(format!("Variant path not found for '{}'", variant_name))
            )
        })?
        .clone();

    run_with_spinner(
        format!("Removing worktree for variant '{}'...", variant_name),
        format!("Removed worktree for variant '{}'", variant_name),
        || {
            repo.remove_worktree(&variant_path)?;
            Ok(())
        },
    )?;

    let base_branch = worktree_set.metadata.branch_name.clone();
    let variant_branch = format_variant_branch(&variant_name, &base_branch);
    repo.delete_branch(&variant_branch).ok(); // Ignore errors if branch doesn't exist

    worktree_set
        .metadata
        .variants
        .retain(|v| v != &variant_name);
    worktree_set.metadata.variant_paths.remove(&variant_name);
    worktree_set.metadata.save(&worktree_set.base_dir)?;

    let session = ZellijSession::from_repo_and_branch(&repo_name, &base_branch);
    if session.session_exists() {
        let tabs = ZellijSession::tabs_from_metadata(&worktree_set.metadata);
        let layout_path = WorktreeMetadata::metadata_dir(&worktree_set.base_dir).join(LAYOUT_FILE);
        session.save_layout(&layout_path, &tabs)?;
    }

    println!(
        "Removed variant '{}' from worktree set '{}'",
        green(variant_name),
        base_branch
    );

    Ok(())
}

pub fn handle_sync(branch: Option<String>) -> Result<()> {
    let mut worktree_set = WorktreeSet::find_current()?;
    let repo = GitRepo::open_from_current_dir()?;
    let base_branch = worktree_set.metadata.branch_name.clone();

    let parent_branch = branch
        .or(worktree_set.metadata.parent_branch.clone())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{}",
                red(
                    "No parent branch specified and no parent_branch found in metadata. \
            Provide a branch to sync with: maram sync <branch>"
                )
            )
        })?;

    let base_path = worktree_set.metadata.base_path.clone();
    run_with_spinner(
        format!(
            "Rebasing base branch '{}' onto '{}'...",
            base_branch, parent_branch
        ),
        format!(
            "Rebased base branch '{}' onto '{}'",
            base_branch, parent_branch
        ),
        || repo.rebase_branch(&base_path, &base_branch, &parent_branch),
    )
    .with_context(|| {
        format!(
            "To manually resolve conflicts, run:{}\n then run: {} again",
            green(format!(
                "\n  $ cd {}\n  $ git rebase {}",
                base_path.display(),
                parent_branch
            )),
            green("maram sync")
        )
    })?;

    let new_base_commit = repo.get_head_commit(&base_path)?;
    worktree_set.metadata.base_commit = new_base_commit;
    worktree_set.metadata.save(&worktree_set.base_dir)?;

    for (index, variant) in worktree_set.metadata.variants.iter().enumerate() {
        let variant_branch = format_variant_branch(variant, &base_branch);
        let variant_path = worktree_set
            .metadata
            .variant_paths
            .get(variant)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "{}",
                    red(format!("Variant path not found for '{}'", variant))
                )
            })?
            .clone();

        run_with_spinner(
            format!(
                "Rebasing variant '{}' onto '{}' ({}/{})...",
                variant,
                parent_branch,
                index + 1,
                worktree_set.metadata.variants.len()
            ),
            format!("Rebased variant '{}' onto '{}'", variant, parent_branch),
            || repo.rebase_branch(&variant_path, &variant_branch, &parent_branch),
        )
        .with_context(|| {
            format!(
                "To manually resolve conflicts, run:{}\n then run: {} again",
                green(format!(
                    "\n  $ cd {}\n  $ git rebase {}",
                    variant_path.display(),
                    parent_branch
                )),
                green("maram sync")
            )
        })?;
    }

    println!("{}", green("All branches synced successfully"));

    Ok(())
}
