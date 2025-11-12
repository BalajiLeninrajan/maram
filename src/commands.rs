use clap::{ArgAction, Subcommand};

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new worktree set
    #[command(alias = "c")]
    Create {
        /// Branch name (optional, will prompt if not provided)
        branch_name: Option<String>,
        /// Don't attach to zellij session, just drop into the base worktree directory
        #[arg(long = "no-session", short = 'n', action= ArgAction::SetTrue)]
        no_session: bool,
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
        #[arg(long = "no-session", short = 'n', action=ArgAction::SetTrue)]
        no_session: bool,
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
    /// List all worktree sets for the repo
    #[command(alias = "ls")]
    List,
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
        /// First variant name (defaults to base)
        variant1: String,
        /// Second variant name
        variant2: Option<String>,
    },
    /// Add a new worktree variant to the set
    #[command(alias = "a")]
    Add {
        /// Variant name (optional, will prompt if not provided)
        variant_name: Option<String>,
    },
    /// Remove a worktree variant from the set
    #[command(alias = "rm")]
    Remove {
        /// Variant name (optional, will prompt if not provided)
        variant_name: Option<String>,
    },
}
